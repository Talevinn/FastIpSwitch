// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Mode portable : si un dossier `data` peut être créé/écrit à côté de
/// l'exécutable, on y stocke tout. Sinon, repli sur les dossiers XDG.
fn portable_dir() -> Option<&'static PathBuf> {
    static PORTABLE: OnceLock<Option<PathBuf>> = OnceLock::new();

    PORTABLE
        .get_or_init(|| {
            if cfg!(test) {
                return None;
            }
            let exe = std::env::current_exe().ok()?;
            let data = exe.parent()?.join("data");
            fs::create_dir_all(&data).ok()?;
            let probe = data.join(".write-test");
            fs::write(&probe, b"ok").ok()?;
            let _ = fs::remove_file(&probe);
            Some(data)
        })
        .as_ref()
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn xdg_dir(var: &str, fallback: &str) -> PathBuf {
    std::env::var_os(var)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home().join(fallback))
        .join("fastipswitch")
}

/// Préférences et liste d'alias : `$XDG_CONFIG_HOME/fastipswitch`.
pub fn config_dir() -> PathBuf {
    portable_dir()
        .cloned()
        .unwrap_or_else(|| xdg_dir("XDG_CONFIG_HOME", ".config"))
}

/// Session de récupération et journal : `$XDG_STATE_HOME/fastipswitch`.
pub fn local_dir() -> PathBuf {
    portable_dir()
        .cloned()
        .unwrap_or_else(|| xdg_dir("XDG_STATE_HOME", ".local/state"))
}
