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
