// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

fn env_dir(var: &str) -> PathBuf {
    std::env::var_os(var)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("FastIpSwitch")
}

/// Mode portable : si un dossier `data` peut être créé/écrit à côté de
/// l'exécutable, on y stocke tout. Sinon, repli sur les dossiers Windows.
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

pub fn config_dir() -> PathBuf {
    portable_dir()
        .cloned()
        .unwrap_or_else(|| env_dir("APPDATA"))
}

pub fn local_dir() -> PathBuf {
    portable_dir()
        .cloned()
        .unwrap_or_else(|| env_dir("LOCALAPPDATA"))
}
