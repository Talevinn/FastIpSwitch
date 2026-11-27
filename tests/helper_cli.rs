// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Tests d'intégration du helper privilégié, sans root et sans réseau.
//!
//! Chaque requête envoyée ici est refusée par la revalidation du helper : soit
//! le JSON est invalide, soit la version est inconnue, soit le nom d'interface
//! est interdit ou inexistant. Aucune opération `nmcli`/`ip`/`resolvectl` ne
//! doit donc être lancée. Ces tests vérifient aussi le contrat de sortie :
//! exactement un objet JSON sur stdout, rien d'autre (les journaux vont sur
//! stderr), et code de sortie 1 en cas d'échec.

#![cfg(target_os = "linux")]

use std::io::Write;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

/// Lance le binaire helper avec `input` sur stdin et attend sa fin.
fn run_helper(input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fastipswitch-helper"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("le helper doit pouvoir être lancé");

    child
        .stdin
        .take()
        .expect("stdin du helper")
        .write_all(input.as_bytes())
        .expect("écriture de la requête sur stdin");

    child
        .wait_with_output()
        .expect("attente de la fin du helper")
}

/// Vérifie qu'un refus respecte le contrat : code 1, un seul objet JSON
/// `{"ok":false,...}` sur stdout, et aucune commande système exécutée.
fn assert_refused(output: &Output, context: &str) -> Value {
    assert_eq!(
        output.status.code(),
        Some(1),
        "{context} : le helper doit échouer avec le code 1\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );

    let stdout = std::str::from_utf8(&output.stdout).expect("stdout UTF-8");
    assert_eq!(
        stdout.lines().count(),
        1,
        "{context} : stdout doit contenir exactement une ligne, obtenu {stdout:?}"
    );

    // `from_str` exige que toute la ligne soit un unique objet JSON : un
    // second objet ou un message de journal la ferait échouer.
    let response: Value = serde_json::from_str(stdout.trim()).unwrap_or_else(|e| {
        panic!("{context} : stdout n'est pas un unique objet JSON ({e}) : {stdout:?}")
    });

    assert_eq!(
        response.get("ok").and_then(Value::as_bool),
        Some(false),
        "{context} : la réponse doit porter \"ok\":false, obtenu {response}"
    );
    let message = response
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{context} : la réponse d'erreur doit contenir un message"));
    assert!(
        !message.trim().is_empty(),
        "{context} : le message d'erreur ne doit pas être vide"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !stderr.contains("exécution :"),
        "{context} : aucune commande réseau ne doit être lancée, stderr : {stderr:?}"
    );

    response
}

#[test]
fn json_invalide_est_refuse() {
    let output = run_helper("{ ceci n'est pas du JSON");
    let response = assert_refused(&output, "JSON invalide");
    let message = response["message"].as_str().unwrap();
    assert!(
        message.contains("JSON invalide"),
        "le message doit mentionner le JSON invalide : {message:?}"
    );
}

#[test]
fn version_inconnue_est_refusee() {
    let output = run_helper(r#"{"version":99,"op":"set_dhcp","ifname":"lo"}"#);
    let response = assert_refused(&output, "version inconnue");
    let message = response["message"].as_str().unwrap();
    assert!(
        message.contains("version"),
        "le message doit mentionner la version : {message:?}"
    );
}

#[test]
fn ifname_avec_traversee_de_chemin_est_refuse() {
    let output = run_helper(r#"{"version":1,"op":"set_dhcp","ifname":"../etc"}"#);
    let response = assert_refused(&output, "ifname ../etc");
    let message = response["message"].as_str().unwrap();
    assert!(
        message.contains("invalide"),
        "un nom hors /sys/class/net doit être invalide : {message:?}"
    );
}

#[test]
fn boucle_locale_est_refusee() {
    let output = run_helper(r#"{"version":1,"op":"set_dhcp","ifname":"lo"}"#);
    let response = assert_refused(&output, "ifname lo");
    let message = response["message"].as_str().unwrap();
    assert!(
        message.contains("autorisée"),
        "le message doit signaler une interface non autorisée : {message:?}"
    );
}

#[test]
fn interface_inexistante_est_refusee_avant_toute_execution() {
    // Requête bien formée et version valide : seul le nom d'interface est
    // refusé (absence de /sys/class/net/zz-no-such-if). Le helper ne doit
    // jamais atteindre `exec::execute`.
    let output = run_helper(r#"{"version":1,"op":"set_dhcp","ifname":"zz-no-such-if"}"#);
    let response = assert_refused(&output, "interface inexistante");
    let message = response["message"].as_str().unwrap();
    assert!(
        message.contains("inconnue"),
        "le message doit signaler une interface inconnue : {message:?}"
    );
}

#[test]
fn stdout_ne_contient_aucun_journal() {
    // Quel que soit le refus, stdout reste réservé au protocole JSON : les
    // diagnostics éventuels passent par stderr uniquement.
    for request in [
        "pas du json",
        r#"{"version":0,"op":"set_dhcp","ifname":"eth0"}"#,
        r#"{"version":1,"op":"set_dhcp","ifname":"-eth0"}"#,
        r#"{"version":1,"op":"reset_dns","ifname":"."}"#,
    ] {
        let output = run_helper(request);
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.contains("fastipswitch-helper:"),
            "stdout ne doit contenir aucun préfixe de journal : {stdout:?}"
        );
        assert_refused(&output, request);
    }
}
