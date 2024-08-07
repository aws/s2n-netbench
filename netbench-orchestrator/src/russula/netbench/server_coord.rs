// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{
    error::{RussulaError, RussulaResult},
    event::EventRecorder,
    netbench::server_worker::WorkerState,
    network_utils::Msg,
    states::{StateApi, TransitionStep},
    WorkflowTrait,
};
use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::{debug, info};

/// Workflow state machine
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CoordState {
    CheckWorker,
    Ready,
    RunWorker,
    WorkersRunning,
    KillWorker,
    WorkerKilled,
    Done,
}

/// Coordinator protocol for the server
#[derive(Clone, Debug)]
pub struct CoordWorkflow {
    state: CoordState,
    peer_state: WorkerState,
    event_recorder: EventRecorder,
}

impl CoordWorkflow {
    pub fn new() -> Self {
        CoordWorkflow {
            state: CoordState::CheckWorker,
            peer_state: WorkerState::WaitCoordInit,
            event_recorder: EventRecorder::default(),
        }
    }
}

impl WorkflowTrait for CoordWorkflow {
    type State = CoordState;
    fn name(&self) -> String {
        format!("server-c-{}", 0)
    }

    async fn pair_peer(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
        info!("attempt to connect on: {}", addr);

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

    async fn run(&mut self, stream: &mut TcpStream) -> RussulaResult<Option<Msg>> {
        match self.state_mut() {
            CoordState::CheckWorker => {
                self.notify_peer(stream).await?;
                self.await_next_msg(stream).await
            }
            CoordState::Ready => {
                self.transition_self_or_user_driven(stream).await?;
                Ok(None)
            }
            CoordState::RunWorker => {
                self.notify_peer(stream).await?;
                self.await_next_msg(stream).await
            }
            CoordState::WorkersRunning => {
                self.transition_self_or_user_driven(stream).await?;
                Ok(None)
            }
            CoordState::KillWorker => {
                self.notify_peer(stream).await?;
                self.await_next_msg(stream).await
            }
            CoordState::WorkerKilled => {
                self.transition_self_or_user_driven(stream).await?;
                Ok(None)
            }
            CoordState::Done => {
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
impl StateApi for CoordState {
    fn transition_step(&self) -> TransitionStep {
        match self {
            CoordState::CheckWorker => TransitionStep::AwaitNext(WorkerState::Ready.as_bytes()),
            CoordState::Ready => TransitionStep::UserDriven,
            CoordState::RunWorker => {
                TransitionStep::AwaitNext(WorkerState::RunningAwaitKill(0).as_bytes())
            }
            CoordState::WorkersRunning => TransitionStep::UserDriven,
            CoordState::KillWorker => TransitionStep::AwaitNext(WorkerState::Stopped.as_bytes()),
            CoordState::WorkerKilled => TransitionStep::UserDriven,
            CoordState::Done => TransitionStep::Finished,
        }
    }

    fn next_state(&self) -> Self {
        match self {
            CoordState::CheckWorker => CoordState::Ready,
            CoordState::Ready => CoordState::RunWorker,
            CoordState::RunWorker => CoordState::WorkersRunning,
            CoordState::WorkersRunning => CoordState::KillWorker,
            CoordState::KillWorker => CoordState::WorkerKilled,
            CoordState::WorkerKilled => CoordState::Done,
            CoordState::Done => CoordState::Done,
        }
    }
}
