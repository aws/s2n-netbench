// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::orchestrator::{OrchError, OrchResult};
use aws_sdk_ec2::types::Instance;
use std::{collections::HashMap, net::IpAddr};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct InstanceDetail {
    pub endpoint_type: EndpointType,
    pub az: Az,
    pub instance_id: String,
    pub host_ips: HostIps,
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
    pub fn new(endpoint_type: EndpointType, az: Az, instance: Instance, host_ips: HostIps) -> Self {
        let instance_id = instance
            .instance_id()
            .ok_or(OrchError::Ec2 {
                dbg: "No instance id".to_string(),
            })
            .expect("instance_id failed")
            .to_string();

        InstanceDetail {
            endpoint_type,
            az,
            instance_id,
            host_ips,
        }
    }

    pub fn instance_id(&self) -> OrchResult<&str> {
        Ok(&self.instance_id)
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

pub type NetworkingInfraDetail = HashMap<Az, SubnetId>;

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
