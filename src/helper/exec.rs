// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Exécution réelle des opérations, une fois la requête revalidée.
//!
//! Aucune commande ne passe par un shell : `Command::new(programme)` avec des
//! arguments séparés. Les modifications sont temporaires : le helper ne
//! persiste aucun profil (les `nmcli device modify` ne touchent qu'à la
//! configuration active en mémoire, conformément à `nmcli(1)`).

use std::io::ErrorKind;
use std::net::Ipv4Addr;
use std::process::{Command, Output};

use serde::Deserialize;

use crate::cidr::Cidr;

use super::protocol::Operation;

const NMCLI: &str = "nmcli";
const IP: &str = "ip";
const RESOLVECTL: &str = "resolvectl";
const NETWORKCTL: &str = "networkctl";

/// Exécute une opération déjà validée. Le helper tourne en root.
pub fn execute(op: &Operation) -> Result<(), String> {
    match op {
        Operation::ApplyStatic {
            ifname,
            cidr,
            gateway,
            dns,
        } => apply_static(ifname, cidr, *gateway, dns),
        Operation::SetDhcp { ifname } => set_dhcp(ifname),
        Operation::SetDns { ifname, dns } => set_dns(ifname, dns),
        Operation::ResetDns { ifname } => reset_dns(ifname),
        Operation::AddAlias { ifname, cidr } => add_alias(ifname, cidr),
        Operation::RemoveAlias { ifname, cidr } => remove_alias(ifname, cidr),
    }
}

// ---------------------------------------------------------------------------
// Adresse principale
// ---------------------------------------------------------------------------

fn apply_static(
    ifname: &str,
    cidr: &Cidr,
    gateway: Option<Ipv4Addr>,
    dns: &[Ipv4Addr],
) -> Result<(), String> {
    if nm_connected(ifname) {
        // `device modify` ne modifie que la configuration active : le profil
        // NetworkManager sur disque n'est pas persisté.
        run_ok(NMCLI, &nmcli_apply_args(ifname, cidr, gateway))?;
    } else {
        run_ok(IP, &ip_flush_global_args(ifname))?;
        run_ok(IP, &ip_add_addr_args(ifname, cidr))?;
        if let Some(gateway) = gateway {
            run_ok(IP, &ip_route_replace_default_args(ifname, gateway))?;
        }
    }

    // Le protocole transporte aussi les DNS dans ApplyStatic, même si
    // l'interface les applique séparément via SetDns.
    if !dns.is_empty() {
        set_dns(ifname, dns)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// DHCP
// ---------------------------------------------------------------------------

fn set_dhcp(ifname: &str) -> Result<(), String> {
    if nm_connected(ifname) {
        // Réactiver le profil persistant efface les overrides temporaires
        // (adresse statique, passerelle) et relance DHCP s'il est configuré.
        let uuid = nm_active_connection(ifname)?;
        return run_ok(NMCLI, &nmcli_connection_up_args(&uuid)).map(|_| ());
    }

    run_ok(IP, &ip_flush_global_args(ifname))?;
    run_ok(IP, &ip_route_flush_default_args(ifname))?;
    if have_program(NETWORKCTL) {
        run_ok(NETWORKCTL, &networkctl_reconfigure_args(ifname))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// DNS
// ---------------------------------------------------------------------------

fn set_dns(ifname: &str, dns: &[Ipv4Addr]) -> Result<(), String> {
    if nm_connected(ifname) {
        return run_ok(NMCLI, &nmcli_dns_args(ifname, dns)).map(|_| ());
    }
    if have_program(RESOLVECTL) {
        return run_ok(RESOLVECTL, &resolvectl_dns_args(ifname, dns)).map(|_| ());
    }
    Err(format!(
        "resolvectl est absent : impossible de configurer les DNS de {ifname} \
         (interface non gérée par NetworkManager)"
    ))
}

fn reset_dns(ifname: &str) -> Result<(), String> {
    if nm_connected(ifname) {
        return run_ok(NMCLI, &nmcli_reset_dns_args(ifname)).map(|_| ());
    }
    if have_program(RESOLVECTL) {
        return run_ok(RESOLVECTL, &resolvectl_revert_args(ifname)).map(|_| ());
    }
    // Rien à réinitialiser sans resolvectl : opération idempotente.
    eprintln!("fastipswitch-helper: resolvectl absent, ResetDns sans effet sur {ifname}");
    Ok(())
}

// ---------------------------------------------------------------------------
// Alias
// ---------------------------------------------------------------------------

fn add_alias(ifname: &str, cidr: &Cidr) -> Result<(), String> {
    let args = ip_add_addr_args(ifname, cidr);
    match run(IP, &args)? {
        output if output.status.success() => Ok(()),
        output => {
            // Idempotence : l'adresse est peut-être déjà présente.
            if stderr_lower(&output).contains("file exists") {
                if address_present(ifname, cidr)? {
                    return Ok(());
                }
                return Err(format!(
                    "l'adresse {cidr} existe déjà sur {ifname} mais reste introuvable \
                     dans `ip addr`"
                ));
            }
            Err(command_error(IP, &args, &output))
        }
    }
}

fn remove_alias(ifname: &str, cidr: &Cidr) -> Result<(), String> {
    let args = ip_del_addr_args(ifname, cidr);
    match run(IP, &args)? {
        output if output.status.success() => Ok(()),
        output => {
            // Idempotence : supprimer une adresse absente n'est pas une erreur.
            let stderr = stderr_lower(&output);
            if stderr.contains("cannot assign requested address")
                || stderr.contains("not found")
                || stderr.contains("no such process")
            {
                return Ok(());
            }
            Err(command_error(IP, &args, &output))
        }
    }
}

/// Vérifie la présence effective d'une adresse via `ip -j addr`.
fn address_present(ifname: &str, cidr: &Cidr) -> Result<bool, String> {
    let output = run_ok(IP, &ip_show_addr_args(ifname))?;
    let json = String::from_utf8_lossy(&output.stdout);
    ip_json_has_address(&json, cidr)
}

// ---------------------------------------------------------------------------
// Détection NetworkManager
// ---------------------------------------------------------------------------

/// Une interface est considérée gérée par NetworkManager lorsqu'elle est
/// listée `connected` par `nmcli device status`.
fn nm_connected(ifname: &str) -> bool {
    if !have_program(NMCLI) {
        return false;
    }
    match run(NMCLI, &nmcli_device_status_args()) {
        Ok(output) if output.status.success() => {
            device_is_connected(&String::from_utf8_lossy(&output.stdout), ifname)
        }
        Ok(output) => {
            eprintln!(
                "fastipswitch-helper: nmcli device status a échoué ({}), \
                 basculement sur ip/resolvectl",
                stderr_lower(&output)
            );
            false
        }
        Err(e) => {
            eprintln!("fastipswitch-helper: {e}, basculement sur ip/resolvectl");
            false
        }
    }
}

/// UUID de la connexion NetworkManager active pour l'interface.
fn nm_active_connection(ifname: &str) -> Result<String, String> {
    let output = run_ok(NMCLI, &nmcli_active_connections_args())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_active_connection_uuid(&stdout, ifname)
        .ok_or_else(|| format!("aucune connexion NetworkManager active pour {ifname}"))
}

// ---------------------------------------------------------------------------
// Construction des arguments (fonctions pures, testées unitairement)
// ---------------------------------------------------------------------------

fn nmcli_device_status_args() -> Vec<String> {
    strings(&["-t", "-f", "DEVICE,STATE", "device", "status"])
}

fn nmcli_active_connections_args() -> Vec<String> {
    strings(&["-t", "-f", "UUID,DEVICE", "connection", "show", "--active"])
}

fn nmcli_apply_args(ifname: &str, cidr: &Cidr, gateway: Option<Ipv4Addr>) -> Vec<String> {
    let mut args = strings(&["device", "modify", ifname, "ipv4.method", "manual"]);
    args.push("ipv4.addresses".into());
    args.push(cidr.to_string());
    args.push("ipv4.gateway".into());
    // Une chaîne vide efface explicitement l'ancienne passerelle du profil
    // actif : sans elle, nmcli conserverait la valeur précédente.
    args.push(gateway.map(|g| g.to_string()).unwrap_or_default());
    args
}

fn nmcli_dns_args(ifname: &str, dns: &[Ipv4Addr]) -> Vec<String> {
    let servers = dns
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(" ");
    vec![
        "device".into(),
        "modify".into(),
        ifname.into(),
        "ipv4.dns".into(),
        servers,
        "ipv4.ignore-auto-dns".into(),
        "yes".into(),
    ]
}

fn nmcli_reset_dns_args(ifname: &str) -> Vec<String> {
    vec![
        "device".into(),
        "modify".into(),
        ifname.into(),
        "ipv4.dns".into(),
        String::new(),
        "ipv4.ignore-auto-dns".into(),
        "no".into(),
    ]
}

fn nmcli_connection_up_args(uuid: &str) -> Vec<String> {
    strings(&["connection", "up", uuid])
}

fn ip_flush_global_args(ifname: &str) -> Vec<String> {
    strings(&["-4", "addr", "flush", "dev", ifname, "scope", "global"])
}

fn ip_add_addr_args(ifname: &str, cidr: &Cidr) -> Vec<String> {
    let cidr = cidr.to_string();
    strings(&["-4", "addr", "add", &cidr, "dev", ifname])
}

fn ip_del_addr_args(ifname: &str, cidr: &Cidr) -> Vec<String> {
    let cidr = cidr.to_string();
    strings(&["-4", "addr", "del", &cidr, "dev", ifname])
}

fn ip_show_addr_args(ifname: &str) -> Vec<String> {
    strings(&["-j", "-4", "addr", "show", "dev", ifname])
}

fn ip_route_replace_default_args(ifname: &str, gateway: Ipv4Addr) -> Vec<String> {
    let gateway = gateway.to_string();
    strings(&[
        "route", "replace", "default", "via", &gateway, "dev", ifname,
    ])
}

fn ip_route_flush_default_args(ifname: &str) -> Vec<String> {
    strings(&["route", "flush", "default", "dev", ifname])
}

fn resolvectl_dns_args(ifname: &str, dns: &[Ipv4Addr]) -> Vec<String> {
    let mut args = vec!["dns".to_string(), ifname.to_string()];
    args.extend(dns.iter().map(ToString::to_string));
    args
}

fn resolvectl_revert_args(ifname: &str) -> Vec<String> {
    strings(&["revert", ifname])
}

fn networkctl_reconfigure_args(ifname: &str) -> Vec<String> {
    strings(&["reconfigure", ifname])
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|s| (*s).to_string()).collect()
}

// ---------------------------------------------------------------------------
// Analyse des sorties (fonctions pures)
// ---------------------------------------------------------------------------

/// Découpe une ligne `nmcli -t` en respectant les échappements (`\:`).
fn split_nmcli_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars();

    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(escaped) => current.push(escaped),
                None => current.push('\\'),
            },
            ':' => fields.push(std::mem::take(&mut current)),
            _ => current.push(c),
        }
    }
    fields.push(current);
    fields
}

/// Vrai si `nmcli device status` signale l'interface comme `connected`.
fn device_is_connected(status: &str, ifname: &str) -> bool {
    status.lines().any(|line| {
        let fields = split_nmcli_fields(line);
        fields.first().is_some_and(|device| device == ifname)
            && fields.get(1).is_some_and(|state| state == "connected")
    })
}

/// Extrait l'UUID de la connexion active associée à l'interface.
fn parse_active_connection_uuid(active: &str, ifname: &str) -> Option<String> {
    active.lines().find_map(|line| {
        let fields = split_nmcli_fields(line);
        let uuid = fields.first()?;
        let device = fields.get(1)?;
        (device == ifname && !uuid.is_empty()).then(|| uuid.clone())
    })
}

#[derive(Deserialize)]
struct IpLink {
    #[serde(default)]
    addr_info: Vec<IpAddrInfo>,
}

#[derive(Deserialize)]
struct IpAddrInfo {
    #[serde(default)]
    family: String,
    /// Adresse sous forme texte : la sortie peut aussi contenir de l'IPv6.
    local: Option<String>,
    #[serde(default)]
    prefixlen: u8,
}

/// Vrai si la sortie de `ip -j -4 addr` contient exactement ce CIDR.
fn ip_json_has_address(json: &str, cidr: &Cidr) -> Result<bool, String> {
    let links: Vec<IpLink> = serde_json::from_str(json)
        .map_err(|e| format!("sortie de `ip -j addr` illisible : {e}"))?;

    Ok(links.iter().any(|link| {
        link.addr_info.iter().any(|info| {
            info.family == "inet"
                && info.local.as_deref().and_then(|s| s.parse().ok()) == Some(cidr.addr)
                && info.prefixlen == cidr.prefix
        })
    }))
}

// ---------------------------------------------------------------------------
// Lancement de commandes
// ---------------------------------------------------------------------------

/// Lance un programme sans shell, en journalisant la ligne sur stderr.
fn run(program: &str, args: &[String]) -> Result<Output, String> {
    eprintln!(
        "fastipswitch-helper: exécution : {program} {}",
        args.join(" ")
    );
    Command::new(program)
        .args(args)
        .output()
        .map_err(|e| match e.kind() {
            ErrorKind::NotFound => format!("programme introuvable : {program}"),
            _ => format!("impossible de lancer {program} : {e}"),
        })
}

/// Comme [`run`], mais transforme un code de sortie non nul en erreur lisible.
fn run_ok(program: &str, args: &[String]) -> Result<Output, String> {
    let output = run(program, args)?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(command_error(program, args, &output))
    }
}

fn command_error(program: &str, args: &[String], output: &Output) -> String {
    let stderr = stderr_lower(output);
    let details = if stderr.is_empty() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stdout = stdout.trim();
        if stdout.is_empty() {
            "aucun détail".to_string()
        } else {
            truncate(stdout, 400)
        }
    } else {
        truncate(&stderr, 400)
    };

    let code = output
        .status
        .code()
        .map(|c| c.to_string())
        .unwrap_or_else(|| "?".into());
    format!(
        "{program} {} a échoué (code {code}) : {details}",
        args.join(" ")
    )
}

fn stderr_lower(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr)
        .trim()
        .to_lowercase()
}

fn truncate(text: &str, max_chars: usize) -> String {
    let mut result: String = text.chars().take(max_chars).collect();
    if text.chars().count() > max_chars {
        result.push('…');
    }
    result
}

/// Présence d'un programme dans `PATH` (sans l'exécuter).
fn have_program(name: &str) -> bool {
    std::env::var_os("PATH")
        .map(|paths| {
            std::env::split_paths(&paths).any(|dir| {
                let candidate = dir.join(name);
                candidate.is_file()
            })
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cidr(text: &str) -> Cidr {
        Cidr::parse(text).unwrap()
    }

    fn ip(text: &str) -> Ipv4Addr {
        text.parse().unwrap()
    }

    #[test]
    fn nmcli_apply_args_with_gateway() {
        assert_eq!(
            nmcli_apply_args("eth0", &cidr("192.168.3.10/24"), Some(ip("192.168.3.1"))),
            vec![
                "device",
                "modify",
                "eth0",
                "ipv4.method",
                "manual",
                "ipv4.addresses",
                "192.168.3.10/24",
                "ipv4.gateway",
                "192.168.3.1",
            ]
        );
    }

    #[test]
    fn nmcli_apply_args_clears_gateway_when_absent() {
        let args = nmcli_apply_args("eth0", &cidr("192.168.3.10/24"), None);
        assert_eq!(args.last().map(String::as_str), Some(""));
        // `ipv4.gateway` est bien suivi d'une valeur vide.
        let position = args.iter().position(|a| a == "ipv4.gateway").unwrap();
        assert_eq!(args[position + 1], "");
    }

    #[test]
    fn nmcli_dns_args_joins_servers() {
        assert_eq!(
            nmcli_dns_args("wlp3s0", &[ip("1.1.1.1"), ip("8.8.8.8")]),
            vec![
                "device",
                "modify",
                "wlp3s0",
                "ipv4.dns",
                "1.1.1.1 8.8.8.8",
                "ipv4.ignore-auto-dns",
                "yes",
            ]
        );
    }

    #[test]
    fn nmcli_reset_dns_args_clears_servers() {
        assert_eq!(
            nmcli_reset_dns_args("eth0"),
            vec![
                "device",
                "modify",
                "eth0",
                "ipv4.dns",
                "",
                "ipv4.ignore-auto-dns",
                "no",
            ]
        );
    }

    #[test]
    fn ip_args_are_separate_tokens() {
        let alias = cidr("192.168.3.20/24");
        assert_eq!(
            ip_add_addr_args("eth0", &alias),
            vec!["-4", "addr", "add", "192.168.3.20/24", "dev", "eth0"]
        );
        assert_eq!(
            ip_del_addr_args("eth0", &alias),
            vec!["-4", "addr", "del", "192.168.3.20/24", "dev", "eth0"]
        );
        assert_eq!(
            ip_flush_global_args("eth0"),
            vec!["-4", "addr", "flush", "dev", "eth0", "scope", "global"]
        );
    }

    #[test]
    fn route_args_target_the_interface() {
        assert_eq!(
            ip_route_replace_default_args("eth0", ip("192.168.3.1")),
            vec![
                "route",
                "replace",
                "default",
                "via",
                "192.168.3.1",
                "dev",
                "eth0",
            ]
        );
        assert_eq!(
            ip_route_flush_default_args("eth0"),
            vec!["route", "flush", "default", "dev", "eth0"]
        );
    }

    #[test]
    fn resolvectl_and_networkctl_args() {
        assert_eq!(
            resolvectl_dns_args("eth0", &[ip("9.9.9.9"), ip("1.1.1.1")]),
            vec!["dns", "eth0", "9.9.9.9", "1.1.1.1"]
        );
        assert_eq!(resolvectl_revert_args("eth0"), vec!["revert", "eth0"]);
        assert_eq!(
            networkctl_reconfigure_args("eth0"),
            vec!["reconfigure", "eth0"]
        );
    }

    #[test]
    fn nmcli_fields_unescape_colons() {
        assert_eq!(
            split_nmcli_fields(r"wlan0:connected:Mon\: réseau"),
            vec!["wlan0", "connected", "Mon: réseau"]
        );
    }

    #[test]
    fn detects_connected_device() {
        let status = "wlan0:disconnected:\neth0:connected:Wired\nlo:unmanaged:\n";
        assert!(device_is_connected(status, "eth0"));
        assert!(!device_is_connected(status, "wlan0"));
        assert!(!device_is_connected(status, "lo"));
        assert!(!device_is_connected(status, "eth1"));
    }

    #[test]
    fn extracts_active_connection_uuid() {
        let active = "0f0e-1234:eth0:802-3-ethernet\nabcd-5678:wlan0:802-11-wireless\n";
        assert_eq!(
            parse_active_connection_uuid(active, "eth0").as_deref(),
            Some("0f0e-1234")
        );
        assert_eq!(parse_active_connection_uuid(active, "enp0s3"), None);
    }

    #[test]
    fn reads_addresses_from_ip_json() {
        let json = r#"[
            {"ifname":"eth0","addr_info":[
                {"family":"inet","local":"192.168.3.10","prefixlen":24},
                {"family":"inet6","local":"fe80::1","prefixlen":64}
            ]}
        ]"#;

        assert!(ip_json_has_address(json, &cidr("192.168.3.10/24")).unwrap());
        assert!(!ip_json_has_address(json, &cidr("192.168.3.10/16")).unwrap());
        assert!(!ip_json_has_address(json, &cidr("192.168.3.20/24")).unwrap());
        assert!(!ip_json_has_address("[]", &cidr("192.168.3.10/24")).unwrap());
        assert!(ip_json_has_address("pas du json", &cidr("192.168.3.10/24")).is_err());
    }

    #[test]
    fn detects_program_in_path() {
        // `sh` existe sur toute machine Unix de développement.
        assert!(have_program("sh"));
        assert!(!have_program("programme-qui-nexiste-pas-fastipswitch"));
    }
}
