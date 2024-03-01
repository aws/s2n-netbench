// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::{InfraDetail, PubIp},
    orchestrator::OrchestratorConfig,
    russula::{
        self,
        netbench::{client, server},
        WorkflowBuilder, WorkflowState,
    },
    ssm_utils,
    ssm_utils::NetbenchDriverType,
    OrchError, OrchResult, STATE,
};
use aws_sdk_ssm::operation::send_command::SendCommandOutput;
use core::time::Duration;
use indicatif::{ProgressBar, ProgressStyle};
use std::{collections::BTreeSet, net::SocketAddr};
use tracing::{debug, info};

fn get_progress_bar(msg: String) -> ProgressBar {
    let bar = ProgressBar::new(0);
    let style = ProgressStyle::with_template("{spinner} [{elapsed_precise}] {msg}")
        .unwrap()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ");
    bar.set_style(style);
    bar.enable_steady_tick(Duration::from_secs(1));
    bar.set_message(msg);
    bar
}

pub struct ServerNetbenchRussula {
    // used to poll the remote worker via ssm
    worker: SendCommandOutput,
    coord: russula::Workflow<server::CoordWorkflow>,
    driver_name: String,
}

impl ServerNetbenchRussula {
    pub async fn new(
        ssm_client: &aws_sdk_ssm::Client,
        infra: &InfraDetail,
        scenario: &OrchestratorConfig,
        driver: &NetbenchDriverType,
    ) -> OrchResult<Self> {
        debug!("starting server worker");
        let instance_ids = infra.server_ids();
        let worker =
            ssm_utils::server::run_russula_worker(ssm_client, instance_ids, driver, scenario)
                .await?;
        // wait for worker to start
        tokio::time::sleep(STATE.poll_delay_ssm).await;

        // server coord
        debug!("starting server coordinator");
        let coord = server_coord(infra.public_server_ips()).await?;
        Ok(ServerNetbenchRussula {
            worker,
            coord,
            driver_name: driver.trim_driver_name(),
        })
    }

    // Poll till netbench is running on the server hosts.
    pub async fn wait_netbench_running(
        &mut self,
        ssm_client: &aws_sdk_ssm::Client,
    ) -> OrchResult<()> {
        let msg = format!("{}: Waiting for server state Running.", self.driver_name);
        let bar = get_progress_bar(msg);
        let cmd_id = self.worker.command().unwrap().command_id().unwrap();

        loop {
            let poll_worker = ssm_utils::poll_ssm_results("server", ssm_client, cmd_id).await?;
            let poll_coord_worker_running = self
                .coord
                .poll_state(WorkflowState::WorkerRunning)
                .await
                .map_err(|err| OrchError::Russula {
                    dbg: err.to_string(),
                })?;
            debug!(
                "Server Russula!: poll worker_running. Coordinator: {:?} Worker {:?}",
                poll_coord_worker_running, poll_worker
            );

            if poll_coord_worker_running.is_ready() {
                break;
            }
            tokio::time::sleep(STATE.poll_delay_ssm).await;
        }
        bar.finish();

        Ok(())
    }

    // Continue to poll the server worker and coordinator till it is done
    pub async fn wait_done(&mut self, ssm_client: &aws_sdk_ssm::Client) -> OrchResult<()> {
        let msg = format!("{}: Waiting for server state Done.", self.driver_name);
        let bar = get_progress_bar(msg);
        let cmd_id = self.worker.command().unwrap().command_id().unwrap();

        loop {
            let poll_worker = ssm_utils::poll_ssm_results("server", ssm_client, cmd_id).await?;
            let poll_coord_done =
                self.coord
                    .poll_state(WorkflowState::Done)
                    .await
                    .map_err(|err| OrchError::Russula {
                        dbg: err.to_string(),
                    })?;
            debug!(
                "Server Russula!: Coordinator: {:?} Worker {:?}",
                poll_coord_done, poll_worker
            );

            // Since the workers are executed via SSM, there is a delay in detecting
            // when they finish. In practice it's not absolutely necessary to wait
            // for the workers to finish.
            //
            // ```
            // // wait for both coordinator and workers to finish
            // poll_coord_done.is_ready() && poll_worker.is_ready()
            // ```
            if poll_coord_done.is_ready() {
                break;
            }
            tokio::time::sleep(STATE.poll_delay_ssm).await;
        }
        bar.finish();

        info!("Server Russula!: Successful");
        Ok(())
    }
}

pub struct ClientNetbenchRussula {
    // used to poll the remote worker via ssm
    worker: SendCommandOutput,
    coord: russula::Workflow<client::CoordWorkflow>,
    driver_name: String,
}

impl ClientNetbenchRussula {
    pub async fn new(
        ssm_client: &aws_sdk_ssm::Client,
        infra: &InfraDetail,
        scenario: &OrchestratorConfig,
        driver: &NetbenchDriverType,
    ) -> OrchResult<Self> {
        let instance_ids = infra.client_ids();
        debug!("starting client worker");
        let worker = ssm_utils::client::run_russula_worker(
            ssm_client,
            instance_ids,
            infra.private_server_ips(),
            driver,
            scenario,
        )
        .await?;

        // wait for worker to start
        tokio::time::sleep(STATE.poll_delay_ssm).await;

        // client coord
        debug!("starting client coordinator");
        let coord = client_coord(infra.public_client_ips()).await?;
        Ok(ClientNetbenchRussula {
            worker,
            coord,
            driver_name: driver.trim_driver_name(),
        })
    }

    // Continue to poll the client worker and coordinator till it is done
    pub async fn wait_done(&mut self, ssm_client: &aws_sdk_ssm::Client) -> OrchResult<()> {
        let msg = format!("{}: Waiting for client state Done.", self.driver_name);
        let bar = get_progress_bar(msg);
        let cmd_id = self.worker.command().unwrap().command_id().unwrap();

        loop {
            let poll_worker = ssm_utils::poll_ssm_results("client", ssm_client, cmd_id).await?;
            let poll_coord = self
                .coord
                .poll_state(WorkflowState::Done)
                .await
                .map_err(|err| OrchError::Russula {
                    dbg: err.to_string(),
                })?;
            debug!(
                "Client Russula!: Coordinator: {:?} Worker {:?}",
                poll_coord, poll_worker
            );

            // Since the workers are executed via SSM, there is a delay in detecting
            // when they finish. In practice it's not absolutely necessary to wait
            // for the workers to finish.
            //
            // ```
            // // wait for both coordinator and workers to finish
            // poll_coord_done.is_ready() && poll_worker.is_ready()
            // ```
            if poll_coord.is_ready() {
                break;
            }
            tokio::time::sleep(STATE.poll_delay_ssm).await;
        }
        bar.finish();

        info!("Client Russula!: Successful");
        Ok(())
    }
}

async fn server_coord(
    server_ips: Vec<&PubIp>,
) -> OrchResult<russula::Workflow<server::CoordWorkflow>> {
    let server_addr: Vec<SocketAddr> = server_ips
        .iter()
        .map(|ip| SocketAddr::new(ip.0, STATE.russula_port))
        .collect();
    let server_coord = WorkflowBuilder::new(
        BTreeSet::from_iter(server_addr),
        server::CoordWorkflow::new(),
        STATE.poll_delay_russula,
    );
    let mut server_coord = server_coord
        .build()
        .await
        .map_err(|err| OrchError::Russula {
            dbg: err.to_string(),
        })?;

    // Attempt to connect to the peer
    server_coord
        .run_till(WorkflowState::Ready)
        .await
        .map_err(|err| OrchError::Russula {
            dbg: err.to_string(),
        })?;

    info!("server coord Ready");
    Ok(server_coord)
}

async fn client_coord(
    client_ips: Vec<&PubIp>,
) -> OrchResult<russula::Workflow<client::CoordWorkflow>> {
    let client_addr: Vec<SocketAddr> = client_ips
        .iter()
        .map(|ip| SocketAddr::new(ip.0, STATE.russula_port))
        .collect();
    let client_coord = WorkflowBuilder::new(
        BTreeSet::from_iter(client_addr),
        client::CoordWorkflow::new(),
        STATE.poll_delay_russula,
    );
    let mut client_coord = client_coord
        .build()
        .await
        .map_err(|err| OrchError::Russula {
            dbg: err.to_string(),
        })?;

    // Attempt to connect to the peer
    client_coord
        .run_till(WorkflowState::Ready)
        .await
        .map_err(|err| OrchError::Russula {
            dbg: err.to_string(),
        })?;

    info!("client coord Ready");
    Ok(client_coord)
}
