// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

pub type OrchResult<T, E = OrchError> = Result<T, E>;

#[derive(Debug)]
pub enum OrchError {
    // Initialization error
    Init { dbg: String },
    // Ec2 sdk error
    Ec2 { dbg: String },
    // Iam sdk error
    Iam { dbg: String },
    // Ssm sdk error
    Ssm { dbg: String },
    // S3 sdk error
    S3 { dbg: String },
    // Russula error
    Russula { dbg: String },
}

impl std::fmt::Display for OrchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrchError::Init { dbg } => write!(f, "{}", dbg),
            OrchError::Ec2 { dbg } => write!(f, "{}", dbg),
            OrchError::Iam { dbg } => write!(f, "{}", dbg),
            OrchError::Ssm { dbg } => write!(f, "{}", dbg),
            OrchError::S3 { dbg } => write!(f, "{}", dbg),
            OrchError::Russula { dbg } => write!(f, "{}", dbg),
        }
    }
}

impl std::error::Error for OrchError {}
