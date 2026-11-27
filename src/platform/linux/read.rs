// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Lecture seule des interfaces IPv4 via `ip -j` et `nmcli`.
//!
//! Aucune commande de modification n'est exécutée ici : uniquement
//! `ip -j addr show`, `ip -j link show`, `ip -j route show default` et
//! des interrogations `nmcli` d'état. Toutes passent par `run_quiet` car
//! la lecture est refaite périodiquement (toutes les 2 s).

use std::net::Ipv4Addr;

use serde::Deserialize;

use super::exec::run_quiet;
use crate::cidr::Cidr;
use crate::platform::{AdapterInfo, IfaceId, IpEntry};

/// Préfixes d'interfaces virtuelles masquées (conteneurs, ponts locaux).
/// Les VPN (`tun*`, `wg*`, `ppp*`, …) sont volontairement conservés.
const HIDDEN_PREFIXES: &[&str] = &["docker", "veth", "virbr", "vmnet", "br-"];

// --- Structures intermédiaires (sortie de `ip -j`) ---

#[derive(Clone, Debug)]
struct LinkInfo {
    ifindex: u32,
    ifname: String,
    flags: Vec<String>,
    operstate: String,
    link_type: String,
    mac: String,
}

impl LinkInfo {
    /// Une interface est « up » si `operstate == "UP"` **et** que le drapeau
    /// `UP` est présent. On exige les deux : le drapeau seul reste vrai pour
    /// une interface sans porteuse (lien débranché, Wi-Fi non associé), tandis
    /// que `operstate` seul peut rapporter `UNKNOWN`. Limite connue : les
    /// interfaces `tun`/`wg` rapportent souvent `UNKNOWN` ; elles sont
    /// conservées dans la liste mais marquées down.
    fn is_up(&self) -> bool {
        self.operstate == "UP" && self.flags.iter().any(|flag| flag == "UP")
    }
}

/// Adresses IPv4 d'une interface, indexées par `ifindex`.
#[derive(Clone, Debug)]
struct IfaceAddrs {
    ifindex: u32,
    entries: Vec<IpEntry>,
}

#[derive(Clone, Debug)]
struct RouteInfo {
    dev: String,
    gateway: Ipv4Addr,
    metric: u32,
}

// --- Structures de désérialisation brutes ---

#[derive(Deserialize)]
struct RawLink {
    ifindex: u32,
    ifname: String,
    #[serde(default)]
    flags: Vec<String>,
    #[serde(default)]
    operstate: String,
    #[serde(default)]
    link_type: String,
    #[serde(default)]
    address: String,
}

#[derive(Deserialize)]
struct RawAddr {
    ifindex: u32,
    #[serde(default)]
    addr_info: Vec<RawAddrInfo>,
}

#[derive(Deserialize)]
struct RawAddrInfo {
    #[serde(default)]
    family: String,
    #[serde(default)]
    local: String,
    #[serde(default)]
    prefixlen: u8,
    #[serde(default)]
    dynamic: bool,
}

#[derive(Deserialize)]
struct RawRoute {
    #[serde(default)]
    dst: String,
    #[serde(default)]
    gateway: String,
    #[serde(default)]
    dev: String,
    #[serde(default)]
    metric: u32,
}

// --- Lecture ---

/// Lit les interfaces IPv4 de la machine.
///
/// Les échecs de `ip` sont fatals ; ceux de `nmcli` sont tolérés
/// (NetworkManager absent ou interface non gérée → `connection: None`,
/// `dns` vide).
pub fn read_adapters() -> Result<Vec<AdapterInfo>, String> {
    let addr_json = run_quiet("ip", &["-j", "addr", "show"])?;
    let link_json = run_quiet("ip", &["-j", "link", "show"])?;
    let route_json = run_quiet("ip", &["-j", "route", "show", "default"])?;

    let addrs = parse_addr_json(&addr_json)?;
    let links = parse_link_json(&link_json)?;
    let routes = parse_route_json(&route_json)?;

    let connections = run_quiet(
        "nmcli",
        &["-t", "-f", "UUID,DEVICE", "connection", "show", "--active"],
    )
    .map(|text| parse_nmcli_connections(&text))
    .unwrap_or_default();

    let mut adapters = build_adapters(&links, &addrs, &routes, &connections);

    // DNS géré par NetworkManager : une interrogation par interface retenue,
    // uniquement si une connexion active la gère (évite un appel par interface
    // virtuelle ou non gérée). En cas d'échec, la liste reste vide.
    for adapter in &mut adapters {
        if adapter.connection.is_none() {
            continue;
        }
        if let Ok(text) = run_quiet("nmcli", &["-g", "IP4.DNS", "device", "show", &adapter.name]) {
            adapter.dns = parse_nmcli_dns(&text);
        }
    }

    Ok(adapters)
}

// --- Parsing (testable séparément) ---

fn parse_link_json(text: &str) -> Result<Vec<LinkInfo>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let raw: Vec<RawLink> =
        serde_json::from_str(text).map_err(|e| format!("lecture ip link : JSON invalide : {e}"))?;
    Ok(raw
        .into_iter()
        .map(|link| LinkInfo {
            ifindex: link.ifindex,
            ifname: link.ifname,
            flags: link.flags,
            operstate: link.operstate,
            link_type: link.link_type,
            mac: link.address,
        })
        .collect())
}

fn parse_addr_json(text: &str) -> Result<Vec<IfaceAddrs>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let raw: Vec<RawAddr> =
        serde_json::from_str(text).map_err(|e| format!("lecture ip addr : JSON invalide : {e}"))?;

    let mut out = Vec::with_capacity(raw.len());
    for iface in raw {
        // Seul IPv4 est géré : les entrées inet6 sont ignorées.
        let mut entries = Vec::new();
        for info in iface.addr_info {
            if info.family != "inet" {
                continue;
            }
            let Ok(addr) = info.local.parse::<Ipv4Addr>() else {
                continue;
            };
            entries.push(IpEntry {
                cidr: Cidr {
                    addr,
                    prefix: info.prefixlen.min(32),
                },
                from_dhcp: info.dynamic,
            });
        }
        if !entries.is_empty() {
            out.push(IfaceAddrs {
                ifindex: iface.ifindex,
                entries,
            });
        }
    }
    Ok(out)
}

fn parse_route_json(text: &str) -> Result<Vec<RouteInfo>, String> {
    if text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let raw: Vec<RawRoute> = serde_json::from_str(text)
        .map_err(|e| format!("lecture ip route : JSON invalide : {e}"))?;

    Ok(raw
        .into_iter()
        .filter(|route| route.dst == "default")
        .filter_map(|route| {
            // Une route par défaut IPv6 a une passerelle non parsable en v4 :
            // elle est écartée silencieusement.
            let gateway = route.gateway.parse::<Ipv4Addr>().ok()?;
            Some(RouteInfo {
                dev: route.dev,
                gateway,
                metric: route.metric,
            })
        })
        .collect())
}

/// Analyse `nmcli -t -f UUID,DEVICE connection show --active`.
/// Retourne des couples `(uuid, device)`.
fn parse_nmcli_connections(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            // `nmcli -t` échappe les `:` par `\:` (noms d'interface MAC,
            // profils, etc.) : un simple split_once couperait le nom.
            let mut fields = split_nmcli_fields(line);
            if fields.len() < 2 {
                return None;
            }
            let device = fields.pop()?;
            let uuid = fields.pop()?;
            let uuid = uuid.trim();
            let device = device.trim();
            if uuid.is_empty() || device.is_empty() {
                None
            } else {
                Some((uuid.to_string(), device.to_string()))
            }
        })
        .collect()
}

/// Découpe une sortie `nmcli -t` en respectant les échappements `\:`.
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

/// Analyse `nmcli -g IP4.DNS device show <if>` : une adresse par ligne.
/// Les lignes vides et valeurs non IPv4 sont ignorées, les doublons retirés.
fn parse_nmcli_dns(text: &str) -> Vec<Ipv4Addr> {
    let mut servers = Vec::new();
    for raw in text.split(|c: char| c.is_whitespace() || c == ',' || c == ';') {
        let raw = raw.trim();
        if raw.is_empty() {
            continue;
        }
        if let Ok(ip) = raw.parse::<Ipv4Addr>()
            && !servers.contains(&ip)
        {
            servers.push(ip);
        }
    }
    servers
}

// --- Assemblage ---

/// Construit les adaptateurs à partir des données déjà analysées.
fn build_adapters(
    links: &[LinkInfo],
    addrs: &[IfaceAddrs],
    routes: &[RouteInfo],
    connections: &[(String, String)],
) -> Vec<AdapterInfo> {
    let mut adapters = Vec::new();

    for link in links {
        if is_hidden(link) {
            continue;
        }

        let mut addresses = addrs
            .iter()
            .find(|iface| iface.ifindex == link.ifindex)
            .map(|iface| iface.entries.clone())
            .unwrap_or_default();
        addresses.sort_by_key(|entry| u32::from(entry.cidr.addr));

        // Passerelle : route par défaut de plus faible métrique pour ce device.
        let gateway = routes
            .iter()
            .filter(|route| route.dev == link.ifname)
            .min_by_key(|route| route.metric)
            .map(|route| route.gateway);

        let connection = connections
            .iter()
            .find(|(_, device)| *device == link.ifname)
            .map(|(uuid, _)| uuid.clone());

        let dhcp = addresses.iter().any(|entry| entry.from_dhcp);

        adapters.push(AdapterInfo {
            name: link.ifname.clone(),
            description: description(link),
            mac: link.mac.clone(),
            id: IfaceId {
                index: link.ifindex,
                name: link.ifname.clone(),
            },
            connection,
            is_up: link.is_up(),
            dhcp,
            addresses,
            gateway,
            dns: Vec::new(),
        });
    }

    // Interfaces actives d'abord, puis ordre alphabétique (tri stable).
    adapters.sort_by(|a, b| b.is_up.cmp(&a.is_up).then_with(|| a.name.cmp(&b.name)));
    adapters
}

fn description(link: &LinkInfo) -> String {
    if link.ifname.is_empty() {
        link.link_type.clone()
    } else {
        link.ifname.clone()
    }
}

fn is_hidden(link: &LinkInfo) -> bool {
    if link.ifname == "lo" || link.link_type == "loopback" {
        return true;
    }
    HIDDEN_PREFIXES
        .iter()
        .any(|prefix| link.ifname.starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LINK_WIFI: &str = include_str!("../../../tests/fixtures/ip_link_wifi.json");
    const ADDR_WIFI: &str = include_str!("../../../tests/fixtures/ip_addr_wifi.json");
    const ROUTE_WIFI: &str = include_str!("../../../tests/fixtures/ip_route_default_wifi.json");
    const LINK_ETH: &str = include_str!("../../../tests/fixtures/ip_link_eth_static_alias.json");
    const ADDR_ETH: &str = include_str!("../../../tests/fixtures/ip_addr_eth_static_alias.json");
    const LINK_DOWN: &str = include_str!("../../../tests/fixtures/ip_link_down.json");
    const LINK_VIRTUALS: &str = include_str!("../../../tests/fixtures/ip_link_virtuals.json");
    const ADDR_VIRTUALS: &str = include_str!("../../../tests/fixtures/ip_addr_virtuals.json");

    fn wifi_links() -> Vec<LinkInfo> {
        parse_link_json(LINK_WIFI).unwrap()
    }

    fn wifi_addrs() -> Vec<IfaceAddrs> {
        parse_addr_json(ADDR_WIFI).unwrap()
    }

    fn wifi_routes() -> Vec<RouteInfo> {
        parse_route_json(ROUTE_WIFI).unwrap()
    }

    #[test]
    fn parse_link_lit_les_champs_utiles() {
        let links = wifi_links();
        let wifi = links
            .iter()
            .find(|link| link.ifname == "wlp3s0")
            .expect("wlp3s0 présente");
        assert_eq!(wifi.ifindex, 2);
        assert_eq!(wifi.mac, "6c:40:08:b0:15:24");
        assert_eq!(wifi.link_type, "ether");
        assert!(wifi.is_up());
        assert!(!description(wifi).is_empty());

        let lo = links.iter().find(|link| link.ifname == "lo").unwrap();
        assert!(!lo.is_up(), "loopback en UNKNOWN ne doit pas être up");
    }

    #[test]
    fn parse_link_tolere_vide() {
        assert!(parse_link_json("   ").unwrap().is_empty());
        assert!(parse_addr_json("").unwrap().is_empty());
        assert!(parse_route_json("").unwrap().is_empty());
    }

    #[test]
    fn parse_addr_ignore_inet6_et_marque_dhcp() {
        let addrs = wifi_addrs();
        let wifi = addrs
            .iter()
            .find(|iface| iface.ifindex == 2)
            .expect("adresses wlp3s0");
        assert_eq!(wifi.entries.len(), 1, "une seule adresse IPv4");
        let entry = &wifi.entries[0];
        assert_eq!(entry.cidr.to_string(), "192.168.1.18/24");
        assert!(entry.from_dhcp);

        // lo n'a qu'une adresse IPv4, conservée ici (le filtrage est plus tard).
        let lo = addrs.iter().find(|iface| iface.ifindex == 1).unwrap();
        assert_eq!(lo.entries.len(), 1);
        assert_eq!(lo.entries[0].cidr.to_string(), "127.0.0.1/8");
    }

    #[test]
    fn parse_route_garde_default_ipv4_et_metric() {
        let routes = wifi_routes();
        assert_eq!(routes.len(), 1);
        assert_eq!(routes[0].dev, "wlp3s0");
        assert_eq!(routes[0].gateway, Ipv4Addr::new(192, 168, 1, 1));
        assert_eq!(routes[0].metric, 600);

        // Route IPv6 : passerelle non v4, écartée.
        let v6 = r#"[{"dst":"default","gateway":"fe80::1","dev":"eth0","metric":100}]"#;
        assert!(parse_route_json(v6).unwrap().is_empty());

        // Route non-default ignorée.
        let other = r#"[{"dst":"192.168.2.0/24","gateway":"192.168.2.1","dev":"eth0"}]"#;
        assert!(parse_route_json(other).unwrap().is_empty());
    }

    #[test]
    fn gateway_prend_la_metrique_la_plus_faible() {
        let routes = parse_route_json(
            r#"[
                {"dst":"default","gateway":"192.168.1.254","dev":"wlp3s0","metric":900},
                {"dst":"default","gateway":"192.168.1.1","dev":"wlp3s0","metric":600},
                {"dst":"default","gateway":"10.1.0.1","dev":"eth1","metric":10}
            ]"#,
        )
        .unwrap();

        let adapters = build_adapters(&wifi_links(), &wifi_addrs(), &routes, &[]);
        let wifi = adapters.iter().find(|a| a.name == "wlp3s0").unwrap();
        assert_eq!(wifi.gateway, Some(Ipv4Addr::new(192, 168, 1, 1)));
    }

    #[test]
    fn assemble_wifi_avec_connexion_gateway_et_tri() {
        let connections = vec![(
            "1be4d962-28a0-4aac-a781-d2563059092b".to_string(),
            "wlp3s0".to_string(),
        )];
        let adapters = build_adapters(&wifi_links(), &wifi_addrs(), &wifi_routes(), &connections);

        assert_eq!(adapters.len(), 1, "loopback masquée");
        let wifi = &adapters[0];
        assert_eq!(wifi.name, "wlp3s0");
        assert_eq!(wifi.description, "wlp3s0");
        assert!(wifi.is_up);
        assert!(wifi.dhcp);
        assert_eq!(wifi.gateway, Some(Ipv4Addr::new(192, 168, 1, 1)));
        assert_eq!(
            wifi.connection.as_deref(),
            Some("1be4d962-28a0-4aac-a781-d2563059092b")
        );
        assert_eq!(wifi.id.index, 2);
        assert_eq!(wifi.id.name, "wlp3s0");
        assert_eq!(wifi.addresses.len(), 1);
        assert!(
            wifi.dns.is_empty(),
            "les DNS sont remplis dans read_adapters"
        );
    }

    #[test]
    fn ethernet_statique_avec_alias() {
        let links = parse_link_json(LINK_ETH).unwrap();
        let addrs = parse_addr_json(ADDR_ETH).unwrap();
        let adapters = build_adapters(&links, &addrs, &[], &[]);

        let eth = adapters.iter().find(|a| a.name == "eth0").unwrap();
        assert!(!eth.dhcp);
        assert_eq!(eth.addresses.len(), 2, "adresse principale + alias");
        assert_eq!(eth.addresses[0].cidr.to_string(), "192.168.3.10/24");
        assert_eq!(eth.addresses[1].cidr.to_string(), "192.168.3.11/24");
        assert!(eth.addresses.iter().all(|entry| !entry.from_dhcp));
        assert!(eth.is_up);
    }

    #[test]
    fn lien_down_est_conserve_mais_marque_down() {
        let links = parse_link_json(LINK_DOWN).unwrap();
        let adapters = build_adapters(&links, &[], &[], &[]);
        let eth1 = adapters.iter().find(|a| a.name == "eth1").unwrap();
        assert!(!eth1.is_up);
        assert!(eth1.addresses.is_empty());
        assert_eq!(eth1.gateway, None);
        assert!(!eth1.dhcp);
    }

    #[test]
    fn filtre_les_interfaces_virtuelles_et_garde_les_vpn() {
        let links = parse_link_json(LINK_VIRTUALS).unwrap();
        let addrs = parse_addr_json(ADDR_VIRTUALS).unwrap();
        let adapters = build_adapters(&links, &addrs, &[], &[]);
        let names: Vec<&str> = adapters.iter().map(|a| a.name.as_str()).collect();

        assert_eq!(names, vec!["eth0", "tun0", "wg0"]);
        assert!(adapters[0].is_up, "eth0 up d'abord");
        assert!(!adapters[1].is_up, "tun0 UNKNOWN");
        assert!(!adapters[2].is_up, "wg0 UNKNOWN");
    }

    #[test]
    fn tri_up_puis_alphabetique() {
        let links = parse_link_json(LINK_VIRTUALS).unwrap();
        let adapters = build_adapters(&links, &[], &[], &[]);
        let names: Vec<&str> = adapters.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, vec!["eth0", "tun0", "wg0"]);
    }

    #[test]
    fn parse_nmcli_dns_lignes_et_doublons() {
        assert_eq!(
            parse_nmcli_dns("192.168.1.1\n1.1.1.1\n192.168.1.1\n"),
            vec![Ipv4Addr::new(192, 168, 1, 1), Ipv4Addr::new(1, 1, 1, 1)]
        );
        assert!(parse_nmcli_dns("").is_empty());
        assert!(parse_nmcli_dns("\n  \n").is_empty());
        assert!(parse_nmcli_dns("pas-une-ip").is_empty());
    }

    #[test]
    fn parse_nmcli_connections_uuid_device() {
        let text = "1be4d962-28a0-4aac-a781-d2563059092b:wlp3s0\n\
                    675c4285-28d6-4939-95f4-270547280fde:lo\n";
        assert_eq!(
            parse_nmcli_connections(text),
            vec![
                (
                    "1be4d962-28a0-4aac-a781-d2563059092b".to_string(),
                    "wlp3s0".to_string()
                ),
                (
                    "675c4285-28d6-4939-95f4-270547280fde".to_string(),
                    "lo".to_string()
                ),
            ]
        );
        assert!(parse_nmcli_connections("").is_empty());
        assert!(parse_nmcli_connections(":eth0\nuuid:\n").is_empty());

        // `nmcli -t` échappe les `:` des noms d'interface (ex. MAC).
        assert_eq!(
            parse_nmcli_connections(
                "1be4d962-28a0-4aac-a781-d2563059092b:44\\:F2\\:1B\\:96\\:18\\:A3\n"
            ),
            vec![(
                "1be4d962-28a0-4aac-a781-d2563059092b".to_string(),
                "44:F2:1B:96:18:A3".to_string()
            )]
        );
    }
}
