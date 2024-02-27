// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::STATE;
use std::path::PathBuf;

pub mod native_tls_driver;
pub mod s2n_quic_dc_driver;
pub mod s2n_quic_driver_crates;
pub mod s2n_quic_driver_github;
pub mod s2n_tls_driver;
pub mod tcp_driver_crates;
pub mod tcp_driver_github;

pub enum NetbenchDriverType {
    GithubRustProj(GithubSource),
    CratesIo(CrateIoSource),
    Local(LocalSource),
}

pub struct GithubSource {
    pub driver_name: String,
    pub repo_name: String,
}

pub struct LocalSource {
    pub driver_name: String,
    pub ssm_build_cmd: Vec<String>,
    pub proj_name: String,
    // Used to copy local driver source to hosts
    //
    // upload to s3 locally and download form s3 in ssm_build_cmd
    local_path_to_proj: PathBuf,
}

pub struct CrateIoSource {
    pub krate: String,
    pub driver_name: String,
    version: String,
}

impl NetbenchDriverType {
    pub fn driver_name(&self) -> &String {
        match self {
            NetbenchDriverType::GithubRustProj(source) => &source.driver_name,
            NetbenchDriverType::Local(source) => &source.driver_name,
            NetbenchDriverType::CratesIo(source) => &source.driver_name,
        }
    }

    pub fn trim_driver_name(&self) -> String {
        self.driver_name()
            .trim_start_matches("s2n-netbench-driver-")
            .trim_start_matches("netbench-driver-")
            .trim_end_matches(".json")
            .to_owned()
    }

    pub fn ssm_build_cmd(&self) -> Vec<String> {
        let build_cmd = match self {
            NetbenchDriverType::GithubRustProj(source) => source.ssm_build_rust_proj(),
            NetbenchDriverType::Local(source) => source.ssm_build_cmd.clone(),
            NetbenchDriverType::CratesIo(source) => source.ssm_build_crates_io_proj(),
        };
        self.ssm_build_collector()
            .into_iter()
            .chain(build_cmd)
            .collect()
    }

    pub fn ssm_build_collector(&self) -> Vec<String> {
        vec![
            format!(
                "runuser -u ec2-user -- env CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse {} install s2n-netbench-collector",
                STATE.cargo_path(),
            ),
            format!(
                "ln -s /home/ec2-user/.cargo/bin/s2n-netbench-collector {}/s2n-netbench-collector",
                STATE.host_bin_path(),
            )
        ]
    }
}

impl GithubSource {
    pub fn ssm_build_rust_proj(&self) -> Vec<String> {
        vec![
            format!(
                "git clone --branch {} {}",
                STATE.netbench_branch, STATE.netbench_repo
            ),
            format!("cd {}", self.repo_name),
            format!(
                "env CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse {} build --release",
                STATE.cargo_path()
            ),
            // copy netbench executables to ~/bin folder
            format!(
                "find target/release -maxdepth 1 -type f -perm /a+x -exec cp {{}} {} \\;",
                STATE.host_bin_path()
            ),
        ]
    }
}

impl CrateIoSource {
    pub fn ssm_build_crates_io_proj(&self) -> Vec<String> {
        vec![
            format!(
                // "runuser -u ec2-user -- ./.cargo/bin/rustup update".to_string(),
                "runuser -u ec2-user -- env CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse {} install {}",
                STATE.cargo_path(),
                self.krate,
                // self.version
            ),
            // link this from /bin folder
            format!(
                "ln -s /home/ec2-user/.cargo/bin/{} {}/{}",
                self.driver_name,
                STATE.host_bin_path(),
                self.driver_name,
            ),
        ]
    }
}
