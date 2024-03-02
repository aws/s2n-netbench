// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::ClientContext;
use crate::russula::states::StateApi;
use crate::russula::states::TransitionStep;
use crate::russula::{
    error::{RussulaError, RussulaResult},
    event::EventRecorder,
    netbench::client::CoordState,
    network_utils::Msg,
    workflow::WorkflowTrait,
};
use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use std::{fs::File, net::SocketAddr, process::Command};
use sysinfo::{Pid, PidExt, ProcessExt, SystemExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info, warn};

// Only used when creating a state variant for comparison
const PLACEHOLDER_PID: u32 = 1000;

/// Workflow state machine
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum WorkerState {
    WaitCoordInit,
    Ready,
    Run,
    Running(#[serde(skip)] u32),
    RunningAwaitComplete(#[serde(skip)] u32),
    Stopped,
    Done,
}

/// Worker protocol for the client
#[derive(Clone)]
pub struct WorkerProtocol {
    id: String,
    state: WorkerState,
    peer_state: CoordState,
    netbench_ctx: ClientContext,
    event_recorder: EventRecorder,
}

impl WorkerProtocol {
    pub fn new(id: String, netbench_ctx: ClientContext) -> Self {
        WorkerProtocol {
            id,
            state: WorkerState::WaitCoordInit,
            peer_state: CoordState::CheckWorker,
            netbench_ctx,
            event_recorder: EventRecorder::default(),
        }
    }
}

impl WorkflowTrait for WorkerProtocol {
    type State = WorkerState;

    fn name(&self) -> String {
        format!("client-w-{}", self.id)
    }

    async fn pair_peer(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
        let listener = TcpListener::bind(addr).await.unwrap();
        info!("{} listening on: {}", self.name(), addr);

        let (stream, _local_addr) = listener.accept().await.map_err(RussulaError::from)?;
        info!("{} success connection: {addr}", self.name());

        Ok(stream)
    }

    fn update_peer_state(&mut self, msg: Msg) -> RussulaResult<()> {
        self.peer_state = CoordState::from_msg(msg)?;
        debug!("{} ... peer_state {:?}", self.name(), self.peer_state);

        Ok(())
    }

    fn state(&self) -> &Self::State {
        &self.state
    }

    fn state_mut(&mut self) -> &mut Self::State {
        &mut self.state
    }

    fn ready_state(&self) -> Self::State {
        WorkerState::Ready
    }

    fn done_state(&self) -> Self::State {
        WorkerState::Done
    }

    fn worker_running_state(&self) -> Self::State {
        unimplemented!("Should only be called by Coordinators")
    }

    async fn run(&mut self, stream: &mut TcpStream) -> RussulaResult<Option<Msg>> {
        match self.state_mut() {
            WorkerState::WaitCoordInit => self.await_next_msg(stream).await,
            WorkerState::Ready => {
                self.notify_peer(stream).await?;
                self.await_next_msg(stream).await
            }
            WorkerState::Run => {
                let child = match &self.netbench_ctx.testing {
                    false => {
                        let output_log_file = format!("{}.json", self.name());
                        let output_log_file =
                            File::create(output_log_file).expect("failed to open log");

                        info!("{} run netbench process", self.name());
                        println!("{} run netbench process", self.name());

                        let netbench_path = self.netbench_ctx.netbench_path.to_str().unwrap();
                        let collector = format!("{}/s2n-netbench-collector", netbench_path);
                        let driver = format!("{}/{}", netbench_path, self.netbench_ctx.driver);
                        let scenario = format!("{}/{}", netbench_path, self.netbench_ctx.scenario);

                        let mut cmd = Command::new(collector);
                        for (i, peer_list) in self.netbench_ctx.netbench_servers.iter().enumerate()
                        {
                            let server_idx = format!("SERVER_{}", i);
                            cmd.env(server_idx, peer_list.to_string());
                        }
                        cmd.args([&driver, "--scenario", &scenario])
                            .stdout(output_log_file);
                        println!("{:?}", cmd);
                        debug!("{:?}", cmd);
                        cmd.spawn()
                            .expect("Failed to start netbench client process")
                    }
                    true => {
                        info!("{} run sim_netbench_client", self.name());
                        Command::new("sh")
                            .args(["scripts/sim_netbench_client.sh", &self.name()])
                            .spawn()
                            .expect("Failed to start sim_netbench_client process")
                    }
                };

                let pid = child.id();
                debug!(
                    "{}----------------------------child id {}",
                    self.name(),
                    pid
                );

                *self.state_mut() = WorkerState::Running(pid);
                Ok(None)
            }
            WorkerState::Running(_pid) => {
                self.notify_peer(stream).await?;
                self.await_next_msg(stream).await
            }
            WorkerState::RunningAwaitComplete(pid) => {
                let pid = Pid::from_u32(*pid);
                self.notify_peer(stream).await?;

                // TODO test only loading the process id we care about
                let system = sysinfo::System::new_all();
                let process = system.process(pid);
                if let Some(process) = process {
                    debug!(
                        "process still RUNNING! pid: {} status: {:?} ----------------------------",
                        process.pid(),
                        process.status()
                    );
                    // FIXME somethings is causing the collector to become a Zombie process.
                    //
                    // We can detect the zombie process and continue with Russula shutdown, which
                    // causes the process to be killed. This indicates that Russula is possibly
                    // preventing a clean close of the collector.
                    //
                    // root       54245  Sl ./target/debug/russula_cli --protocol NetbenchClientWorker --port 9000 --peer-list 54.198.168.151:4433
                    // root       54688  Z  [netbench-collec] <defunct>

                    if let sysinfo::ProcessStatus::Zombie = process.status() {
                        warn!(
                            "Process pid: {} is a Zombie.. ignoring and continuing",
                            process.pid()
                        );
                        self.transition_self_or_user_driven(stream).await?;
                    }
                } else {
                    info!(
                        "Process COMPLETED! pid: {} ----------------------------",
                        pid
                    );

                    self.transition_self_or_user_driven(stream).await?;
                }

                Ok(None)
            }
            WorkerState::Stopped => {
                self.notify_peer(stream).await?;
                self.await_next_msg(stream).await
            }
            WorkerState::Done => {
                self.notify_peer(stream).await?;
                Ok(None)
            }
        }
    }

    fn event_recorder(&mut self) -> &mut EventRecorder {
        &mut self.event_recorder
    }
}

/// State APIs for the protocol state
impl StateApi for WorkerState {
    fn transition_step(&self) -> TransitionStep {
        match self {
            WorkerState::WaitCoordInit => {
                TransitionStep::AwaitNext(CoordState::CheckWorker.as_bytes())
            }
            WorkerState::Ready => TransitionStep::AwaitNext(CoordState::RunWorker.as_bytes()),
            WorkerState::Run => TransitionStep::SelfDriven,
            WorkerState::Running(_) => {
                TransitionStep::AwaitNext(CoordState::WorkersRunning.as_bytes())
            }
            WorkerState::RunningAwaitComplete(_) => TransitionStep::SelfDriven,
            WorkerState::Stopped => TransitionStep::AwaitNext(CoordState::Done.as_bytes()),
            WorkerState::Done => TransitionStep::Finished,
        }
    }

    fn next_state(&self) -> Self {
        match self {
            WorkerState::WaitCoordInit => WorkerState::Ready,
            WorkerState::Ready => WorkerState::Run,
            WorkerState::Run => WorkerState::Running(PLACEHOLDER_PID),
            WorkerState::Running(pid) => WorkerState::RunningAwaitComplete(*pid),
            WorkerState::RunningAwaitComplete(_) => WorkerState::Stopped,
            WorkerState::Stopped => WorkerState::Done,
            WorkerState::Done => WorkerState::Done,
        }
    }
}
