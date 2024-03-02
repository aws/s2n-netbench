// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{send_command, Step};
use crate::{
    ec2_utils::PrivIp,
    orchestrator::OrchestratorConfig,
    ssm_utils::{netbench_driver::NetbenchDriverType, STATE},
    OrchError, OrchResult,
};
use aws_sdk_ssm::operation::send_command::SendCommandOutput;
use std::net::SocketAddr;
use tracing::debug;

pub async fn run_russula_worker(
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
    server_ips: Vec<&PrivIp>,
    driver: &NetbenchDriverType,
    config: &OrchestratorConfig,
) -> OrchResult<SendCommandOutput> {
    // assemble the list of server ips into a string
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
        format!("env RUST_LOG=debug ./target/release/russula_cli netbench-client-worker --russula-port {} --driver {} --scenario {} --netbench-servers {netbench_server_addr}",
            STATE.russula_port, driver.driver_name(), config.netbench_scenario_filename());
    debug!("{}", netbench_cmd);

    send_command(
        vec![Step::BuildDriver("".to_string()), Step::BuildRussula],
        Step::RunRussula,
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
    .ok_or(OrchError::Ssm {
        dbg: "failed to start russula worker".to_string(),
    })
}
