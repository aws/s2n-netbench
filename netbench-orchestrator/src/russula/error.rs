// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use tokio::io::ErrorKind;

pub type RussulaResult<T, E = RussulaError> = Result<T, E>;

#[derive(Debug)]
pub enum RussulaError {
    NetworkConnectionRefused { dbg: String },
    NetworkFail { dbg: String },
    NetworkBlocked { dbg: String },
    BadMsg { dbg: String },
}

impl std::fmt::Display for RussulaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RussulaError::NetworkConnectionRefused { dbg } => {
                write!(f, "NetworkConnectionRefused {}", dbg)
            }
            RussulaError::NetworkFail { dbg } => write!(f, "NetworkFail {}", dbg),
            RussulaError::NetworkBlocked { dbg } => write!(f, "NetworkBlocked {}", dbg),
            RussulaError::BadMsg { dbg } => write!(f, "BadMsg {}", dbg),
        }
    }
}

impl std::error::Error for RussulaError {}

impl RussulaError {
    #[allow(clippy::match_like_matches_macro)]
    pub fn is_fatal(&self) -> bool {
        match self {
            RussulaError::NetworkBlocked { dbg: _ } => false,
            _ => true,
        }
    }
}

impl From<tokio::io::Error> for RussulaError {
    fn from(err: tokio::io::Error) -> Self {
        match err.kind() {
            ErrorKind::WouldBlock => RussulaError::NetworkBlocked {
                dbg: err.to_string(),
            },
            ErrorKind::ConnectionRefused => RussulaError::NetworkConnectionRefused {
                dbg: err.to_string(),
            },
            _ => RussulaError::NetworkFail {
                dbg: err.to_string(),
            },
        }
    }
}
