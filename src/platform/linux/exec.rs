// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::process::{Command, Stdio};

use crate::log;

/// Exécute une commande sans passer par un shell et journalise l'appel.
pub fn run(program: &str, args: &[&str]) -> Result<String, String> {
    run_inner(program, args, true)
}

/// Variante silencieuse (sondages fréquents : thème, état).
pub fn run_quiet(program: &str, args: &[&str]) -> Result<String, String> {
    run_inner(program, args, false)
}

fn run_inner(program: &str, args: &[&str], log_it: bool) -> Result<String, String> {
    if log_it {
        log::write(&format!("{program} {}", args.join(" ")));
    }

    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| crate::i18n::tr().command_failed(program, &e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let details = if stderr.is_empty() {
        stdout.clone()
    } else if stdout.is_empty() {
        stderr.clone()
    } else {
        format!("{stdout} {stderr}")
    };

    if output.status.success() {
        if log_it {
            log::write("  -> ok");
        }
        Ok(stdout)
    } else {
        let code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".into());
        if log_it {
            log::write(&format!("  -> échec ({code}) {details}"));
        }
        Err(crate::i18n::tr().command_failed(program, &format!("code {code} : {details}")))
    }
}
