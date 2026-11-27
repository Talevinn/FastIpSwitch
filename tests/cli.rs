// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Test d'intégration du binaire principal en mode `--print-adapters`.
//!
//! Ce mode retourne avant l'initialisation de Slint : il ne doit ouvrir aucune
//! fenêtre et se terminer seul. Le test l'exécute avec stdin fermé et un délai
//! maximal : un binaire qui attendrait un affichage ou une action utilisateur
//! serait tué et le test échouerait.
//!
//! Aucune modification réseau n'est effectuée : `--print-adapters` se contente
//! des lectures `ip -j`/`nmcli` du backend. Si `ip` est absent de
//! l'environnement, le binaire ne peut pas lire les interfaces : on vérifie
//! alors seulement qu'il se termine sans panic ni signal.

#![cfg(target_os = "linux")]

use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// Délai maximal accordé au binaire : bien au-delà du temps des lectures `ip`,
/// mais court devant un blocage sur une boucle d'événements Slint.
const TIMEOUT: Duration = Duration::from_secs(60);

/// Lance `fastipswitch --print-adapters` et borne son exécution dans le temps.
fn run_print_adapters() -> (ExitStatus, String, String) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fastipswitch"))
        .arg("--print-adapters")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("le binaire fastipswitch doit pouvoir être lancé");

    let deadline = Instant::now() + TIMEOUT;
    loop {
        match child.try_wait().expect("attente du binaire") {
            Some(_) => break,
            None if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!(
                    "--print-adapters ne s'est pas terminé en {} s : une fenêtre a-t-elle été ouverte ?",
                    TIMEOUT.as_secs()
                );
            }
            None => thread::sleep(Duration::from_millis(20)),
        }
    }

    let output = child.wait_with_output().expect("récupération de la sortie");
    (
        output.status,
        String::from_utf8_lossy(&output.stdout).into_owned(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn print_adapters_termine_sans_fenetre_ni_panic() {
    let (status, stdout, stderr) = run_print_adapters();

    assert!(
        !stderr.contains("panicked at"),
        "le binaire ne doit pas paniquer :\n{stderr}"
    );
    assert!(
        status.code().is_some(),
        "le binaire doit se terminer normalement, pas être tué par un signal : {status:?}"
    );

    // Sans `ip`, `read_adapters` ne peut rien lire : le backend remonte une
    // erreur et le binaire sort en erreur. C'est un environnement dégradé, pas
    // un défaut du mode `--print-adapters` : on n'exige pas le code 0.
    let ip_available = Command::new("ip")
        .arg("-V")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    if !ip_available {
        eprintln!(
            "`ip` absent : --print-adapters ne peut pas lire les interfaces, \
             code de sortie {:?} accepté",
            status.code()
        );
        return;
    }

    assert_eq!(
        status.code(),
        Some(0),
        "--print-adapters a échoué\nstdout: {stdout}\nstderr: {stderr}"
    );

    // Sortie vide acceptée (aucune interface exploitable), sinon une ligne par
    // interface avec des champs séparés par des tabulations.
    for (index, line) in stdout.lines().enumerate() {
        let fields: Vec<&str> = line.split('\t').collect();
        assert!(
            fields.len() >= 9,
            "ligne {index} : format tabulé attendu (9 champs), obtenu {line:?}"
        );
        assert!(
            !fields[0].is_empty(),
            "ligne {index} : nom d'interface vide"
        );
        for (position, prefix) in [
            (1, "index="),
            (2, "mac="),
            (3, "up="),
            (4, "dhcp="),
            (5, "gw="),
            (6, "dns="),
            (7, "conn="),
        ] {
            assert!(
                fields[position].starts_with(prefix),
                "ligne {index} : champ {position} attendu prefixé par {prefix:?}, obtenu {:?}",
                fields[position]
            );
        }
    }
}
