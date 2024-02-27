// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{
    error::RussulaError,
    event::EventType,
    network_utils,
    network_utils::Msg,
    states::{StateApi, TransitionStep},
    RussulaResult,
};
use crate::russula::event::EventRecorder;
use core::{task::Poll, time::Duration};
use paste::paste;
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::{debug, info};

const NOTIFY_DONE_TIMEOUT: Duration = Duration::from_secs(1);

macro_rules! state_api {
{
    $(#[$meta:meta])*
    $state:ident
} => {paste!{

    $(#[$meta])*
    fn [<$state _state>](&self) -> Self::State;

    $(#[$meta])*
    /// Check if the Instance is at the desired state
    fn [<is_ $state _state>](&self) -> bool {
        let state = self.[<$state _state>]();
        self.state().eq(&state)
    }
}};
}

macro_rules! notify_peer {
{$protocol:ident, $stream:ident} => {
    use crate::russula::network_utils;
    let msg = Msg::new($protocol.state().as_bytes());
    debug!(
        "{} ----> send msg {}",
        $protocol.name(),
        std::str::from_utf8(&msg.data).unwrap()
    );
    network_utils::send_msg($stream, msg).await?;
    $protocol.on_event(EventType::SendMsg);
}
}
pub(crate) use notify_peer;

pub trait Protocol: Clone {
    type State: StateApi;

    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream>;
    async fn run(&mut self, stream: &TcpStream) -> RussulaResult<Option<Msg>>;
    fn name(&self) -> String;
    fn update_peer_state(&mut self, msg: Msg) -> RussulaResult<()>;
    fn state(&self) -> &Self::State;
    fn state_mut(&mut self) -> &mut Self::State;
    fn event_recorder(&mut self) -> &mut EventRecorder;

    // Ready ==============
    state_api!(ready);
    async fn poll_ready(&mut self, stream: &TcpStream) -> RussulaResult<Poll<()>> {
        let state = self.ready_state();
        self.poll_state(stream, &state).await
    }

    // Done ==============
    // state_api!(done);
    fn done_state(&self) -> Self::State;
    async fn poll_done(&mut self, stream: &TcpStream) -> RussulaResult<Poll<()>> {
        let state = self.done_state();
        self.poll_state(stream, &state).await
    }

    /// Done is the only State with TransitionStep::Finished
    fn is_done_state(&self) -> bool {
        // TODO figure out why doesnt this work
        // let state = self.done_state();
        // matches!(self.state(), state)

        matches!(self.state().transition_step(), TransitionStep::Finished)
    }

    // Running ==============
    state_api!(
        /// Should only be called by Coordinators
        worker_running
    );
    /// Check if worker the Instance is Running
    async fn poll_worker_running(&mut self, stream: &TcpStream) -> RussulaResult<Poll<()>> {
        let state = self.worker_running_state();
        self.poll_state(stream, &state).await
    }

    // If the peer is not at the desired state then attempt to make progress
    async fn poll_state(
        &mut self,
        stream: &TcpStream,
        state: &Self::State,
    ) -> RussulaResult<Poll<()>> {
        if !self.state().eq(state) {
            let prev = self.state().clone();
            self.run_current(stream).await?;
            debug!(
                "{} poll_state--------{:?} -> {:?}",
                self.name(),
                prev,
                self.state()
            );
        }

        // Notify the peer that we have reached a terminal state
        if self.is_done_state() {
            tracing::info!("{}", self.event_recorder());

            // Notify 3 time in case of packet loss.. this is best effort
            for _i in 0..3 {
                match self.run_current(stream).await {
                    Ok(_) => (),
                    // We notify the peer of the Done state multiple times. Since the peer could
                    // have killed the connection in the meantime, its better to ignore network
                    // failures
                    Err(RussulaError::NetworkConnectionRefused { dbg: _ })
                    | Err(RussulaError::NetworkBlocked { dbg: _ })
                    | Err(RussulaError::NetworkFail { dbg: _ }) => {
                        debug!("Ignore network failure since coordination is Done.")
                    }
                    Err(err) => return Err(err),
                }
                tokio::time::sleep(NOTIFY_DONE_TIMEOUT).await;
            }
        }

        let poll = if self.state().eq(state) {
            Poll::Ready(())
        } else {
            Poll::Pending
        };
        Ok(poll)
    }

    // run action for the current state and update the peer state
    async fn run_current(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        if let Some(msg) = self.run(stream).await? {
            self.update_peer_state(msg)?;
        }
        Ok(())
    }

    async fn await_next_msg(&mut self, stream: &TcpStream) -> RussulaResult<Option<Msg>> {
        if !matches!(self.state().transition_step(), TransitionStep::AwaitNext(_)) {
            panic!(
                "expected AwaitNext but found: {:?}",
                self.state().transition_step()
            );
        }
        // loop until we receive a transition msg from peer or drain all msg from queue.
        // recv_msg aborts if the read queue is empty
        let mut last_msg = None;
        // Continue to read from stream until:
        // - the msg results in a transition
        // - there is no more data available (drained all messages)
        // - there is a error while reading
        loop {
            match network_utils::recv_msg(stream).await {
                Ok(msg) => {
                    self.on_event(EventType::RecvMsg);
                    debug!(
                        "{} <---- recv msg {}",
                        self.name(),
                        std::str::from_utf8(&msg.data).unwrap()
                    );

                    let should_transition = self.matches_transition_msg(&msg)?;
                    last_msg = Some(msg);
                    if should_transition {
                        self.transition_next(stream).await?;
                        break;
                    }
                }
                Err(RussulaError::NetworkBlocked { dbg: _ }) => {
                    // This might not be extra since a protocol needs to be polled
                    // to make progress
                    //
                    // notify the peer and make progress
                    notify_peer!(self, stream);
                    break;
                }
                Err(err) => return Err(err),
            }
        }

        Ok(last_msg)
    }

    fn matches_transition_msg(&self, recv_msg: &Msg) -> RussulaResult<bool> {
        let state = self.state();
        if let TransitionStep::AwaitNext(expected_msg) = state.transition_step() {
            let should_transition_to_next = expected_msg == recv_msg.as_bytes();
            debug!(
                "{} expect: {} actual: {}",
                self.name(),
                std::str::from_utf8(&expected_msg).unwrap(),
                std::str::from_utf8(&recv_msg.data).unwrap()
            );
            Ok(should_transition_to_next)
        } else {
            Ok(false)
        }
    }

    async fn transition_next(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        let nxt = self.state().next_state();
        info!(
            "{:?} MOVING TO NEXT STATE. {:?} ===> {:?}",
            self.name(),
            self.state(),
            nxt
        );

        *self.state_mut() = nxt;
        notify_peer!(self, stream);
        Ok(())
    }

    async fn transition_self_or_user_driven(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        let state = self.state();
        assert!(
            matches!(state.transition_step(), TransitionStep::SelfDriven)
                || matches!(state.transition_step(), TransitionStep::UserDriven)
        );

        self.transition_next(stream).await
    }

    fn on_event(&mut self, event: EventType) {
        self.event_recorder().process(event);
    }
}
