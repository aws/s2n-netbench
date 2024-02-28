// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{
    error::{RussulaError, RussulaResult},
    event::{EventRecorder, EventType},
    netbench::client::WorkerState,
    network_utils::Msg,
    protocol::{notify_peer, Protocol},
    StateApi, TransitionStep,
};
use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::{debug, info};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CoordState {
    CheckWorker,
    Ready,
    RunWorker,
    WorkersRunning,
    Done,
}

#[derive(Debug, Clone)]
pub struct CoordProtocol {
    state: CoordState,
    peer_state: WorkerState,
    event_recorder: EventRecorder,
}

impl CoordProtocol {
    pub fn new() -> Self {
        CoordProtocol {
            state: CoordState::CheckWorker,
            peer_state: WorkerState::WaitCoordInit,
            event_recorder: EventRecorder::default(),
        }
    }
}

impl Protocol for CoordProtocol {
    type State = CoordState;
    fn name(&self) -> String {
        format!("client-c-{}", 0)
    }

    async fn pair_peer(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
        info!("--- Coordinator: attempt to connect on: {}", addr);

        let connect = TcpStream::connect(addr).await.map_err(RussulaError::from)?;
        Ok(connect)
    }

    fn update_peer_state(&mut self, msg: Msg) -> RussulaResult<()> {
        self.peer_state = WorkerState::from_msg(msg)?;
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
        CoordState::Ready
    }

    fn done_state(&self) -> Self::State {
        CoordState::Done
    }

    fn worker_running_state(&self) -> Self::State {
        CoordState::WorkersRunning
    }
    fn event_recorder(&mut self) -> &mut EventRecorder {
        &mut self.event_recorder
    }
}

impl StateApi for CoordState {
    fn transition_step(&self) -> TransitionStep {
        match self {
            CoordState::CheckWorker => TransitionStep::AwaitNext(WorkerState::Ready.as_bytes()),
            CoordState::Ready => TransitionStep::UserDriven,
            CoordState::RunWorker => TransitionStep::AwaitNext(WorkerState::Running(0).as_bytes()),
            CoordState::WorkersRunning => {
                TransitionStep::AwaitNext(WorkerState::Stopped.as_bytes())
            }
            CoordState::Done => TransitionStep::Finished,
        }
    }

    fn next_state(&self) -> Self {
        match self {
            CoordState::CheckWorker => CoordState::Ready,
            CoordState::Ready => CoordState::RunWorker,
            CoordState::RunWorker => CoordState::WorkersRunning,
            CoordState::WorkersRunning => CoordState::Done,
            CoordState::Done => CoordState::Done,
        }
    }
}
