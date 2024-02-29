// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{send_command, Step};
use crate::{
    orchestrator::{OrchestratorConfig, STATE},
    ssm_utils::netbench_driver::NetbenchDriverType,
};
use aws_sdk_ssm::operation::send_command::SendCommandOutput;
use tracing::debug;

pub async fn run_russula_worker(
    ssm_client: &aws_sdk_ssm::Client,
    instance_ids: Vec<String>,
    driver: &NetbenchDriverType,
    config: &OrchestratorConfig,
) -> SendCommandOutput {
    let netbench_cmd =
        format!("env RUST_LOG=debug ./target/release/russula_cli netbench-server-worker --russula-port {} --driver {} --scenario {} --netbench-port {}",
            STATE.russula_port, driver.driver_name(), config.netbench_scenario_filename(), STATE.netbench_port);
    debug!("{}", netbench_cmd);

    send_command(
        vec![Step::BuildDriver("".to_string()), Step::BuildRussula],
        Step::RunRussula,
        "run_server_russula",
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
