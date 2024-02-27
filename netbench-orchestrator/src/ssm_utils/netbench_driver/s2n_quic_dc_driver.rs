// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{LocalSource, NetbenchDriverType};
use crate::{ssm_utils::OrchestratorConfig, STATE};
use std::{
    path::PathBuf,
    process::{Command, Stdio},
};
use tracing::debug;

pub fn dc_quic_server_driver(unique_id: &str, config: &OrchestratorConfig) -> NetbenchDriverType {
    let proj_name = "SaltyLib-Rust".to_string();

    let driver = LocalSource {
        driver_name: "s2n-netbench-driver-server-s2n-quic-dc".to_string(),
        ssm_build_cmd: vec![
            // copy s3 to host: `aws s3 sync s3://netbenchrunnerlogs-source/2024-01-09T05:25:30Z-v2.0.1//SaltyLib-Rust/ /home/ec2-user/SaltyLib-Rust`
            format!(
                "aws s3 sync {}/{proj_name}/ {}/{proj_name}",
                STATE.s3_private_path(unique_id, config),
                STATE.host_home_path,
            ),
            format!("cd {}", proj_name),
            // SSM agent doesn't pick up the newest rustc version installed via rustup`
            // so instead refer to it directly
            format!(
                // "env CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse RUSTFLAGS='--cfg s2n_quic_unstable' {} build",
                "env CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse RUSTFLAGS='--cfg s2n_quic_unstable' {} build --release",
                STATE.cargo_path()
            ),
            // copy executables to bin directory
            format!(
                // "find target/debug -maxdepth 1 -type f -perm /a+x -exec cp {{}} {} \\;",
                "find target/release -maxdepth 1 -type f -perm /a+x -exec cp {{}} {} \\;",
                STATE.host_bin_path()
            ),
        ],
        proj_name: proj_name.clone(),
        local_path_to_proj: "/Users/apoorvko/projects/ws_SaltyLib/src".into(),
    };

    local_upload_source_to_s3(
        &driver.local_path_to_proj,
        &driver.proj_name,
        unique_id,
        config,
    );
    NetbenchDriverType::Local(driver)
}

pub fn dc_quic_client_driver(unique_id: &str, config: &OrchestratorConfig) -> NetbenchDriverType {
    let proj_name = "SaltyLib-Rust".to_string();

    let driver = LocalSource {
        driver_name: "s2n-netbench-driver-client-s2n-quic-dc".to_string(),
        ssm_build_cmd: vec![
            // copy s3 to host
            // `aws s3 sync s3://netbenchrunnerlogs/2024-01-09T05:25:30Z-v2.0.1//SaltyLib-Rust/ /home/ec2-user/SaltyLib-Rust`
            format!(
                "aws s3 sync {}/{proj_name}/ {}/{proj_name}",
                STATE.s3_private_path(unique_id, config),
                STATE.host_home_path,
            ),
            format!("cd {}", proj_name),
            // SSM agent doesn't pick up the newest rustc version installed via rustup`
            // so instead refer to it directly
            format!(
                // "env CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse RUSTFLAGS='--cfg s2n_quic_unstable' {} build",
                "env CARGO_REGISTRIES_CRATES_IO_PROTOCOL=sparse RUSTFLAGS='--cfg s2n_quic_unstable' {} build --release",
                STATE.cargo_path()
            ),
            // copy executables to bin directory
            format!(
                // "find target/debug -maxdepth 1 -type f -perm /a+x -exec cp {{}} {} \\;",
                "find target/release -maxdepth 1 -type f -perm /a+x -exec cp {{}} {} \\;",
                STATE.host_bin_path()
            ),
        ],
        proj_name: proj_name.clone(),
        local_path_to_proj: "/Users/apoorvko/projects/ws_SaltyLib/src".into(),
    };

    local_upload_source_to_s3(
        &driver.local_path_to_proj,
        &driver.proj_name,
        unique_id,
        config,
    );
    NetbenchDriverType::Local(driver)
}

// This local command runs twice; once for server and once for client.
// For this reason `aws sync` is preferred over `aws cp` since sync avoids
// object copy if the same copy already exists.
fn local_upload_source_to_s3(
    local_path_to_proj: &PathBuf,
    proj_name: &str,
    unique_id: &str,
    config: &OrchestratorConfig,
) {
    let mut local_to_s3_cmd = Command::new("aws");
    local_to_s3_cmd.args(["s3", "sync"]).stdout(Stdio::null());
    local_to_s3_cmd
        .arg(format!(
            "{}/{}",
            local_path_to_proj.to_str().unwrap(),
            proj_name
        ))
        .arg(format!(
            "{}/{}/",
            STATE.s3_private_path(unique_id, config),
            proj_name
        ));
    local_to_s3_cmd.args(["--exclude", "target/*", "--exclude", ".git/*"]);
    debug!("{:?}", local_to_s3_cmd);
    let status = local_to_s3_cmd.status().unwrap();
    assert!(status.success(), "aws sync command failed");
}
