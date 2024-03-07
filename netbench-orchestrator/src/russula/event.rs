// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use core::fmt::Debug;

/// A list of events emitted by Russula.
pub enum EventType {
    /// A Msg was sent.
    SendMsg,

    /// A Msg was received.
    RecvMsg,
}

/// An event recorder for Russula.
#[derive(Debug, Default, Clone)]
pub struct EventRecorder {
    send_msg: u64,
    recv_msg: u64,
}

impl EventRecorder {
    pub fn process(&mut self, event: EventType) {
        match event {
            EventType::SendMsg => self.send_msg += 1,
            EventType::RecvMsg => self.recv_msg += 1,
        }
    }
}
