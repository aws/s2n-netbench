// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::{net::SocketAddr, path::PathBuf};
use structopt::StructOpt;

mod client_coord;
mod client_worker;
mod server_coord;
mod server_worker;

#[derive(StructOpt, Debug, Clone)]
pub struct ClientContext {
    /// Run a test program instead of the Netbench process.
    #[structopt(long)]
    testing: bool,

    /// The path to the netbench utility and scenario file.
    #[structopt(long, default_value = "/home/ec2-user/bin")]
    netbench_path: PathBuf,

    /// Specify the Netbench driver which should be run.
    #[structopt(long)]
    driver: String,

    /// The name of the scenario file.
    ///
    /// https://github.com/aws/s2n-netbench/tree/main/netbench-scenarios
    #[structopt(long, default_value = "request_response.json")]
    scenario: String,

    /// List of Netbench Server the client should connect to.
    #[structopt(long)]
    netbench_servers: Vec<SocketAddr>,
}

#[derive(StructOpt, Debug, Clone)]
pub struct ServerContext {
    /// Run a test program instead of the Netbench process.
    #[structopt(long)]
    testing: bool,

    /// The path to the netbench utility and scenario file.
    #[structopt(long, default_value = "/home/ec2-user/bin")]
    netbench_path: PathBuf,

    /// Specify the Netbench driver which should be run.
    #[structopt(long)]
    driver: String,

    /// The name of the scenario file.
    ///
    /// https://github.com/aws/s2n-netbench/tree/main/netbench-scenarios
    #[structopt(long, default_value = "request_response.json")]
    scenario: String,

    /// The port which the Netbench Server process should accept connections.
    #[structopt(long, default_value = "4433")]
    netbench_port: u16,
}

impl ServerContext {
    #[cfg(test)]
    pub fn testing() -> Self {
        ServerContext {
            netbench_path: "".into(),
            driver: "".to_string(),
            scenario: "".to_string(),
            testing: true,
            netbench_port: 4433,
        }
    }

    pub fn trim_driver_name(&self) -> String {
        self.driver
            .trim_start_matches("s2n-netbench-driver-")
            .trim_start_matches("netbench-driver-")
            .trim_end_matches(".json")
            .to_owned()
    }
}

impl ClientContext {
    #[cfg(test)]
    pub fn testing() -> Self {
        ClientContext {
            netbench_servers: vec![],
            netbench_path: "".into(),
            driver: "".to_string(),
            scenario: "".to_string(),
            testing: true,
        }
    }

    pub fn trim_driver_name(&self) -> String {
        self.driver
            .trim_start_matches("s2n-netbench-driver-")
            .trim_start_matches("netbench-driver-")
            .trim_end_matches(".json")
            .to_owned()
    }
}

// CheckWorker   --------->  WaitCoordInit
//                              |
//                              v
// CheckWorker   <---------  Ready
//    |
//    v
// Ready
//    | (user)
//    v
// RunWorker     --------->  Ready
//                              |
//                              v
//                           Run
//                              | (self)
//                              v
// RunWorker     <---------  RunningAwaitKill
//    |
//    v
// WorkersRunning
//    | (user)
//    v
// KillWorker    --------->  RunningAwaitKill
//                              |
//                              v
//                           Killing
//                              | (self)
//                              v
// WorkerKilled  <---------  Stopped
//    |
//    v
// Done          --------->  Stopped
//                              |
//                              v
//                           Done
pub mod server {
    pub use super::server_coord::CoordProtocol;
    // clippy complains about unused import since its used by the russula_cli bin
    #[allow(unused_imports)]
    pub use super::server_worker::WorkerProtocol;
}

// CheckWorker   --------->  WaitCoordInit
//                              |
//                              v
// CheckWorker   <---------  Ready
//    |
//    v
// Ready
//    | (user)
//    v
// RunWorker     --------->  Ready
//                              |
//                              v
//                           Run
//                              | (self)
//                              v
// RunWorker     <---------  Running
//    |
//    v
// WorkersRunning ---------> Running
//                              |
//                              v
//                           RunningAwaitComplete
//                              | (self)
//                              v
// WorkersRunning <---------  Stopped
//    |
//    v
// Done          --------->  Stopped
//                              |
//                              v
//                           Done
pub mod client {
    pub use super::client_coord::{CoordProtocol, CoordState};
    // clippy complains about unused import since its used by the russula_cli bin
    #[allow(unused_imports)]
    pub use super::client_worker::{WorkerProtocol, WorkerState};
}
