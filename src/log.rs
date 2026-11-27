// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn init() -> PathBuf {
    let dir = crate::store::local_dir();
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("fastipswitch.log");

    // Rotation simple : on conserve un ancien journal, on ne tronque jamais
    // celui du dernier crash.
    if let Ok(metadata) = fs::metadata(&path)
        && metadata.len() > 1_000_000
    {
        let rotated = path.with_extension("log.1");
        let _ = fs::remove_file(&rotated);
        let _ = fs::rename(&path, &rotated);
    }
    let _ = OpenOptions::new().create(true).append(true).open(&path);

    let _ = LOG_PATH.set(path.clone());
    path
}

pub fn write(msg: &str) {
    let Some(path) = LOG_PATH.get() else {
        return;
    };
    let newline = if cfg!(windows) { "\r\n" } else { "\n" };
    let line = format!("[{}] {msg}{newline}", timestamp());
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(line.as_bytes());
    }
    #[cfg(debug_assertions)]
    eprintln!("{line}");
}

fn timestamp() -> String {
    chrono::Local::now().format("%H:%M:%S%.3f").to_string()
}
