// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use crate::helper::protocol::{Request, Response};
use crate::log;

/// Chemin du helper.
///
/// En release, seul l'emplacement système est accepté : une variable
/// d'environnement ou un binaire voisin choisis par l'utilisateur ne doivent
/// pas pouvoir détourner l'élévation pkexec. En debug, on autorise
/// `FASTIPSWITCH_HELPER` et le binaire voisin de `target/` pour le
/// développement.
fn helper_path() -> Result<PathBuf, String> {
    #[cfg(debug_assertions)]
    if let Some(path) = std::env::var_os("FASTIPSWITCH_HELPER") {
        let path = PathBuf::from(path);
        if path.is_file() {
            return Ok(path);
        }
        return Err(crate::i18n::tr().helper_missing());
    }

    let system = PathBuf::from("/usr/libexec/fastipswitch/fastipswitch-helper");
    if system.is_file() {
        return Ok(system);
    }

    #[cfg(debug_assertions)]
    if let Ok(exe) = std::env::current_exe()
        && let Some(dir) = exe.parent()
    {
        let sibling = dir.join("fastipswitch-helper");
        if sibling.is_file() {
            return Ok(sibling);
        }
    }

    Err(crate::i18n::tr().helper_missing())
}

/// Envoie une requête au helper et renvoie sa réponse.
pub fn call(request: &Request) -> Result<Response, String> {
    let payload = serde_json::to_vec(request)
        .map_err(|e| crate::i18n::tr().command_failed("helper", &e.to_string()))?;

    if std::env::var_os("FASTIPSWITCH_DRY_RUN").is_some() {
        log::write(&format!(
            "[dry-run] helper {}",
            String::from_utf8_lossy(&payload)
        ));
        return Ok(Response::ok());
    }

    let helper = helper_path()?;

    let mut command = if unsafe { libc::geteuid() } == 0 {
        Command::new(&helper)
    } else {
        let mut command = Command::new("pkexec");
        command.arg(&helper);
        command
    };

    log::write(&format!("pkexec {}", helper.display()));

    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                crate::i18n::tr().helper_missing()
            } else {
                crate::i18n::tr().command_failed("pkexec", &e.to_string())
            }
        })?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| "stdin du helper indisponible".to_string())?;
        stdin
            .write_all(&payload)
            .map_err(|e| crate::i18n::tr().command_failed("helper", &e.to_string()))?;
    }

    let output = child
        .wait_with_output()
        .map_err(|e| crate::i18n::tr().command_failed("helper", &e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if let Ok(response) = serde_json::from_str::<Response>(stdout.trim())
        && (response.ok || response.message.is_some())
    {
        if !response.ok {
            log::write(&format!(
                "helper -> échec : {}",
                response.message.as_deref().unwrap_or("?")
            ));
        }
        return Ok(response);
    }

    match output.status.code() {
        Some(126) => Err(crate::i18n::tr().auth_cancelled()),
        Some(127) => Err(crate::i18n::tr().auth_failed(127)),
        code => {
            let details = if stderr.trim().is_empty() {
                stdout.trim().to_string()
            } else {
                stderr.trim().to_string()
            };
            Err(crate::i18n::tr().command_failed(
                "helper",
                &format!(
                    "code {} : {details}",
                    code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
                ),
            ))
        }
    }
}
