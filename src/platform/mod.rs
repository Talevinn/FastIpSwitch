// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Façade multiplateforme.
//!
//! Les types de données (`AdapterInfo`, `IfaceId`, …) et le contrat
//! `NetworkBackend` sont communs. Chaque système fournit son implémentation
//! dans `platform::windows` ou `platform::linux`.

use std::net::Ipv4Addr;
use std::path::PathBuf;

use crate::cidr::Cidr;

#[cfg(windows)]
pub(crate) mod windows;
#[cfg(windows)]
use windows as imp;

#[cfg(target_os = "linux")]
pub(crate) mod linux;
#[cfg(target_os = "linux")]
use linux as imp;

#[cfg(not(any(windows, target_os = "linux")))]
compile_error!("FastIpSwitch prend en charge Windows et Linux uniquement");

/// Identité stable d'une interface réseau.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IfaceId {
    /// Index noyau (Windows : `InterfaceIndex`, Linux : `ifindex`).
    pub index: u32,
    /// Nom système (Windows : alias netsh, Linux : `eth0`, `wlp3s0`, …).
    pub name: String,
}

#[derive(Clone, Debug)]
pub struct IpEntry {
    pub cidr: Cidr,
    /// Adresse issue de DHCP (par opposition à une adresse manuelle).
    pub from_dhcp: bool,
}

#[derive(Clone, Debug)]
pub struct AdapterInfo {
    pub name: String,
    pub description: String,
    pub mac: String,
    pub id: IfaceId,
    /// Connexion NetworkManager active (Linux uniquement).
    pub connection: Option<String>,
    pub is_up: bool,
    pub dhcp: bool,
    pub addresses: Vec<IpEntry>,
    pub gateway: Option<Ipv4Addr>,
    pub dns: Vec<Ipv4Addr>,
}

impl AdapterInfo {
    /// Adresse « principale » : celle du sous-réseau de la passerelle, sinon
    /// la première adresse DHCP, sinon la première tout court. Les adresses
    /// APIPA (169.254.x.x) sont ignorées : elles signifient « pas de bail ».
    pub fn primary(&self) -> Option<&IpEntry> {
        let usable = || {
            self.addresses
                .iter()
                .filter(|entry| !entry.cidr.addr.is_link_local())
        };

        usable()
            .find(|entry| self.gateway.is_some_and(|gw| entry.cidr.contains(gw)))
            .or_else(|| usable().find(|entry| entry.from_dhcp))
            .or_else(|| usable().next())
    }

    pub fn has_dhcp_address(&self) -> bool {
        self.addresses.iter().any(|e| e.from_dhcp)
    }

    pub fn has_address(&self, ip: Ipv4Addr) -> bool {
        self.addresses.iter().any(|e| e.cidr.addr == ip)
    }
}

/// Référence minimale passée au worker (survit à un rafraîchissement).
#[derive(Clone, Debug)]
pub struct AdapterRef {
    pub id: IfaceId,
    pub mac: String,
    pub dhcp: bool,
    pub connection: Option<String>,
}

impl From<&AdapterInfo> for AdapterRef {
    fn from(adapter: &AdapterInfo) -> Self {
        Self {
            id: adapter.id.clone(),
            mac: adapter.mac.clone(),
            dhcp: adapter.dhcp,
            connection: adapter.connection.clone(),
        }
    }
}

/// Opérations réseau nécessitant les privilèges.
///
/// Windows les exécute en cours de processus (l'application est déjà élevée).
/// Linux les délègue au helper via pkexec, une opération par appel.
pub trait NetworkBackend: Send + Sync {
    fn read(&self) -> Result<Vec<AdapterInfo>, String>;

    fn set_static(
        &self,
        id: &IfaceId,
        cidr: &Cidr,
        gateway: Option<Ipv4Addr>,
    ) -> Result<(), String>;

    fn set_dhcp(&self, id: &IfaceId) -> Result<(), String>;

    fn reset_dns_dhcp(&self, id: &IfaceId) -> Result<(), String>;

    fn set_dns_static(&self, id: &IfaceId, servers: &[Ipv4Addr]) -> Result<(), String>;

    fn add_address(&self, id: &IfaceId, cidr: &Cidr) -> Result<(), String>;

    fn delete_address(&self, id: &IfaceId, cidr: &Cidr) -> Result<(), String>;

    /// Windows : `dhcpstaticipcoexistence`. Linux : sans objet (no-op).
    fn set_coexistence(&self, id: &IfaceId, enabled: bool) -> Result<(), String>;
}

pub fn backend() -> &'static dyn NetworkBackend {
    imp::backend()
}

pub fn read_adapters() -> Result<Vec<AdapterInfo>, String> {
    backend().read()
}

// --- Système ---

pub fn is_admin() -> bool {
    imp::is_admin()
}

pub fn admin_instance_running() -> bool {
    imp::admin_instance_running()
}

pub fn single_instance(admin: bool) -> bool {
    imp::single_instance(admin)
}

pub fn elevate_self() -> Result<(), String> {
    imp::elevate_self()
}

pub fn system_is_dark() -> bool {
    imp::system_is_dark()
}

pub fn lang_is_french() -> bool {
    imp::lang_is_french()
}

/// La fermeture de fenêtre masque-t-elle l'application dans la zone de
/// notification ? Windows : oui en mode élevé. Linux : non (le tray n'est pas
/// garanti, cacher la fenêtre la rendrait inaccessible).
pub fn hides_on_close(admin: bool) -> bool {
    imp::hides_on_close(admin)
}

pub fn error_box(message: &str) {
    imp::error_box(message);
}

pub fn info_box(message: &str) {
    imp::info_box(message);
}

// --- Chemins ---

pub fn config_dir() -> PathBuf {
    imp::config_dir()
}

pub fn local_dir() -> PathBuf {
    imp::local_dir()
}

// --- Installation ---

pub fn install_dir() -> PathBuf {
    imp::install_dir()
}

pub fn is_installed() -> bool {
    imp::is_installed()
}

pub fn autostart_enabled() -> bool {
    imp::autostart_enabled()
}

pub fn install(with_autostart: bool) -> Result<String, String> {
    imp::install(with_autostart)
}
