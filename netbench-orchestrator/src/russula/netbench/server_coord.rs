// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::russula::{
    error::{RussulaError, RussulaResult},
    event::{EventRecorder, EventType},
    netbench::server_worker::WorkerState,
    network_utils::Msg,
    protocol::{notify_peer, Protocol},
    StateApi, TransitionStep,
};
use core::fmt::Debug;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::{debug, info};

#[derive(Copy, Clone, Debug, Serialize, Deserialize)]
pub enum CoordState {
    CheckWorker,
    Ready,
    RunWorker,
    WorkersRunning,
    KillWorker,
    WorkerKilled,
    Done,
}

#[derive(Clone, Debug)]
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
        format!("server-c-{}", 0)
    }

    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream> {
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

    async fn run(&mut self, stream: &TcpStream) -> RussulaResult<Option<Msg>> {
        match self.state_mut() {
            CoordState::CheckWorker => {
                notify_peer!(self, stream);
                self.await_next_msg(stream).await
            }
            CoordState::Ready => {
                self.transition_self_or_user_driven(stream).await?;
                Ok(None)
            }
            CoordState::RunWorker => {
                notify_peer!(self, stream);
                self.await_next_msg(stream).await
            }
            CoordState::WorkersRunning => {
                self.transition_self_or_user_driven(stream).await?;
                Ok(None)
            }
            CoordState::KillWorker => {
                notify_peer!(self, stream);
                self.await_next_msg(stream).await
            }
            CoordState::WorkerKilled => {
                self.transition_self_or_user_driven(stream).await?;
                Ok(None)
            }
            CoordState::Done => {
                notify_peer!(self, stream);
                Ok(None)
            }
        }
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

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn netbench_state() {}
}
