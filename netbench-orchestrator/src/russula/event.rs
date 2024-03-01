// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use core::fmt::{Debug, Display};

pub enum EventType {
    SendMsg,
    RecvMsg,
}

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

impl Display for EventRecorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "send_cnt: {}, recv_cnt: {}",
            self.send_msg, self.recv_msg
        )
    }
}
