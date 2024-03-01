// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::orchestrator::{OrchError, OrchResult};
use aws_sdk_ec2::types::Instance;
use std::net::IpAddr;
use tracing::debug;

// Details about a provisioned instance
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct InstanceDetail {
    endpoint_type: EndpointType,
    az: Az,
    instance_id: String,
    host_ips: HostIps,
}

impl std::fmt::Display for &InstanceDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:?} ({}): {} -- {}",
            self.endpoint_type, self.az, self.instance_id, self.host_ips
        )?;
        Ok(())
    }
}

impl InstanceDetail {
    pub fn new(
        endpoint_type: EndpointType,
        az: Az,
        instance: Instance,
        host_ips: HostIps,
    ) -> OrchResult<Self> {
        let instance_id = instance
            .instance_id()
            .ok_or(OrchError::Ec2 {
                dbg: "No instance id".to_string(),
            })
            .map_err(|err| {
                debug!("{}", err);
                err
            })?
            .to_string();

        Ok(InstanceDetail {
            endpoint_type,
            az,
            instance_id,
            host_ips,
        })
    }

    pub fn instance_id(&self) -> &str {
        &self.instance_id
    }

    pub fn host_ips(&self) -> &HostIps {
        &self.host_ips
    }

    pub fn endpoint_type(&self) -> &EndpointType {
        &self.endpoint_type
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum EndpointType {
    Server,
    Client,
}

impl EndpointType {
    pub fn as_str(&self) -> &str {
        match self {
            EndpointType::Server => "Server",
            EndpointType::Client => "Client",
        }
    }
}

// The public and private ips for a remote Ec2 host
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct HostIps {
    private_ip: PrivIp,
    public_ip: PubIp,
}

impl HostIps {
    pub fn new(private_ip: PrivIp, public_ip: PubIp) -> Self {
        HostIps {
            private_ip,
            public_ip,
        }
    }

    pub fn public_ip(&self) -> &PubIp {
        &self.public_ip
    }

    pub fn private_ip(&self) -> &PrivIp {
        &self.private_ip
    }
}

impl std::fmt::Display for HostIps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "public_ip: {}, private_ip: {}",
            self.public_ip, self.private_ip
        )
    }
}

macro_rules! ec2_new_types {
    ($name:ident) => {
        #[derive(Clone, Debug, Hash, Eq, PartialEq, PartialOrd, Ord)]
        pub struct $name(String);

        impl $name {
            pub fn as_string(&self) -> String {
                self.clone().0
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                $name(value)
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}: {}", std::any::type_name::<$name>(), self.0)?;
                Ok(())
            }
        }
    };
}

ec2_new_types!(SubnetId);
ec2_new_types!(VpcId);
ec2_new_types!(Az);

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PubIp(pub IpAddr);

impl std::fmt::Display for PubIp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct PrivIp(pub IpAddr);

impl std::fmt::Display for PrivIp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
