// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{send_command, Step};
use crate::{orchestrator::OrchestratorConfig, NetbenchDriverType, PrivIp, STATE};
use aws_sdk_ssm::operation::send_command::SendCommandOutput;
use std::net::SocketAddr;
use tracing::{debug, info};

pub async fn upload_netbench_data(
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
    unique_id: &str,
    config: &OrchestratorConfig,
    driver: &NetbenchDriverType,
) -> SendCommandOutput {
    let driver_name = driver.trim_driver_name();
    let s3_command = format!(
        "aws s3 cp *{driver_name}.json {}/results/{}/{driver_name}/",
        STATE.s3_path(unique_id, config),
        config.netbench_scenario_file_stem()
    );
    let cmd = vec!["cd netbench_orchestrator", s3_command.as_str()];
    info!("Copying client results to s3 for driver: {:?}", cmd);
    send_command(
        vec![Step::RunRussula],
        Step::UploadNetbenchRawData,
        "client",
        "upload_netbench_raw_data",
        ssm_client,
        instance_ids,
        cmd.into_iter().map(String::from).collect(),
        config,
    )
    .await
    .expect("Timed out")
}

pub async fn run_russula_worker(
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
    server_ips: Vec<&PrivIp>,
    driver: &NetbenchDriverType,
    config: &OrchestratorConfig,
) -> SendCommandOutput {
    let netbench_server_addr = server_ips
        .iter()
        .map(|ip| SocketAddr::new(ip.0, STATE.netbench_port).to_string())
        .reduce(|mut accum, item| {
            accum.push(' ');
            accum.push_str(&item);
            accum
        })
        .unwrap();

    let netbench_cmd =
        format!("env RUST_LOG=debug ./target/debug/russula_cli netbench-client-worker --russula-port {} --driver {} --scenario {} --netbench-servers {netbench_server_addr}",
            STATE.russula_port, driver.driver_name(), config.netbench_scenario_filename);
    debug!("{}", netbench_cmd);

    send_command(
        vec![Step::BuildDriver("".to_string()), Step::BuildRussula],
        Step::RunRussula,
        "client",
        "run_client_russula",
        ssm_client,
        instance_ids,
        vec!["cd netbench_orchestrator", netbench_cmd.as_str()]
            .into_iter()
            .map(String::from)
            .collect(),
        config,
    )
    .await
    .expect("Timed out")
}
