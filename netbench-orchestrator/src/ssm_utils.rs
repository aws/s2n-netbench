// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::orchestrator::{OrchError, OrchResult, OrchestratorConfig, STATE};
use aws_sdk_ssm::{
    operation::send_command::SendCommandOutput,
    types::{CloudWatchOutputConfig, CommandInvocationStatus},
};
use core::task::Poll;
use tracing::trace;

pub mod client;
pub mod common;
mod coordination_utils;
pub mod netbench_driver;
pub mod server;

pub use coordination_utils::{ClientNetbenchRussula, ServerNetbenchRussula};
pub use netbench_driver::*;

// Group of SSM commands
//
// SSM executes commands asynchronously on remote hosts, and doesn't have
// a concept of order.
// To work around this limitation we create files based on the [`Step`]
// name and poll till the previous steps has finished.
//
// For example, the Step::RunRussula step waits for the Step::BuildRussula
// and Step::BuildDriver steps to finish.
pub enum Step {
    UploadScenarioFile,
    Configure,
    // We currently don't differentiate between driver builds. This
    // is ok since all driver commands are idempotent and don't depend
    // on one another.
    BuildDriver(String),
    BuildRussula,
    RunRussula,
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
            Step::UploadNetbenchRawData => None,
        }
    }
}

pub async fn send_command(
    // Steps to wait for before running the current Step
    wait_steps: Vec<Step>,
    // The current Step identifier
    curr_step: Step,
    // String useful for displaying and debugging
    comment: &str,
    // sm sdk client
    ssm_client: &aws_sdk_ssm::Client,
    // EC2 instance Ids
    ids: Vec<String>,
    // The ssm commands to execute
    commands: Vec<String>,
    // Orchestrator config object for this run
    config: &OrchestratorConfig,
) -> Option<SendCommandOutput> {
    // SSM executes commands asynchronously on remote hosts, and doesn't have
    // a concept of order.
    // To work around this limitation we create files based on the [`Step`]
    // name and poll till the previous steps has finished.
    //
    // For example, the Step::RunRussula step waits for the Step::BuildRussula
    // and Step::BuildDriver steps to finish.
    let command = {
        let mut assemble_command = wait_previous_step(wait_steps);
        indicate_curr_step_started(&mut assemble_command, &curr_step);

        // execute current step operations
        assemble_command.extend(commands);

        indicate_curr_step_finished(&mut assemble_command, &curr_step);
        trace!("{:?}", assemble_command);

        assemble_command
    };

    send_and_wait_ssm_command(comment, ssm_client, ids, command, config).await
}

async fn send_and_wait_ssm_command(
    comment: &str,
    ssm_client: &aws_sdk_ssm::Client,
    ids: Vec<String>,
    command: Vec<String>,
    config: &OrchestratorConfig,
) -> Option<SendCommandOutput> {
    let mut remaining_try_count: u32 = 5;
    while remaining_try_count > 0 {
        let send = ssm_client
            .send_command()
            .comment(comment)
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
            .map_err(|x| format!("{:#?}", x));

        match send {
            Ok(sent_command) => {
                return Some(sent_command);
            }
            Err(err) => {
                trace!("Send command failed: remaining: {remaining_try_count} err: {err}",);
                remaining_try_count -= 1;

                tokio::time::sleep(STATE.poll_delay_ssm).await;
            }
        };
    }

    None
}

fn wait_previous_step(wait_steps: Vec<Step>) -> Vec<String> {
    let mut assemble_command = Vec::new();
    // Insert at beginning of user provided commands
    for step in wait_steps {
        // wait for previous steps
        assemble_command.push(format!(
            "cd /home/ec2-user; until [ -f fin_{}___ ]; do sleep 5; done",
            step.as_str()
        ));
    }

    assemble_command
}

fn indicate_curr_step_started(assemble_command: &mut Vec<String>, curr_step: &Step) {
    // indicate that the current step has started
    assemble_command.push(format!(
        "cd /home/ec2-user; touch start_{}___",
        curr_step.as_str()
    ));
    if let Some(detail) = curr_step.task_detail() {
        assemble_command.push(format!(
            "cd /home/ec2-user; touch start_{}_{}___",
            curr_step.as_str(),
            detail
        ));
    }
}

fn indicate_curr_step_finished(assemble_command: &mut Vec<String>, curr_step: &Step) {
    // Insert at end of user provided commands
    // indicate that this step has finished.
    assemble_command.extend(vec![
        "cd /home/ec2-user".to_string(),
        format!(
            "mv start_{}___ fin_{}___",
            curr_step.as_str(),
            curr_step.as_str()
        ),
    ]);
    if let Some(detail) = curr_step.task_detail() {
        assemble_command.push(format!(
            "cd /home/ec2-user; mv start_{}_{}___ fin_{}_{}___",
            curr_step.as_str(),
            detail,
            curr_step.as_str(),
            detail
        ));
    }
}

async fn poll_ssm_results(
    endpoint: &str,
    ssm_client: &aws_sdk_ssm::Client,
    command_id: &str,
) -> OrchResult<Poll<()>> {
    let status_comment = ssm_client
        .list_command_invocations()
        .command_id(command_id)
        .send()
        .await
        .map_err(|err| OrchError::Ssm {
            dbg: format!("error listing ssm command {err}"),
        })?
        .command_invocations()
        .iter()
        .find_map(|command| {
            let status = command.status().cloned();
            let comment = command.comment().map(|s| s.to_string());
            status.zip(comment)
        });
    let (status, comment) = match status_comment {
        Some((status, comment)) => (status, comment),
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
                dbg: format!("ssm command timeout {}", comment),
            })
        }
        CommandInvocationStatus::Delayed
        | CommandInvocationStatus::InProgress
        | CommandInvocationStatus::Pending => Poll::Pending,
        CommandInvocationStatus::Success => Poll::Ready(()),
        _ => {
            return Err(OrchError::Ssm {
                dbg: format!("error polling ssm command {}", comment),
            })
        }
    };
    Ok(status)
}
