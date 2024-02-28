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

// Send the Done status multiple times to the peer incase there is packet loss.
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

    $(#[$meta])*
    async fn [<poll_ $state>](&mut self, stream: &TcpStream) -> RussulaResult<Poll<()>> {
        self.poll_state(stream, &self.[<$state _state>]()).await
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

    // Protocol specific connect behavior.
    //
    // Worker should connect to Coordinators. Coordinators should accept connections
    // from Workers.
    async fn connect(&self, addr: &SocketAddr) -> RussulaResult<TcpStream>;

    // Run operations for the current state.
    async fn run(&mut self, stream: &TcpStream) -> RussulaResult<Option<Msg>>;

    // Identifies used primarily for debugging.
    fn name(&self) -> String;

    // Retrieve the current state.
    fn state(&self) -> &Self::State;
    fn state_mut(&mut self) -> &mut Self::State;

    // Track events for the current protocol.
    fn event_recorder(&mut self) -> &mut EventRecorder;

    // Track the peers state; used for debugging.
    fn update_peer_state(&mut self, msg: Msg) -> RussulaResult<()>;

    state_api!(ready);
    state_api!(done);
    state_api!(
        /// Should only be called by Coordinators
        worker_running
    );

    // Attempt to make progress until we reach the desired state
    async fn poll_state(
        &mut self,
        stream: &TcpStream,
        desired_state: &Self::State,
    ) -> RussulaResult<Poll<()>> {
        if !self.state().eq(desired_state) {
            let initial_state = self.state().as_bytes();
            self.run_current(stream).await?;

            debug!(
                "{} poll_state--------{:?} -> {:?}",
                self.name(),
                initial_state,
                self.state()
            );
        }

        // Notify the peer that we have reached a terminal state
        //
        // The Done state is special and only notifies the peer of our Done status.
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

        if self.state().eq(desired_state) {
            Ok(Poll::Ready(()))
        } else {
            Ok(Poll::Pending)
        }
    }

    // Run operations for the current state
    async fn run_current(&mut self, stream: &TcpStream) -> RussulaResult<()> {
        if let Some(msg) = self.run(stream).await? {
            self.update_peer_state(msg)?;
        }
        Ok(())
    }

    async fn await_next_msg(&mut self, stream: &TcpStream) -> RussulaResult<Option<Msg>> {
        // Check to ensure correct usage
        if !matches!(self.state().transition_step(), TransitionStep::AwaitNext(_)) {
            panic!(
                "should await_next_msg only if the transition_step is AwaitNext. Actual: {:?}",
                self.state().transition_step()
            );
        }

        // loop until we transition or drain all msg from queue.
        //
        // network_utils::recv_msg aborts if the read queue is empty.
        // Continue to read from stream until:
        // - the msg results in a transition
        // - there is no more data available (drained all messages)
        // - there is a error while reading
        let mut last_msg = None;
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
                Err(err) if !err.is_fatal() => {
                    // notifying the peer here is an optimization since the protocol
                    // should be polled externally and this operation retried.
                    notify_peer!(self, stream);

                    break;
                }
                Err(err) => return Err(err),
            }
        }

        Ok(last_msg)
    }

    // Check if it's possible to transition to the next state.
    //
    // The current transition_step should be AwaitNext and match the received msg.
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

        // notify the peer of the new state
        notify_peer!(self, stream);

        Ok(())
    }

    // Transition to next state triggered by a user input or self triggered event.
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
