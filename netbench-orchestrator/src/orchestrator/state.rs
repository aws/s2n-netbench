// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use core::time::Duration;

pub const STATE: State = State {
    version: "v1.0.0",

    // netbench
    netbench_repo: "https://github.com/aws/s2n-netbench.git",
    netbench_branch: "main",
    netbench_port: 4433,

    // orchestrator
    host_home_path: "/home/ec2-user",
    workspace_dir: "./target/netbench",
    shutdown_min: 120, // 1 hour
    poll_delay_ssm: Duration::from_secs(10),

    // russula
    russula_repo: "https://github.com/toidiu/netbench_orchestrator.git",
    russula_branch: "ak-main",
    russula_port: 9000,
    poll_delay_russula: Duration::from_secs(5),

    // aws
    ami_name: "/aws/service/ami-amazon-linux-latest/al2023-ami-kernel-default-x86_64",
    // https://github.com/aws/s2n-netbench/issues/35
    // set a key pair to access the ec2 hosts
    ssh_key_name: None,
};

pub struct State {
    pub version: &'static str,

    // netbench
    pub netbench_repo: &'static str,
    pub netbench_branch: &'static str,
    pub netbench_port: u16,

    // orchestrator
    pub host_home_path: &'static str,
    pub workspace_dir: &'static str,
    pub shutdown_min: u16,
    pub poll_delay_ssm: Duration,

    // russula
    pub russula_repo: &'static str,
    pub russula_branch: &'static str,
    pub russula_port: u16,
    pub poll_delay_russula: Duration,

    // aws
    pub ami_name: &'static str,
    pub ssh_key_name: Option<&'static str>,
}

impl State {
    // All executables should be placed in the bin path.
    //
    // Since drivers and executables can be installed from multiple sources (Github, Source,
    // rustup), we link all executables from a common bin folder. This makes executable
    // discovery trivial and also helps with debugging.
    pub fn host_bin_path(&self) -> String {
        format!("{}/bin", self.host_home_path)
    }

    pub fn cargo_path(&self) -> String {
        format!("{}/cargo", self.host_bin_path())
    }

    pub fn security_group_name(&self, unique_id: &str) -> String {
        format!("netbench_{}", unique_id)
    }
}
