// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Backend Windows : API IpHelper pour la lecture, `netsh` pour l'écriture.

use std::net::Ipv4Addr;
use std::path::PathBuf;

use crate::cidr::Cidr;
use crate::platform::{AdapterInfo, IfaceId, NetworkBackend};

mod installer;
mod netsh;
mod paths;
mod read;
mod sys;

pub fn is_admin() -> bool {
    sys::is_admin()
}

pub fn admin_instance_running() -> bool {
    sys::admin_instance_running()
}

pub fn single_instance(admin: bool) -> bool {
    sys::single_instance(admin)
}

pub fn elevate_self() -> Result<(), String> {
    sys::elevate_self()
}

pub fn system_is_dark() -> bool {
    sys::system_is_dark()
}

pub fn lang_is_french() -> bool {
    sys::lang_is_french()
}

pub fn hides_on_close(admin: bool) -> bool {
    admin
}

pub fn error_box(message: &str) {
    sys::error_box(message);
}

pub fn info_box(message: &str) {
    sys::info_box(message);
}

pub fn config_dir() -> PathBuf {
    paths::config_dir()
}

pub fn local_dir() -> PathBuf {
    paths::local_dir()
}

pub fn install_dir() -> PathBuf {
    installer::install_dir()
}

pub fn is_installed() -> bool {
    installer::is_installed()
}

pub fn autostart_enabled() -> bool {
    installer::autostart_enabled()
}

pub fn install(with_autostart: bool) -> Result<String, String> {
    installer::install(with_autostart)
}

struct WindowsBackend;

static BACKEND: WindowsBackend = WindowsBackend;

pub fn backend() -> &'static dyn NetworkBackend {
    &BACKEND
}

impl NetworkBackend for WindowsBackend {
    fn read(&self) -> Result<Vec<AdapterInfo>, String> {
        read::read_adapters().map_err(|e| e.to_string())
    }

    fn set_static(
        &self,
        id: &IfaceId,
        cidr: &Cidr,
        gateway: Option<Ipv4Addr>,
    ) -> Result<(), String> {
        netsh::set_static(id.index, cidr, gateway)
    }

    fn set_dhcp(&self, id: &IfaceId) -> Result<(), String> {
        netsh::set_dhcp(id.index)
    }

    fn reset_dns_dhcp(&self, id: &IfaceId) -> Result<(), String> {
        netsh::reset_dns_dhcp(id.index)
    }

    fn set_dns_static(&self, id: &IfaceId, servers: &[Ipv4Addr]) -> Result<(), String> {
        netsh::set_dns_static(id.index, servers)
    }

    fn add_address(&self, id: &IfaceId, cidr: &Cidr) -> Result<(), String> {
        netsh::add_address(id.index, cidr)
    }

    fn delete_address(&self, id: &IfaceId, cidr: &Cidr) -> Result<(), String> {
        netsh::delete_address(id.index, cidr.addr)
    }

    fn set_coexistence(&self, id: &IfaceId, enabled: bool) -> Result<(), String> {
        netsh::set_coexistence(id.index, enabled)
    }
}
