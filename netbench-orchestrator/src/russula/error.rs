// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use tokio::io::ErrorKind;

pub type RussulaResult<T, E = RussulaError> = Result<T, E>;

/// Custom error type emitted by Russula.
#[derive(Debug)]
pub enum RussulaError {
    /// The remote server refused the connection.
    NetworkConnectionRefused { dbg: String },

    /// A non-recoverable network error.
    NetworkFail { dbg: String },

    /// A read from the socket returned an error.
    ReadFail { dbg: String },

    /// Possibly no more data to read. Try again later.
    NetworkBlocked { dbg: String },

    /// Failure when trying to read a [Msg](crate::russula::network_utils::Msg)
    BadMsg { dbg: String },
}

impl std::fmt::Display for RussulaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RussulaError::NetworkConnectionRefused { dbg } => {
                write!(f, "NetworkConnectionRefused {}", dbg)
            }
            RussulaError::NetworkFail { dbg } => write!(f, "NetworkFail {}", dbg),
            RussulaError::ReadFail { dbg } => write!(f, "ReadFail {}", dbg),
            RussulaError::NetworkBlocked { dbg } => write!(f, "NetworkBlocked {}", dbg),
            RussulaError::BadMsg { dbg } => write!(f, "BadMsg {}", dbg),
        }
    }
}

impl std::error::Error for RussulaError {}

impl RussulaError {
    /// Specify which errors are non-recoverable.
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
