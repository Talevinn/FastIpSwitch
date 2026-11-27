// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::cidr::Cidr;

/// Répertoire des préférences (XDG sous Linux, `%APPDATA%` sous Windows).
pub fn config_dir() -> PathBuf {
    crate::platform::config_dir()
}

/// Répertoire d'état : session de récupération et journal.
pub fn local_dir() -> PathBuf {
    crate::platform::local_dir()
}

fn ui_state_path() -> PathBuf {
    config_dir().join("state.toml")
}

pub fn session_path() -> PathBuf {
    local_dir().join("session.toml")
}

/// Préférences d'interface : jamais d'état réseau actif ici.
#[derive(Debug, Clone, Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct UiState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter_mac: Option<String>,
    pub aliases: Vec<String>,
    pub last_ip: String,
    pub last_gateway: String,
    pub last_dns: String,
}

/// Trace des modifications temporaires appliquées par l'application.
/// Sert à restaurer DHCP après un arrêt brutal.
#[derive(Debug, Clone, Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct Session {
    pub adapter_mac: String,
    /// Nom système de l'interface (Linux : `eth0` ; Windows : vide).
    pub ifname: String,
    /// UUID de la connexion NetworkManager active au moment de l'application.
    pub connection: Option<String>,
    pub if_index: u32,
    pub applied: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending: Option<String>,
    pub static_set: bool,
    pub coexistence: bool,
    pub dns_set: bool,
}

impl Session {
    /// Vrai s'il n'y a aucune modification réseau à restaurer.
    /// `adapter_mac`/`if_index` ne sont que des métadonnées.
    pub fn is_empty(&self) -> bool {
        self.applied.is_empty()
            && self.pending.is_none()
            && !self.static_set
            && !self.coexistence
            && !self.dns_set
    }

    pub fn note_alias(&mut self, cidr: &Cidr) {
        let s = cidr.to_string();
        if !self.applied.contains(&s) {
            self.applied.push(s);
        }
    }

    pub fn forget_alias(&mut self, cidr: &Cidr) {
        let s = cidr.to_string();
        self.applied.retain(|a| a != &s);
        if self.pending.as_deref() == Some(s.as_str()) {
            self.pending = None;
        }
    }
}

fn load_toml<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let text = fs::read_to_string(path).ok()?;
    toml::from_str(&text).ok()
}

fn save_toml<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("création de {parent:?} : {e}"))?;
    }
    let tmp = path.with_extension("toml.tmp");
    let data = toml::to_string_pretty(value).map_err(|e| e.to_string())?;
    fs::write(&tmp, data.as_bytes()).map_err(|e| format!("écriture de {tmp:?} : {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("remplacement de {path:?} : {e}"))?;
    Ok(())
}

pub fn load_ui_state() -> UiState {
    load_toml(&ui_state_path()).unwrap_or_default()
}

pub fn save_ui_state(state: &UiState) {
    if let Err(e) = save_toml(&ui_state_path(), state) {
        crate::log::write(&format!("state.toml non enregistré : {e}"));
    }
}

pub fn load_session() -> Option<Session> {
    load_toml(&session_path())
}

pub fn save_session(session: &Session) {
    if let Err(e) = save_toml(&session_path(), session) {
        crate::log::write(&format!("session.toml non enregistré : {e}"));
    }
}

pub fn clear_session() {
    let _ = fs::remove_file(session_path());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_state_toml_roundtrip() {
        let state = UiState {
            adapter_mac: Some("08-3A-88-59-27-40".into()),
            aliases: vec!["192.168.3.10/24".into()],
            last_ip: "192.168.2.10/24".into(),
            last_gateway: "192.168.2.1".into(),
            last_dns: "1.1.1.1, 8.8.8.8".into(),
        };
        let text = toml::to_string_pretty(&state).unwrap();
        assert!(text.contains("adapter_mac"));
        let back: UiState = toml::from_str(&text).unwrap();
        assert_eq!(back.adapter_mac, state.adapter_mac);
        assert_eq!(back.aliases, state.aliases);
        assert_eq!(back.last_dns, state.last_dns);
    }

    #[test]
    fn session_toml_roundtrip_without_pending() {
        let session = Session {
            adapter_mac: "08-3A-88-59-27-40".into(),
            ifname: String::new(),
            connection: None,
            if_index: 12,
            applied: vec!["192.168.3.10/24".into()],
            pending: None,
            static_set: true,
            coexistence: true,
            dns_set: true,
        };
        let text = toml::to_string_pretty(&session).unwrap();
        assert!(!text.contains("pending"));
        let back: Session = toml::from_str(&text).unwrap();
        assert!(back.pending.is_none());
        assert!(back.dns_set);
        assert!(!back.is_empty());
    }

    #[test]
    fn metadata_only_session_is_empty() {
        let session = Session {
            adapter_mac: "aa".into(),
            if_index: 12,
            ..Session::default()
        };
        assert!(session.is_empty());
    }
}
