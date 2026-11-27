// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Helper privilégié : reçoit une requête JSON sur stdin, la revalide, exécute
//! une opération réseau whitelistée et renvoie une réponse JSON sur stdout.

use std::io::Read;
use std::process::ExitCode;

use fastipswitch::helper::protocol::{Request, Response};
use fastipswitch::helper::{exec, validate};

fn main() -> ExitCode {
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        return reply(Response::error(format!("lecture de la requête : {e}")));
    }

    let request: Request = match serde_json::from_str(&input) {
        Ok(request) => request,
        Err(e) => return reply(Response::error(format!("requête JSON invalide : {e}"))),
    };

    if let Err(e) = validate::validate(&request) {
        return reply(Response::error(e));
    }

    match exec::execute(&request.op) {
        Ok(()) => reply(Response::ok()),
        Err(e) => reply(Response::error(e)),
    }
}

fn reply(response: Response) -> ExitCode {
    println!(
        "{}",
        serde_json::to_string(&response).unwrap_or_else(|_| r#"{"ok":false}"#.into())
    );
    if response.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
