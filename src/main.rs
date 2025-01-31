/*
Licensed to the Apache Software Foundation (ASF) under one
or more contributor license agreements.  See the NOTICE file
distributed with this work for additional information
regarding copyright ownership.  The ASF licenses this file
to you under the Apache License, Version 2.0 (the
"License"); you may not use this file except in compliance
with the License.  You may obtain a copy of the License at

  http://www.apache.org/licenses/LICENSE-2.0

  Unless required by applicable law or agreed to in writing,
software distributed under the License is distributed on an
"AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
KIND, either express or implied.  See the License for the
specific language governing permissions and limitations
under the License.
*/

use std::env::args;
use std::path::Path;
use std::time::SystemTime;

use anyhow::bail;
use anyhow::Result;

mod job;
mod job_factory;

mod node;
mod registry;
mod resource;

mod scheduler;

fn main() -> Result<()> {
    let arguments: Vec<_> = args().collect();

    if arguments.len() < 1 + 3 || arguments.len() > 1 + 3 + 1 {
        bail!("Expected arguments: \
            <path to node definition> \
            <path to node connection definition> \
            <path to job definition> \
            [<path to output file for output trace>]")
    }

    let path_nodes = Path::new(&arguments[1]);
    let path_connections = Path::new(&arguments[2]);
    let path_jobs = Path::new(&arguments[3]);

    println!("Instantiating node registry");
    let registry = registry::NodeRegistry::from_paths(path_nodes, path_connections).unwrap();

    println!("Instantiating job factory");
    let jfactory: Box<dyn job_factory::JobFactory>;
    if arguments.len() == 1 + 3 {
        let jf = job_factory::JobStreaming::from_path(path_jobs).unwrap();
        jfactory = Box::new(jf);
    } else {
        let path_output_trace = Path::new(&arguments[4]);
        let jf = job_factory::JobStreamingWithOutput::from_path_to_path(
            path_jobs, path_output_trace).unwrap();
        jfactory = Box::new(jf);
    }

    println!("Instantiating scheduler");
    let mut sched = scheduler::Scheduler::new(registry, jfactory);


    println!("Starting simulation");
    let report_every_secs = 5.0;
    let mut last_report_time = SystemTime::now();
    let start = last_report_time;

    let mut throughput_last = 0;
    let mut throughput_delta = 0;

    while sched.tick() {
        let now = SystemTime::now();
        let delta = now.duration_since(last_report_time).unwrap();

        if delta.as_secs_f32() > report_every_secs {
            let throughput = sched.jobs_running.len()
                + sched.jobs_done.len()
                + sched.jobs_queuing.len();
            throughput_delta += throughput - throughput_last;
            throughput_last = throughput;
            last_report_time = now;

            let since_beg = now.duration_since(start).unwrap();
            println!("{:#?}) At tick {}, finished: {} - running: {} - queueing: {}",
                     since_beg, sched.now, sched.jobs_done.len(), sched.jobs_running.len(),
                     sched.jobs_queuing.len());
            let (cores, memory) = sched.registry.get_max_cores_memory();
            println!("  Max cores: {}, Max memory: {}", cores, memory);
            println!("  Simulator throughput events: {}", throughput_delta);
            println!("  Simulator throughput events/sec: {}",
                     throughput_delta as f32 / (delta.as_secs_f32()));
            throughput_delta = 0;
        }
        if sched.has_unschedulable() {
            break
        }
    }
    let delta = SystemTime::now().duration_since(start).unwrap();

    println!("{}) Scheduled {} jobs in simulated seconds {}",
             delta.as_secs_f32(), sched.jobs_done.len(), sched.now);

    if sched.has_unschedulable() {
        eprintln!("There were {} unschedulable jobs", sched.jobs_queuing.len());

        for j in &sched.jobs_queuing {
            println!("{}", j);
        }

        bail!("Unable to schedule {} jobs", sched.jobs_queuing.len())
    } else {
        Ok(())
    }
}