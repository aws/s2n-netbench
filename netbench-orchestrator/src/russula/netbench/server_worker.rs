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

// Only used when creating a state variant for comparison
const PLACEHOLDER_PID: u32 = 1000;

#[derive(Clone, Debug, Serialize, Deserialize)]
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
        format!("server-w-{}", self.id)
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

