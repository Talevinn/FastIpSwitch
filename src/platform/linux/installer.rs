// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::path::PathBuf;

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Répertoire d'installation par utilisateur.
pub fn install_dir() -> PathBuf {
    home().join(".local").join("bin")
}

pub fn installed_exe() -> PathBuf {
    install_dir().join("fastipswitch")
}

pub fn is_installed() -> bool {
    installed_exe().is_file()
}

pub fn autostart_enabled() -> bool {
    autostart_file().is_file()
}

fn autostart_file() -> PathBuf {
    super::paths::config_dir()
        .parent()
        .map(|dir| dir.join("autostart").join("fastipswitch.desktop"))
        .unwrap_or_else(|| home().join(".config/autostart/fastipswitch.desktop"))
}

pub fn install(_with_autostart: bool) -> Result<String, String> {
    Err("installation Linux non implémentée".into())
}
