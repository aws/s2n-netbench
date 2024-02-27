// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::orchestrator::{OrchError, OrchResult, OrchestratorConfig, STATE};
use aws_sdk_ssm::{
    operation::send_command::SendCommandOutput,
    types::{CloudWatchOutputConfig, CommandInvocationStatus},
};
use core::{task::Poll, time::Duration};
use tracing::{error, trace};

pub mod client;
pub mod common;
pub mod coordination_utils;
mod netbench_driver;
pub mod server;

pub use netbench_driver::*;

pub enum Step {
    UploadScenarioFile,
    Configure,
    BuildDriver(String),
    BuildRussula,
    RunRussula,
    RunNetbench,
    UploadNetbenchRawData,
}

impl Step {
    fn as_str(&self) -> &str {
        match self {
            Step::UploadScenarioFile => "upload_scenario_file",
            Step::Configure => "configure",
            Step::BuildDriver(_driver_name) => "build_driver",
            Step::BuildRussula => "build_russula",
            Step::RunRussula => "run_russula",
            Step::RunNetbench => "run_netbench",
            Step::UploadNetbenchRawData => "upload_netbench_raw_data",
        }
    }

    fn task_detail(&self) -> Option<&str> {
        match self {
            Step::UploadScenarioFile => None,
            Step::Configure => None,
            Step::BuildDriver(driver_name) => Some(driver_name),
            Step::BuildRussula => None,
            Step::RunRussula => None,
            Step::RunNetbench => None,
            Step::UploadNetbenchRawData => None,
        }
    }
}

pub async fn send_command(
    wait_steps: Vec<Step>,
    step: Step,
    endpoint: &str,
    comment: &str,
    ssm_client: &aws_sdk_ssm::Client,
    ids: Vec<String>,
    commands: Vec<String>,
    config: &OrchestratorConfig,
) -> Option<SendCommandOutput> {
    let command = {
        // SSM doesnt have a concept of order. However, we would still
        // like to execute commands in parallel. To achieve this we
        // create files based on the [`Step`] name and poll till the
        // previous steps has finished.
        //
        // For example, the Step::RunRussula step waits for the
        // Step::BuildRussula and Step::BuildDriver steps to finish.
        let mut assemble_command = Vec::new();

        // Insert at beginning of user provided commands
        //
        // FIXME: use `for entry in ./start_build_driver*; do echo "$entry"; done`
        // this doesnt work if more than one task share the same step. Multiple BuildDriver
        // for example. Instead wait for ALL sub-tasks to finish: `for {}_*_start; wait `.
        // This is not an issue now since the driver build and russula run are not run in
        // parallel.
        for step in wait_steps {
            // wait for previous steps
            assemble_command.push(format!(
                "cd /home/ec2-user; until [ -f fin_{}___ ]; do sleep 5; done",
                step.as_str()
            ));
        }
        // indicate that this step has started
        assemble_command.push(format!(
            "cd /home/ec2-user; touch start_{}___",
            step.as_str()
        ));
        if let Some(detail) = step.task_detail() {
            assemble_command.push(format!(
                "cd /home/ec2-user; touch start_{}_{}___",
                step.as_str(),
                detail
            ));
        }
        assemble_command.extend(commands);

        // Insert at end of user provided commands
        // indicate that this step has finished.
        assemble_command.extend(vec![
            "cd /home/ec2-user".to_string(),
            format!("mv start_{}___ fin_{}___", step.as_str(), step.as_str()),
        ]);
        if let Some(detail) = step.task_detail() {
            assemble_command.push(format!(
                "cd /home/ec2-user; mv start_{}_{}___ fin_{}_{}___",
                step.as_str(),
                detail,
                step.as_str(),
                detail
            ));
        }

        trace!("{} {:?}", endpoint, assemble_command);
        assemble_command
    };

    let mut remaining_try_count: u32 = 10;
    loop {
        match ssm_client
            .send_command()
            .comment(comment)
            // .instance_ids(ids)
            .set_instance_ids(Some(ids.clone()))
            .document_name("AWS-RunShellScript")
            .document_version("$LATEST")
            .parameters("commands", command.clone())
            .cloud_watch_output_config(
                CloudWatchOutputConfig::builder()
                    .cloud_watch_log_group_name(config.cdk_config.netbench_runner_log_group())
                    .cloud_watch_output_enabled(true)
                    .build(),
            )
            .send()
            .await
            .map_err(|x| format!("{:#?}", x))
        {
            Ok(sent_command) => {
                break Some(sent_command);
            }
            Err(err) => {
                if remaining_try_count > 0 {
                    trace!("Send command failed: remaining: {remaining_try_count} err: {err}",);
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    remaining_try_count -= 1;
                    continue;
                } else {
                    error!("Send command failed: err: {err}",);
                    return None;
                }
            }
        };
    }
}

pub(crate) async fn wait_for_ssm_results(
    endpoint: &str,
    ssm_client: &aws_sdk_ssm::Client,
    command_id: &str,
) -> bool {
    loop {
        match poll_ssm_results(endpoint, ssm_client, command_id).await {
            Ok(Poll::Ready(_)) => break true,
            Ok(Poll::Pending) => {
                tokio::time::sleep(STATE.poll_delay_ssm).await;
                continue;
            }
            Err(_err) => break false,
        }
    }
}

pub(crate) async fn poll_ssm_results(
    endpoint: &str,
    ssm_client: &aws_sdk_ssm::Client,
    command_id: &str,
) -> OrchResult<Poll<()>> {
    let status_comment = ssm_client
        .list_command_invocations()
        .command_id(command_id)
        .send()
        .await
        .unwrap()
        .command_invocations()
        .unwrap()
        .iter()
        .find_map(|command| {
            let status = command.status().cloned();
            let comment = command.comment().map(|s| s.to_string());
            status.zip(comment)
        });
    let status = match status_comment {
        Some((status, _comment)) => {
            // debug!(
            //     "endpoint: {} status: {:?}  comment {}",
            //     endpoint, status, comment
            // );

            status
        }
        None => {
            return Ok(Poll::Ready(()));
        }
    };
    trace!("endpoint: {}  command_id {}", endpoint, command_id);

    let status = match status {
        CommandInvocationStatus::Cancelled
        | CommandInvocationStatus::Cancelling
        | CommandInvocationStatus::Failed
        | CommandInvocationStatus::TimedOut => {
            return Err(OrchError::Ssm {
                dbg: "timeout".to_string(),
            })
        }
        CommandInvocationStatus::Delayed
        | CommandInvocationStatus::InProgress
        | CommandInvocationStatus::Pending => Poll::Pending,
        CommandInvocationStatus::Success => Poll::Ready(()),
        _ => {
            return Err(OrchError::Ssm {
                dbg: "unhandled status".to_string(),
            })
        }
    };
    Ok(status)
}
