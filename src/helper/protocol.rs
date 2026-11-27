// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Protocole JSON entre l'interface non privilégiée et le helper pkexec.
//!
//! Une requête = une opération. Le helper revalide tout : le fait que la
//! requête vienne de l'interface ne constitue jamais une garantie.

use std::net::Ipv4Addr;

use serde::{Deserialize, Serialize};

use crate::cidr::Cidr;

pub const VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub version: u32,
    #[serde(flatten)]
    pub op: Operation,
}

impl Request {
    pub fn new(op: Operation) -> Self {
        Self {
            version: VERSION,
            op,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Operation {
    ApplyStatic {
        ifname: String,
        cidr: Cidr,
        gateway: Option<Ipv4Addr>,
        #[serde(default)]
        dns: Vec<Ipv4Addr>,
    },
    SetDhcp {
        ifname: String,
    },
    SetDns {
        ifname: String,
        dns: Vec<Ipv4Addr>,
    },
    ResetDns {
        ifname: String,
    },
    AddAlias {
        ifname: String,
        cidr: Cidr,
    },
    RemoveAlias {
        ifname: String,
        cidr: Cidr,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Response {
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl Response {
    pub fn ok() -> Self {
        Self {
            ok: true,
            message: None,
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: Some(message.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrip() {
        let request = Request::new(Operation::ApplyStatic {
            ifname: "wlp3s0".into(),
            cidr: Cidr::parse("192.168.3.10/24").unwrap(),
            gateway: Some(Ipv4Addr::new(192, 168, 3, 1)),
            dns: vec![Ipv4Addr::new(1, 1, 1, 1)],
        });

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"op\":\"apply_static\""));
        let back: Request = serde_json::from_str(&json).unwrap();
        match back.op {
            Operation::ApplyStatic {
                ifname,
                cidr,
                gateway,
                dns,
            } => {
                assert_eq!(ifname, "wlp3s0");
                assert_eq!(cidr.to_string(), "192.168.3.10/24");
                assert_eq!(gateway, Some(Ipv4Addr::new(192, 168, 3, 1)));
                assert_eq!(dns, vec![Ipv4Addr::new(1, 1, 1, 1)]);
            }
            other => panic!("opération inattendue : {other:?}"),
        }
    }

    #[test]
    fn rejects_invalid_cidr() {
        let json = r#"{"version":1,"op":"add_alias","ifname":"eth0","cidr":"192.168.1.0/24"}"#;
        assert!(serde_json::from_str::<Request>(json).is_err());
    }
}
