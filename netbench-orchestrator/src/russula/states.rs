// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use super::{error::RussulaError, network_utils::Msg};
use crate::russula::RussulaResult;
use bytes::Bytes;
use core::fmt::Debug;
use serde::{Deserialize, Serialize};

/// Specify how a state machine might transition to the next State.
///
/// A state machine can move to the next state by receiving a message from
/// its peer, a signal from the external user or be itself after after
/// completing a task.
#[derive(Debug)]
pub enum TransitionStep {
    /// The State machine should transition to the next state itself.
    ///
    /// Can be use to represent waiting for some long running task.
    SelfDriven,

    /// Wait for external user input before moving to the next state.
    UserDriven,

    /// Wait for a peer [Msg] before moving to the next state
    AwaitNext(Bytes),

    /// Used to signal the terminal state of a state machine.
    Finished,
}

pub trait StateApi: Debug + Serialize + for<'a> Deserialize<'a> {
    /// The [TransitionStep] required to move to the next state.
    fn transition_step(&self) -> TransitionStep;

    /// Returns the next step in the state machine.
    fn next_state(&self) -> Self;

    fn eq(&self, other: &Self) -> bool {
        self.as_bytes() == other.as_bytes()
    }

    fn as_bytes(&self) -> Bytes {
        serde_json::to_string(self).unwrap().into()
    }

    fn from_msg(msg: Msg) -> RussulaResult<Self> {
        let msg_str = msg.as_str();
        serde_json::from_str(msg_str).map_err(|_err| RussulaError::BadMsg {
            dbg: format!(
                "received a malformed msg. len: {} data: {:?}",
                msg.payload_len(),
                msg.as_str()
            ),
        })
    }
}
