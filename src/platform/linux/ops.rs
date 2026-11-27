// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::net::Ipv4Addr;

use crate::cidr::Cidr;
use crate::platform::IfaceId;

// Toutes les opérations passent par `super::helper::call` avec une requête
// `crate::helper::protocol::Request`.

pub fn set_static(_id: &IfaceId, _cidr: &Cidr, _gateway: Option<Ipv4Addr>) -> Result<(), String> {
    Err("set_static : opération non implémentée".into())
}

pub fn set_dhcp(_id: &IfaceId) -> Result<(), String> {
    Err("set_dhcp : opération non implémentée".into())
}

pub fn reset_dns_dhcp(_id: &IfaceId) -> Result<(), String> {
    Err("reset_dns_dhcp : opération non implémentée".into())
}

pub fn set_dns_static(_id: &IfaceId, _servers: &[Ipv4Addr]) -> Result<(), String> {
    Err("set_dns_static : opération non implémentée".into())
}

pub fn add_address(_id: &IfaceId, _cidr: &Cidr) -> Result<(), String> {
    Err("add_address : opération non implémentée".into())
}

pub fn delete_address(_id: &IfaceId, _cidr: &Cidr) -> Result<(), String> {
    Err("delete_address : opération non implémentée".into())
}
