// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

#![allow(unused)]
pub type OrchResult<T, E = OrchError> = Result<T, E>;

#[derive(Debug)]
pub enum OrchError {
    Init { dbg: String },
    Ec2 { dbg: String },
    Iam { dbg: String },
    Ssm { dbg: String },
}

impl std::fmt::Display for OrchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrchError::Init { dbg } => write!(f, "{}", dbg),
            OrchError::Ec2 { dbg } => write!(f, "{}", dbg),
            OrchError::Iam { dbg } => write!(f, "{}", dbg),
            OrchError::Ssm { dbg } => write!(f, "{}", dbg),
        }
    }
}

impl std::error::Error for OrchError {}
