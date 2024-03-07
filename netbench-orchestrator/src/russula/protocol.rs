// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{
    error::RussulaError,
    event::EventType,
    network_utils,
    network_utils::Msg,
    states::{StateApi, TransitionStep},
    ProtocolState, RussulaResult,
};
use crate::russula::event::EventRecorder;
use core::{task::Poll, time::Duration};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tracing::{debug, info};

// Send the Done status multiple times to the peer in case there is packet loss.
const NOTIFY_DONE_TIMEOUT: Duration = Duration::from_secs(1);
// Notify done multiple time in case of packet loss.. this is best effort
const DONE_SENT_COUNT: usize = 3;

pub trait Protocol: Clone {
    type State: StateApi;

    /// Protocol specific pairing behavior.
    ///
    /// Coordinators should connect to Workers. Workers should accept connections
    /// from Coordinators.
    async fn pair_peer(&self, addr: &SocketAddr) -> RussulaResult<TcpStream>;

    /// Run operations for the current state.
    async fn run(&mut self, stream: &mut TcpStream) -> RussulaResult<Option<Msg>>;

    /// Retrieve the current state.
    fn state(&self) -> &Self::State;
    fn state_mut(&mut self) -> &mut Self::State;

    /// Track events for the current protocol.
    fn event_recorder(&mut self) -> &mut EventRecorder;

    /// Used for debugging and creating unique log files.
    fn name(&self) -> String;

    /// Track the peers state. Mainly used for debugging.
    fn update_peer_state(&mut self, msg: Msg) -> RussulaResult<()>;

    fn ready_state(&self) -> Self::State;
    fn done_state(&self) -> Self::State;
    /// Should only be called by Coordinators
    fn worker_running_state(&self) -> Self::State;

    /// Check if the Instance is at the desired state
    fn is_state(&self, proto_state: ProtocolState) -> bool {
        let state = match proto_state {
            ProtocolState::Ready => self.ready_state(),
            ProtocolState::Done => self.done_state(),
            ProtocolState::WorkerRunning => self.worker_running_state(),
        };
        self.state().eq(&state)
    }

    async fn poll_state(
        &mut self,
        stream: &mut TcpStream,
        proto_state: ProtocolState,
    ) -> RussulaResult<Poll<()>> {
        match proto_state {
            ProtocolState::Ready => self.poll_state_impl(stream, &self.ready_state()).await,
            ProtocolState::Done => self.poll_state_impl(stream, &self.done_state()).await,
            ProtocolState::WorkerRunning => {
                self.poll_state_impl(stream, &self.worker_running_state())
                    .await
            }
        }
    }

    /// Run operations for the current state and attempt to make progress until
    /// the desired state is reached.
    async fn poll_state_impl(
        &mut self,
        stream: &mut TcpStream,
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
        if self.is_state(ProtocolState::Done) {
            tracing::info!("{:?}", self.event_recorder());

            // Notify done multiple time in case of packet loss.. this is best effort
            for _i in 0..DONE_SENT_COUNT {
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

    /// Run operations for the current [Self::State]
    async fn run_current(&mut self, stream: &mut TcpStream) -> RussulaResult<()> {
        if let Some(msg) = self.run(stream).await? {
            self.update_peer_state(msg)?;
        }
        Ok(())
    }

    /// Attempt to receive a [Msg] from the peer.
    async fn await_next_msg(&mut self, stream: &mut TcpStream) -> RussulaResult<Option<Msg>> {
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
                    debug!("{} <---- recv msg {}", self.name(), &msg.as_str());

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
                    self.notify_peer(stream).await?;

                    break;
                }
                Err(err) => return Err(err),
            }
        }

        Ok(last_msg)
    }

    /// Check if a received [Msg] transitions self to the next state.
    ///
    /// The current transition_step should be [TransitionStep::AwaitNext].
    fn matches_transition_msg(&self, recv_msg: &Msg) -> RussulaResult<bool> {
        let state = self.state();
        if let TransitionStep::AwaitNext(expected_msg) = state.transition_step() {
            let should_transition_to_next = expected_msg == recv_msg.as_bytes();
            debug!(
                "{} expect: {} actual: {}",
                self.name(),
                std::str::from_utf8(&expected_msg)
                    .expect("AwaitNext should contain valid string slices"),
                recv_msg.as_str()
            );
            Ok(should_transition_to_next)
        } else {
            Ok(false)
        }
    }

    /// Transition to next state
    async fn transition_next(&mut self, stream: &mut TcpStream) -> RussulaResult<()> {
        let nxt = self.state().next_state();
        info!(
            "{:?} MOVING TO NEXT STATE. {:?} ===> {:?}",
            self.name(),
            self.state(),
            nxt
        );

        *self.state_mut() = nxt;

        // notify the peer of the new state
        self.notify_peer(stream).await?;

        Ok(())
    }

    /// Transition to next state triggered by a user input or self triggered event.
    async fn transition_self_or_user_driven(
        &mut self,
        stream: &mut TcpStream,
    ) -> RussulaResult<()> {
        let state = self.state();
        assert!(
            matches!(state.transition_step(), TransitionStep::SelfDriven)
                || matches!(state.transition_step(), TransitionStep::UserDriven)
        );

        self.transition_next(stream).await
    }

    /// Process an event.
    fn on_event(&mut self, event: EventType) {
        self.event_recorder().process(event);
    }

    async fn notify_peer(&mut self, stream: &mut TcpStream) -> RussulaResult<()> {
        let msg = Msg::new(self.state().as_bytes()).expect("Msg data should be a valid string");
        debug!("{} ----> send msg {}", self.name(), &msg.as_str());
        network_utils::send_msg(stream, msg).await?;
        self.on_event(EventType::SendMsg);

        Ok(())
    }
}
