// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Backend Linux : lecture via `ip -j`, écriture via le helper pkexec
//! (`nmcli` pour l'adresse principale, `ip` pour les alias).

use std::net::Ipv4Addr;
use std::path::PathBuf;

use crate::cidr::Cidr;
use crate::platform::{AdapterInfo, IfaceId, NetworkBackend};

mod exec;
mod helper;
mod installer;
mod instance;
mod ops;
mod paths;
mod read;
mod theme;

/// L'interface n'est jamais verrouillée : l'élévation se fait à la demande,
/// au moment d'appliquer une modification (polkit).
pub fn is_admin() -> bool {
    true
}

/// Pas d'instance administrateur distincte sous Linux.
pub fn admin_instance_running() -> bool {
    false
}

pub fn single_instance(_admin: bool) -> bool {
    instance::lock()
}

/// Aucune relance de l'application n'est nécessaire sous Linux.
pub fn elevate_self() -> Result<(), String> {
    Ok(())
}

pub fn system_is_dark() -> bool {
    theme::system_is_dark()
}

pub fn hides_on_close(_admin: bool) -> bool {
    false
}

pub fn lang_is_french() -> bool {
    let value = ["LC_ALL", "LC_MESSAGES", "LANG"].iter().find_map(|var| {
        std::env::var(var)
            .ok()
            .filter(|v| !v.is_empty() && v != "C" && v != "POSIX")
    });

    value
        .map(|value| value.to_lowercase().starts_with("fr"))
        .unwrap_or(false)
}

pub fn error_box(message: &str) {
    eprintln!("FastIpSwitch: {message}");
}

pub fn info_box(message: &str) {
    eprintln!("FastIpSwitch: {message}");
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

struct LinuxBackend;

static BACKEND: LinuxBackend = LinuxBackend;

pub fn backend() -> &'static dyn NetworkBackend {
    &BACKEND
}

impl NetworkBackend for LinuxBackend {
    fn read(&self) -> Result<Vec<AdapterInfo>, String> {
        read::read_adapters()
    }

    fn set_static(
        &self,
        id: &IfaceId,
        cidr: &Cidr,
        gateway: Option<Ipv4Addr>,
    ) -> Result<(), String> {
        ops::set_static(id, cidr, gateway)
    }

    fn set_dhcp(&self, id: &IfaceId) -> Result<(), String> {
        ops::set_dhcp(id)
    }

    fn reset_dns_dhcp(&self, id: &IfaceId) -> Result<(), String> {
        ops::reset_dns_dhcp(id)
    }

    fn set_dns_static(&self, id: &IfaceId, servers: &[Ipv4Addr]) -> Result<(), String> {
        ops::set_dns_static(id, servers)
    }

    fn add_address(&self, id: &IfaceId, cidr: &Cidr) -> Result<(), String> {
        ops::add_address(id, cidr)
    }

    fn delete_address(&self, id: &IfaceId, cidr: &Cidr) -> Result<(), String> {
        ops::delete_address(id, cidr)
    }

    fn set_coexistence(&self, _id: &IfaceId, _enabled: bool) -> Result<(), String> {
        // Sous Linux, les alias coexistent nativement avec DHCP.
        Ok(())
    }
}
