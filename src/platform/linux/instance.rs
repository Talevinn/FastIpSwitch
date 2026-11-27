// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::fs::{self, File, OpenOptions};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Verrou d'instance unique, conservé pendant toute la durée du processus.
static LOCK: OnceLock<Option<File>> = OnceLock::new();

pub fn lock() -> bool {
    LOCK.get_or_init(|| {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("/tmp"));
        let _ = fs::create_dir_all(&dir);
        let path = dir.join(format!("fastipswitch-{}.lock", unsafe { libc::geteuid() }));

        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .truncate(false)
            .open(path)
            .ok()?;

        let result = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if result == 0 { Some(file) } else { None }
    })
    .is_some()
}
