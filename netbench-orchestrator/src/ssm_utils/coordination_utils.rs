// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    ec2_utils::InfraDetail,
    orchestrator::OrchestratorConfig,
    poll_ssm_results,
    russula::{
        self,
        netbench::{client, server},
        RussulaBuilder,
    },
    ssm_utils, NetbenchDriverType, PubIp, STATE,
};
use aws_sdk_ssm::operation::send_command::SendCommandOutput;
use core::time::Duration;
use indicatif::{ProgressBar, ProgressStyle};
use std::{collections::BTreeSet, net::SocketAddr};
use tracing::{debug, info};

fn get_progress_bar(msg: String) -> ProgressBar {
    // TODO use multi-progress bar https://github.com/console-rs/indicatif/blob/main/examples/multi.rs
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
    worker: SendCommandOutput,
    coord: russula::Russula<server::CoordProtocol>,
    driver_name: String,
}

impl ServerNetbenchRussula {
    pub async fn new(
        ssm_client: &aws_sdk_ssm::Client,
        infra: &InfraDetail,
        instance_ids: Vec<String>,
        scenario: &OrchestratorConfig,
        driver: &NetbenchDriverType,
    ) -> Self {
        // server run commands
        debug!("starting server worker");

        let worker =
            ssm_utils::server::run_russula_worker(ssm_client, instance_ids, driver, scenario).await;

        // wait for worker to start
        tokio::time::sleep(Duration::from_secs(5)).await;

        // server coord
        debug!("starting server coordinator");
        let coord = server_coord(infra.public_server_ips()).await;
        ServerNetbenchRussula {
            worker,
            coord,
            driver_name: driver.trim_driver_name(),
        }
    }

    pub async fn wait_workers_running(&mut self, ssm_client: &aws_sdk_ssm::Client) {
        let msg = format!("{}: Waiting for server state Running.", self.driver_name);
        let bar = get_progress_bar(msg);
        loop {
            let poll_worker = poll_ssm_results(
                "server",
                ssm_client,
                self.worker.command().unwrap().command_id().unwrap(),
            )
            .await
            .unwrap();

            let poll_coord_worker_running = self.coord.poll_worker_running().await.unwrap();

            debug!(
                "Server Russula!: poll worker_running. Coordinator: {:?} Worker {:?}",
                poll_coord_worker_running, poll_worker
            );

            if poll_coord_worker_running.is_ready() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        bar.finish();
    }

    pub async fn wait_done(&mut self, ssm_client: &aws_sdk_ssm::Client) {
        let msg = format!("{}: Waiting for server state Done.", self.driver_name);
        let bar = get_progress_bar(msg);
        // poll server russula workers/coord
        loop {
            let poll_worker = poll_ssm_results(
                "server",
                ssm_client,
                self.worker.command().unwrap().command_id().unwrap(),
            )
            .await
            .unwrap();

            let poll_coord_done = self.coord.poll_done().await.unwrap();

            debug!(
                "Server Russula!: Coordinator: {:?} Worker {:?}",
                poll_coord_done, poll_worker
            );

            // FIXME the worker doesnt complete but its not necessary to wait so continue.
            //
            // maybe try sudo
            //
            // The collector launches the driver process, which doesnt get killed when the
            // collector is killed. However its not necessary to wait for its completing
            // for the purpose of a single run.
            // ```
            //  55320  ./target/debug/russula_cli
            //  55646  /home/ec2-user/bin/netbench-collector
            //  55647  /home/ec2-user/bin/netbench-driver-s2n-quic-server
            // ```
            if poll_coord_done.is_ready() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        bar.finish();

        info!("Server Russula!: Successful");
    }
}

pub struct ClientNetbenchRussula {
    worker: SendCommandOutput,
    coord: russula::Russula<client::CoordProtocol>,
    driver_name: String,
}

impl ClientNetbenchRussula {
    pub async fn new(
        ssm_client: &aws_sdk_ssm::Client,
        infra: &InfraDetail,
        instance_ids: Vec<String>,
        scenario: &OrchestratorConfig,
        driver: &NetbenchDriverType,
    ) -> Self {
        // client run commands
        debug!("starting client worker");
        let worker = ssm_utils::client::run_russula_worker(
            ssm_client,
            instance_ids,
            infra.private_server_ips(),
            driver,
            scenario,
        )
        .await;

        // wait for worker to start
        tokio::time::sleep(Duration::from_secs(5)).await;

        // client coord
        debug!("starting client coordinator");
        let coord = client_coord(infra.public_client_ips()).await;
        ClientNetbenchRussula {
            worker,
            coord,
            driver_name: driver.trim_driver_name(),
        }
    }

    pub async fn wait_done(&mut self, ssm_client: &aws_sdk_ssm::Client) {
        let msg = format!("{}: Waiting for client state Done.", self.driver_name);
        let bar = get_progress_bar(msg);
        // poll client russula workers/coord
        loop {
            let poll_worker = poll_ssm_results(
                "client",
                ssm_client,
                self.worker.command().unwrap().command_id().unwrap(),
            )
            .await
            .unwrap();

            let poll_coord_done = self.coord.poll_done().await.unwrap();

            debug!(
                "Client Russula!: Coordinator: {:?} Worker {:?}",
                poll_coord_done, poll_worker
            );

            if poll_coord_done.is_ready() {
                // if poll_coord_done.is_ready() && poll_worker.is_ready() {
                break;
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        bar.finish();

        info!("Client Russula!: Successful");
    }
}

async fn server_coord(server_ips: Vec<&PubIp>) -> russula::Russula<server::CoordProtocol> {
    let protocol = server::CoordProtocol::new();
    let server_addr: Vec<SocketAddr> = server_ips
        .iter()
        .map(|ip| SocketAddr::new(ip.0, STATE.russula_port))
        .collect();
    let server_coord = RussulaBuilder::new(
        BTreeSet::from_iter(server_addr),
        protocol,
        STATE.poll_delay_russula,
    );
    let mut server_coord = server_coord.build().await.unwrap();
    server_coord.run_till_ready().await.unwrap();
    info!("server coord Ready");
    server_coord
}

async fn client_coord(client_ips: Vec<&PubIp>) -> russula::Russula<client::CoordProtocol> {
    let protocol = client::CoordProtocol::new();
    let client_addr: Vec<SocketAddr> = client_ips
        .iter()
        .map(|ip| SocketAddr::new(ip.0, STATE.russula_port))
        .collect();
    let client_coord = RussulaBuilder::new(
        BTreeSet::from_iter(client_addr),
        protocol,
        STATE.poll_delay_russula,
    );
    let mut client_coord = client_coord.build().await.unwrap();
    client_coord.run_till_ready().await.unwrap();
    info!("client coord Ready");
    client_coord
}
