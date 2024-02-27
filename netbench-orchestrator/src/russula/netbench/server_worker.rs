// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::ServerContext;
use crate::russula::{
    error::{RussulaError, RussulaResult},
    event::{EventRecorder, EventType},
    netbench::server_coord::CoordState,
    network_utils::Msg,
    protocol::{notify_peer, Protocol},
    StateApi, TransitionStep,
};
use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use std::{fs::File, net::SocketAddr, process::Command};
use sysinfo::{Pid, PidExt, ProcessExt, SystemExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info};

// Only used when creating a state variant
const PLACEHOLDER_PID: u32 = 1000;

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum WorkerState {
    WaitCoordInit,
    Ready,
    Run,
    RunningAwaitKill(#[serde(skip)] u32),
    Killing(#[serde(skip)] u32),
    Stopped,
    Done,
}

#[derive(Clone, Debug)]
pub struct WorkerProtocol {
    id: String,
    state: WorkerState,
    peer_state: CoordState,
    netbench_ctx: ServerContext,
    event_recorder: EventRecorder,
}

impl WorkerProtocol {
    pub fn new(id: String, netbench_ctx: ServerContext) -> Self {
        WorkerProtocol {
            id,
            state: WorkerState::WaitCoordInit,
            peer_state: CoordState::CheckWorker,
            netbench_ctx,
            event_recorder: EventRecorder::default(),
        }
    }
}

impl Protocol for WorkerProtocol {
    type State = WorkerState;

    fn name(&self) -> String {
        format!("server-{}", self.id)
    }

    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
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
        unimplemented!()
    }

    async fn run(&mut self, stream: &TcpStream) -> RussulaResult<Option<Msg>> {
        match self.state_mut() {
            WorkerState::WaitCoordInit => {
                // self.notify_peer(stream).await?;
                self.await_next_msg(stream).await
            }
            WorkerState::Ready => {
                notify_peer!(self, stream);
                self.await_next_msg(stream).await
            }
            WorkerState::Run => {
                let child = match &self.netbench_ctx.testing {
                    false => {
                        let output_log_file = format!("{}.json", self.name());
                        let output_log_file =
                            File::create(output_log_file).expect("failed to open log");

                        // sudo SCENARIO=./target/netbench/connect.json ./target/release/netbench-collector
                        //   ./target/release/netbench-driver-s2n-quic-server
                        info!("{} run task netbench", self.name());
                        println!("{} run task netbench", self.name());

                        let netbench_path = self.netbench_ctx.netbench_path.to_str().unwrap();
                        let collector = format!("{}/s2n-netbench-collector", netbench_path);
                        // driver value ex.: netbench-driver-s2n-quic-server
                        let driver = format!("{}/{}", netbench_path, self.netbench_ctx.driver);
                        let scenario = format!("{}/{}", netbench_path, self.netbench_ctx.scenario);

                        debug!("netbench_port: {}", self.netbench_ctx.netbench_port);

                        let mut cmd = Command::new(collector);
                        cmd.env("PORT", self.netbench_ctx.netbench_port.to_string());
                        // cmd.arg("--disable-bpf");
                        cmd.args([&driver, "--scenario", &scenario])
                            .stdout(output_log_file);
                        println!("{:?}", cmd);
                        debug!("{:?}", cmd);
                        cmd.spawn()
                            .expect("Failed to start netbench server process")
                    }
                    true => {
                        info!("{} run task sim_netbench_server", self.name());
                        Command::new("sh")
                            .args(["scripts/sim_netbench_server.sh", &self.name()])
                            .spawn()
                            .expect("Failed to start echo process")
                    }
                };

                let pid = child.id();
                debug!(
                    "{}----------------------------child id {}",
                    self.name(),
                    pid
                );

                *self.state_mut() = WorkerState::RunningAwaitKill(pid);
                Ok(None)
            }
            WorkerState::RunningAwaitKill(_pid) => {
                notify_peer!(self, stream);
                self.await_next_msg(stream).await
            }
            WorkerState::Killing(pid) => {
                let pid = Pid::from_u32(*pid);
                let mut system = sysinfo::System::new_all();
                if system.refresh_process(pid) {
                    let process = system.process(pid).unwrap();
                    let kill = process.kill();
                    debug!("did KILL pid: {} {}----------------------------", pid, kill);
                }

                self.transition_self_or_user_driven(stream).await?;
                Ok(None)
            }
            WorkerState::Stopped => {
                notify_peer!(self, stream);
                self.await_next_msg(stream).await
            }
            WorkerState::Done => {
                notify_peer!(self, stream);
                Ok(None)
            }
        }
    }

    fn event_recorder(&mut self) -> &mut EventRecorder {
        &mut self.event_recorder
    }
}

impl StateApi for WorkerState {
    fn transition_step(&self) -> TransitionStep {
        match self {
            WorkerState::WaitCoordInit => {
                TransitionStep::AwaitNext(CoordState::CheckWorker.as_bytes())
            }
            WorkerState::Ready => TransitionStep::AwaitNext(CoordState::RunWorker.as_bytes()),
            WorkerState::Run => TransitionStep::SelfDriven,
            WorkerState::RunningAwaitKill(_) => {
                TransitionStep::AwaitNext(CoordState::KillWorker.as_bytes())
            }
            WorkerState::Killing(_) => TransitionStep::SelfDriven,
            WorkerState::Stopped => TransitionStep::AwaitNext(CoordState::Done.as_bytes()),
            WorkerState::Done => TransitionStep::Finished,
        }
    }

    fn next_state(&self) -> Self {
        match self {
            WorkerState::WaitCoordInit => WorkerState::Ready,
            WorkerState::Ready => WorkerState::Run,
            // FIXME error prone
            WorkerState::Run => WorkerState::RunningAwaitKill(PLACEHOLDER_PID),
            WorkerState::RunningAwaitKill(pid) => WorkerState::Killing(*pid),
            WorkerState::Killing(_) => WorkerState::Stopped,
            WorkerState::Stopped => WorkerState::Done,
            WorkerState::Done => WorkerState::Done,
        }
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn netbench_state() {}
}
