// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Chemins de données Linux (XDG) et mode portable.

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
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
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Résout un répertoire de base XDG : une valeur absolue non vide prioritaire,
/// sinon `<home>/<fallback>`. Fonction pure pour rester testable sans muter
/// l'environnement du processus (interdit en edition 2024).
fn xdg_base(value: Option<&OsStr>, home: &Path, fallback: &str) -> PathBuf {
    // La spécification XDG impose un chemin absolu : une valeur relative est
    // ignorée (elle pourrait sinon écrire dans le répertoire courant).
    value
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| home.join(fallback))
}

/// `$XDG_CONFIG_HOME` (défaut `~/.config`), sans sous-dossier applicatif.
pub fn config_home() -> PathBuf {
    let value = std::env::var_os("XDG_CONFIG_HOME");
    xdg_base(value.as_deref(), &home(), ".config")
}

/// `$XDG_STATE_HOME` (défaut `~/.local/state`), sans sous-dossier applicatif.
fn state_home() -> PathBuf {
    let value = std::env::var_os("XDG_STATE_HOME");
    xdg_base(value.as_deref(), &home(), ".local/state")
}

/// `$XDG_DATA_HOME` (défaut `~/.local/share`), sans sous-dossier applicatif.
pub fn data_home() -> PathBuf {
    let value = std::env::var_os("XDG_DATA_HOME");
    xdg_base(value.as_deref(), &home(), ".local/share")
}

/// `$XDG_RUNTIME_DIR` (défaut `/tmp/fastipswitch-<uid>`) : verrou d'instance,
/// PID et socket. En l'absence de répertoire runtime, on crée un sous-dossier
/// privé (0700) plutôt que d'écrire directement dans `/tmp`.
pub fn runtime_dir() -> PathBuf {
    let value = std::env::var_os("XDG_RUNTIME_DIR");
    if let Some(dir) = value
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
    {
        return dir;
    }

    let dir = PathBuf::from("/tmp").join(format!("fastipswitch-{}", unsafe { libc::geteuid() }));
    let _ = fs::create_dir_all(&dir);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
    }
    dir
}

/// Préférences et liste d'alias : `$XDG_CONFIG_HOME/fastipswitch`.
pub fn config_dir() -> PathBuf {
    portable_dir()
        .cloned()
        .unwrap_or_else(|| config_home().join("fastipswitch"))
}

/// Session de récupération et journal : `$XDG_STATE_HOME/fastipswitch`.
pub fn local_dir() -> PathBuf {
    portable_dir()
        .cloned()
        .unwrap_or_else(|| state_home().join("fastipswitch"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(value: &str) -> PathBuf {
        PathBuf::from(value)
    }

    #[test]
    fn xdg_valeur_prioritaire() {
        let home = path("/home/test");
        assert_eq!(
            xdg_base(Some(OsStr::new("/tmp/cfg")), &home, ".config"),
            path("/tmp/cfg")
        );
        assert_eq!(
            xdg_base(Some(OsStr::new("/tmp/state")), &home, ".local/state"),
            path("/tmp/state")
        );
    }

    #[test]
    fn xdg_relatif_ignore() {
        let home = path("/home/test");
        assert_eq!(
            xdg_base(Some(OsStr::new("config-relatif")), &home, ".config"),
            path("/home/test/.config")
        );
    }

    #[test]
    fn xdg_repli_sur_home() {
        let home = path("/home/test");
        assert_eq!(xdg_base(None, &home, ".config"), path("/home/test/.config"));
        assert_eq!(
            xdg_base(None, &home, ".local/share"),
            path("/home/test/.local/share")
        );
    }

    #[test]
    fn xdg_valeur_vide_ignoree() {
        let home = path("/home/test");
        assert_eq!(
            xdg_base(Some(OsStr::new("")), &home, ".config"),
            path("/home/test/.config")
        );
    }
}
