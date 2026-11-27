// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Instance unique : verrou `flock`, fichier PID et chemin de socket.
//!
//! Le socket `$XDG_RUNTIME_DIR/fastipswitch-<uid>.sock` est prévu pour qu'une
//! seconde instance puisse demander l'affichage de la fenêtre. `main.rs`
//! n'expose toutefois pas de point d'accroche pour démarrer un serveur ou
//! traiter la demande : `notify_running()` est donc fourni pour une intégration
//! ultérieure, sans consommateur pour l'instant.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::fd::AsRawFd;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::paths;

/// Verrou d'instance unique, conservé pendant toute la durée du processus.
static LOCK: OnceLock<Option<File>> = OnceLock::new();

fn uid() -> u32 {
    // `geteuid` est un appel système sans pointeur : sûr tant que l'on ne
    // suppose rien de son résultat.
    unsafe { libc::geteuid() }
}

fn pid_file_name(uid: u32) -> String {
    format!("fastipswitch-{uid}.pid")
}

fn socket_file_name(uid: u32) -> String {
    format!("fastipswitch-{uid}.sock")
}

fn pid_path() -> PathBuf {
    paths::runtime_dir().join(pid_file_name(uid()))
}

fn socket_path_in(dir: &Path, uid: u32) -> PathBuf {
    dir.join(socket_file_name(uid))
}

fn socket_path() -> PathBuf {
    socket_path_in(&paths::runtime_dir(), uid())
}

/// Écrit le PID de l'instance qui détient le verrou. Le fichier est lisible
/// (mode 0600) par l'utilisateur courant et réécrit à chaque démarrage.
fn write_pid_file() {
    let Ok(mut file) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .mode(0o600)
        .open(pid_path())
    else {
        return;
    };
    let _ = writeln!(file, "{}", std::process::id());
}

/// Prend le verrou d'instance unique. Renvoie `false` si une autre instance
/// le détient déjà.
pub fn lock() -> bool {
    LOCK.get_or_init(|| {
        let dir = paths::runtime_dir();
        // `/tmp` existe déjà ; un `XDG_RUNTIME_DIR` fourni mais absent est créé.
        let _ = fs::create_dir_all(&dir);
        let path = dir.join(format!("fastipswitch-{}.lock", uid()));

        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .mode(0o600)
            .open(path)
            .ok()?;

        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result != 0 {
            return None;
        }

        write_pid_file();
        Some(file)
    })
    .is_some()
}

/// Demande à l'instance déjà lancée d'afficher sa fenêtre.
///
/// Échoue silencieusement (`false`) tant qu'aucun serveur n'écoute sur le
/// socket. Fonction conservée pour l'intégration future côté `main.rs`.
#[allow(dead_code)]
pub fn notify_running() -> bool {
    use std::os::unix::net::UnixStream;

    let Ok(mut stream) = UnixStream::connect(socket_path()) else {
        return false;
    };
    stream.write_all(b"show\n").is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_de_fichiers_incluent_uid() {
        assert_eq!(pid_file_name(1000), "fastipswitch-1000.pid");
        assert_eq!(socket_file_name(1000), "fastipswitch-1000.sock");
    }

    #[test]
    fn socket_dans_le_repertoire_runtime() {
        let dir = Path::new("/run/user/1000");
        assert_eq!(
            socket_path_in(dir, 1000),
            PathBuf::from("/run/user/1000/fastipswitch-1000.sock")
        );
    }
}
