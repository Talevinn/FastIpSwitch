// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Revalidation complète des requêtes reçues par le helper.
//!
//! Le helper tourne en root : il ne fait jamais confiance à l'appelant, même
//! lorsque la requête vient de l'interface officielle. Toute valeur est
//! revérifiée avant d'atteindre une commande réseau.

use std::net::Ipv4Addr;
use std::path::Path;

use crate::cidr::Cidr;

use super::protocol::{Operation, Request, VERSION};

/// Longueur maximale d'un nom d'interface (`IFNAMSIZ - 1`).
const IFNAME_MAX: usize = 15;

/// Nombre maximal de serveurs DNS acceptés dans une requête.
const DNS_MAX: usize = 4;

/// Interfaces virtuelles masquées par l'interface : les accepter côté helper
/// permettrait de casser un pont Docker/libvirt via une requête forgée.
const HIDDEN_PREFIXES: &[&str] = &["docker", "veth", "virbr", "vmnet", "br-"];

impl Operation {
    pub fn ifname(&self) -> &str {
        match self {
            Operation::ApplyStatic { ifname, .. }
            | Operation::SetDhcp { ifname }
            | Operation::SetDns { ifname, .. }
            | Operation::ResetDns { ifname }
            | Operation::AddAlias { ifname, .. }
            | Operation::RemoveAlias { ifname, .. } => ifname,
        }
    }
}

/// Revalide intégralement une requête reçue par le helper.
pub fn validate(request: &Request) -> Result<(), String> {
    if request.version != VERSION {
        return Err(format!(
            "version de protocole non prise en charge : {} (attendu {VERSION})",
            request.version
        ));
    }

    validate_ifname(request.op.ifname())?;
    validate_operation(&request.op)
}

/// Valide les paramètres d'une opération, une fois le nom d'interface vérifié.
pub fn validate_operation(op: &Operation) -> Result<(), String> {
    match op {
        Operation::ApplyStatic {
            cidr, gateway, dns, ..
        } => {
            validate_cidr(cidr)?;
            if let Some(gateway) = gateway {
                validate_gateway(cidr, *gateway)?;
            }
            if !dns.is_empty() {
                validate_dns(dns)?;
            }
            Ok(())
        }
        Operation::SetDhcp { .. } | Operation::ResetDns { .. } => Ok(()),
        Operation::SetDns { dns, .. } => validate_dns(dns),
        Operation::AddAlias { cidr, .. } | Operation::RemoveAlias { cidr, .. } => {
            validate_cidr(cidr)
        }
    }
}

/// Le nom d'interface doit désigner une interface noyau existante et ne doit
/// jamais permettre de sortir de `/sys/class/net`, d'injecter un argument ou
/// de viser l'interface de bouclage.
pub fn validate_ifname(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("nom d'interface vide".into());
    }
    if name.len() > IFNAME_MAX {
        return Err(format!("nom d'interface trop long : {name}"));
    }
    if name == "lo" || name == "." || name == ".." {
        return Err(format!("interface non autorisée : {name}"));
    }
    if HIDDEN_PREFIXES
        .iter()
        .any(|prefix| name.starts_with(prefix))
    {
        return Err(format!("interface virtuelle non autorisée : {name}"));
    }

    // Le premier caractère ne peut pas être un option (`-`) ni un séparateur
    // de chemin (`.`), même si l'interface existe : on refuse par principe.
    let mut chars = name.chars();
    let first = chars.next().unwrap_or_default();
    if !first.is_ascii_alphanumeric() {
        return Err(format!("nom d'interface invalide : {name}"));
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | '@')) {
        return Err(format!("nom d'interface invalide : {name}"));
    }

    if !Path::new("/sys/class/net").join(name).exists() {
        return Err(format!("interface inconnue : {name}"));
    }
    Ok(())
}

/// Reconstruit le CIDR pour garantir ses invariants (préfixe, adresse réseau,
/// broadcast, plages réservées), même si la structure a été forgée à la main.
pub fn validate_cidr(cidr: &Cidr) -> Result<(), String> {
    Cidr::new(cidr.addr, cidr.prefix).map_err(|e| format!("CIDR invalide : {e}"))?;
    Ok(())
}

/// La passerelle doit être une adresse unicast du même sous-réseau que
/// l'adresse principale, distincte de celle-ci, du réseau et du broadcast.
pub fn validate_gateway(cidr: &Cidr, gateway: Ipv4Addr) -> Result<(), String> {
    if !is_unicast(gateway) {
        return Err(format!("passerelle invalide : {gateway}"));
    }
    if gateway == cidr.addr {
        return Err(format!(
            "la passerelle ne peut pas être l'adresse locale {gateway}"
        ));
    }
    if cidr.prefix <= 30 {
        if gateway == cidr.network() {
            return Err(format!(
                "la passerelle ne peut pas être l'adresse réseau {}",
                cidr.network()
            ));
        }
        if gateway == cidr.broadcast() {
            return Err(format!(
                "la passerelle ne peut pas être l'adresse de broadcast {}",
                cidr.broadcast()
            ));
        }
    }
    if !cidr.contains(gateway) {
        return Err(format!(
            "la passerelle {gateway} n'appartient pas au sous-réseau {}",
            cidr.network()
        ));
    }
    Ok(())
}

/// Valide une liste de serveurs DNS : non vide, bornée, uniquement des
/// adresses IPv4 unicast utilisables.
pub fn validate_dns(dns: &[Ipv4Addr]) -> Result<(), String> {
    if dns.is_empty() {
        return Err("liste de serveurs DNS vide".into());
    }
    if dns.len() > DNS_MAX {
        return Err(format!(
            "trop de serveurs DNS : {} (maximum {DNS_MAX})",
            dns.len()
        ));
    }
    for &server in dns {
        if !is_unicast(server) {
            return Err(format!("serveur DNS invalide : {server}"));
        }
    }
    Ok(())
}

/// Adresse IPv4 unicast exploitable comme hôte, passerelle ou DNS.
fn is_unicast(ip: Ipv4Addr) -> bool {
    !ip.is_unspecified()
        && !ip.is_loopback()
        && !ip.is_multicast()
        && !ip.is_broadcast()
        && ip.octets()[0] != 0
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
    fn rejects_dangerous_names() {
        let long = "a".repeat(16);
        for name in [
            "",
            "lo",
            "..",
            "../etc",
            "eth0/../../x",
            "eth 0",
            "-eth0",
            ".eth0",
            "@eth0",
            "docker0",
            "veth1234",
            long.as_str(),
        ] {
            assert!(
                validate_ifname(name).is_err(),
                "{name:?} aurait dû être refusé"
            );
        }
    }

    #[test]
    fn rejects_wrong_protocol_version() {
        let request = Request {
            version: 2,
            op: Operation::SetDhcp {
                ifname: "eth0".into(),
            },
        };
        assert!(validate(&request).is_err());
    }

    #[test]
    fn rejects_network_address_as_cidr() {
        let forged = Cidr {
            addr: ip("192.168.1.0"),
            prefix: 24,
        };
        assert!(validate_cidr(&forged).is_err());

        let broadcast = Cidr {
            addr: ip("192.168.1.255"),
            prefix: 24,
        };
        assert!(validate_cidr(&broadcast).is_err());

        let loopback = Cidr {
            addr: ip("127.0.0.1"),
            prefix: 8,
        };
        assert!(validate_cidr(&loopback).is_err());
    }

    #[test]
    fn accepts_valid_gateway() {
        let c = cidr("192.168.3.10/24");
        assert!(validate_gateway(&c, ip("192.168.3.1")).is_ok());
    }

    #[test]
    fn rejects_out_of_subnet_gateway() {
        let c = cidr("192.168.3.10/24");
        assert!(validate_gateway(&c, ip("192.168.4.1")).is_err());
        assert!(validate_gateway(&c, ip("10.0.0.1")).is_err());
    }

    #[test]
    fn rejects_special_gateways() {
        let c = cidr("192.168.3.10/24");
        // Adresse locale, réseau, broadcast.
        assert!(validate_gateway(&c, ip("192.168.3.10")).is_err());
        assert!(validate_gateway(&c, ip("192.168.3.0")).is_err());
        assert!(validate_gateway(&c, ip("192.168.3.255")).is_err());
        // Adresses non unicast.
        assert!(validate_gateway(&c, ip("0.0.0.0")).is_err());
        assert!(validate_gateway(&c, ip("224.0.0.1")).is_err());
        assert!(validate_gateway(&c, ip("255.255.255.255")).is_err());
    }

    #[test]
    fn rejects_empty_dns_list() {
        assert!(validate_dns(&[]).is_err());
    }

    #[test]
    fn rejects_unusable_dns() {
        for server in ["0.0.0.0", "224.0.0.1", "239.255.255.250", "255.255.255.255"] {
            assert!(
                validate_dns(&[ip(server)]).is_err(),
                "DNS {server} aurait dû être refusé"
            );
        }
    }

    #[test]
    fn accepts_valid_dns() {
        assert!(validate_dns(&[ip("1.1.1.1"), ip("8.8.8.8")]).is_ok());
    }

    #[test]
    fn rejects_too_many_dns_servers() {
        let servers = vec![ip("1.1.1.1"); DNS_MAX + 1];
        assert!(validate_dns(&servers).is_err());
    }

    #[test]
    fn validates_operations_without_touching_the_network() {
        // Aucun accès sysfs ici : on valide directement les paramètres.
        let op = Operation::ApplyStatic {
            ifname: "eth0".into(),
            cidr: cidr("192.168.3.10/24"),
            gateway: Some(ip("192.168.3.1")),
            dns: vec![ip("1.1.1.1")],
        };
        assert!(validate_operation(&op).is_ok());

        let bad_gateway = Operation::ApplyStatic {
            ifname: "eth0".into(),
            cidr: cidr("192.168.3.10/24"),
            gateway: Some(ip("192.168.9.1")),
            dns: vec![],
        };
        assert!(validate_operation(&bad_gateway).is_err());

        let bad_dns = Operation::SetDns {
            ifname: "eth0".into(),
            dns: vec![ip("0.0.0.0")],
        };
        assert!(validate_operation(&bad_dns).is_err());

        let empty_dns = Operation::SetDns {
            ifname: "eth0".into(),
            dns: vec![],
        };
        assert!(validate_operation(&empty_dns).is_err());
    }
}
