// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Client des opérations réseau Linux.
//!
//! Ce module ne touche jamais directement au réseau : il construit une
//! requête du protocole figé (`crate::helper::protocol`) et la confie au
//! helper privilégié via `super::helper::call`. Toute la validation réelle
//! (nmcli/ip) appartient au helper, qui revalide la requête.

use std::net::Ipv4Addr;

use crate::cidr::Cidr;
use crate::helper::protocol::{Operation, Request, Response};
use crate::platform::IfaceId;

/// Message utilisé quand le helper signale un échec sans détail.
const FAILED_WITHOUT_DETAIL: &str = "le helper a échoué sans fournir de détail";

/// Construit la requête d'application d'une adresse statique.
///
/// Les DNS ne sont volontairement pas inclus : `set_dns_static` les applique
/// séparément, ce qui évite qu'un échec DNS fasse échouer l'adressage.
fn request_for_static(id: &IfaceId, cidr: &Cidr, gateway: Option<Ipv4Addr>) -> Request {
    Request::new(Operation::ApplyStatic {
        ifname: id.name.clone(),
        cidr: *cidr,
        gateway,
        dns: Vec::new(),
    })
}

fn request_for_dhcp(id: &IfaceId) -> Request {
    Request::new(Operation::SetDhcp {
        ifname: id.name.clone(),
    })
}

fn request_for_reset_dns(id: &IfaceId) -> Request {
    Request::new(Operation::ResetDns {
        ifname: id.name.clone(),
    })
}

fn request_for_dns(id: &IfaceId, servers: &[Ipv4Addr]) -> Request {
    Request::new(Operation::SetDns {
        ifname: id.name.clone(),
        dns: servers.to_vec(),
    })
}

fn request_for_add_alias(id: &IfaceId, cidr: &Cidr) -> Request {
    Request::new(Operation::AddAlias {
        ifname: id.name.clone(),
        cidr: *cidr,
    })
}

/// La suppression porte le préfixe : le helper en a besoin pour construire
/// `ip addr del <adresse>/<préfixe> dev <interface>`.
fn request_for_remove_alias(id: &IfaceId, cidr: &Cidr) -> Request {
    Request::new(Operation::RemoveAlias {
        ifname: id.name.clone(),
        cidr: *cidr,
    })
}

/// Traduit la réponse du helper en résultat.
fn response_to_result(response: Response) -> Result<(), String> {
    if response.ok {
        Ok(())
    } else {
        Err(response
            .message
            .unwrap_or_else(|| FAILED_WITHOUT_DETAIL.to_string()))
    }
}

/// Envoie une opération au helper et remonte son éventuel message d'erreur.
///
/// `FASTIPSWITCH_DRY_RUN` est pris en charge par `super::helper::call` : en
/// mode simulation, aucune commande système n'est lancée.
fn send(op: Operation) -> Result<(), String> {
    let request = Request::new(op);
    let response = super::helper::call(&request)?;
    response_to_result(response)
}

pub fn set_static(id: &IfaceId, cidr: &Cidr, gateway: Option<Ipv4Addr>) -> Result<(), String> {
    send(request_for_static(id, cidr, gateway).op)
}

pub fn set_dhcp(id: &IfaceId) -> Result<(), String> {
    send(request_for_dhcp(id).op)
}

pub fn reset_dns_dhcp(id: &IfaceId) -> Result<(), String> {
    send(request_for_reset_dns(id).op)
}

pub fn set_dns_static(id: &IfaceId, servers: &[Ipv4Addr]) -> Result<(), String> {
    // Liste vide : rien à appliquer, aucun appel au helper.
    if servers.is_empty() {
        return Ok(());
    }
    send(request_for_dns(id, servers).op)
}

pub fn add_address(id: &IfaceId, cidr: &Cidr) -> Result<(), String> {
    send(request_for_add_alias(id, cidr).op)
}

pub fn delete_address(id: &IfaceId, cidr: &Cidr) -> Result<(), String> {
    send(request_for_remove_alias(id, cidr).op)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::helper::protocol::VERSION;

    fn iface() -> IfaceId {
        IfaceId {
            index: 3,
            name: "wlp3s0".into(),
        }
    }

    fn cidr(text: &str) -> Cidr {
        Cidr::parse(text).unwrap()
    }

    /// Repasse par le JSON pour vérifier le contrat réellement transmis.
    fn roundtrip(request: Request) -> Request {
        let json = serde_json::to_string(&request).unwrap();
        serde_json::from_str(&json).unwrap()
    }

    #[test]
    fn static_request_has_no_dns() {
        let request = roundtrip(request_for_static(
            &iface(),
            &cidr("192.168.3.10/24"),
            Some(Ipv4Addr::new(192, 168, 3, 1)),
        ));

        assert_eq!(request.version, VERSION);
        match request.op {
            Operation::ApplyStatic {
                ifname,
                cidr,
                gateway,
                dns,
            } => {
                assert_eq!(ifname, "wlp3s0");
                assert_eq!(cidr.to_string(), "192.168.3.10/24");
                assert_eq!(gateway, Some(Ipv4Addr::new(192, 168, 3, 1)));
                assert!(dns.is_empty(), "les DNS sont appliqués séparément");
            }
            other => panic!("opération inattendue : {other:?}"),
        }
    }

    #[test]
    fn static_request_without_gateway() {
        let request = roundtrip(request_for_static(&iface(), &cidr("10.0.0.5/16"), None));
        match request.op {
            Operation::ApplyStatic { gateway, .. } => assert_eq!(gateway, None),
            other => panic!("opération inattendue : {other:?}"),
        }
    }

    #[test]
    fn dhcp_and_reset_requests() {
        match roundtrip(request_for_dhcp(&iface())).op {
            Operation::SetDhcp { ifname } => assert_eq!(ifname, "wlp3s0"),
            other => panic!("opération inattendue : {other:?}"),
        }

        match roundtrip(request_for_reset_dns(&iface())).op {
            Operation::ResetDns { ifname } => assert_eq!(ifname, "wlp3s0"),
            other => panic!("opération inattendue : {other:?}"),
        }
    }

    #[test]
    fn dns_request_keeps_servers() {
        let servers = [Ipv4Addr::new(1, 1, 1, 1), Ipv4Addr::new(9, 9, 9, 9)];
        match roundtrip(request_for_dns(&iface(), &servers)).op {
            Operation::SetDns { ifname, dns } => {
                assert_eq!(ifname, "wlp3s0");
                assert_eq!(dns, servers);
            }
            other => panic!("opération inattendue : {other:?}"),
        }
    }

    #[test]
    fn empty_dns_is_noop() {
        // Aucun helper, donc aucune commande système, n'est sollicité.
        assert_eq!(set_dns_static(&iface(), &[]), Ok(()));
    }

    #[test]
    fn alias_requests_carry_prefix() {
        match roundtrip(request_for_add_alias(&iface(), &cidr("192.168.3.20/24"))).op {
            Operation::AddAlias { ifname, cidr } => {
                assert_eq!(ifname, "wlp3s0");
                assert_eq!(cidr.to_string(), "192.168.3.20/24");
            }
            other => panic!("opération inattendue : {other:?}"),
        }

        // Le préfixe doit survivre : `ip addr del` en a besoin.
        match roundtrip(request_for_remove_alias(&iface(), &cidr("192.168.3.20/24"))).op {
            Operation::RemoveAlias { ifname, cidr } => {
                assert_eq!(ifname, "wlp3s0");
                assert_eq!(cidr.to_string(), "192.168.3.20/24");
                assert_eq!(cidr.prefix, 24);
            }
            other => panic!("opération inattendue : {other:?}"),
        }
    }

    #[test]
    fn error_response_is_reported() {
        assert_eq!(
            response_to_result(Response::error("nmcli a échoué")),
            Err("nmcli a échoué".to_string())
        );
    }

    #[test]
    fn error_without_message_uses_fallback() {
        let response = Response {
            ok: false,
            message: None,
        };
        assert_eq!(
            response_to_result(response),
            Err(FAILED_WITHOUT_DETAIL.to_string())
        );
    }

    #[test]
    fn successful_response_is_ok() {
        assert_eq!(response_to_result(Response::ok()), Ok(()));
    }

    #[test]
    fn dry_run_launches_no_system_command() {
        // `helper::call` renvoie Ok sans résoudre ni lancer le helper.
        // SAFETY : seul ce test manipule la variable ; les autres tests ne
        // passent pas par `send`.
        unsafe {
            std::env::set_var("FASTIPSWITCH_DRY_RUN", "1");
        }

        let result = set_static(
            &iface(),
            &cidr("192.168.3.10/24"),
            Some(Ipv4Addr::new(192, 168, 3, 1)),
        );

        unsafe {
            std::env::remove_var("FASTIPSWITCH_DRY_RUN");
        }

        assert_eq!(result, Ok(()));
    }
}
