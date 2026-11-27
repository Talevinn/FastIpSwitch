// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::net::Ipv4Addr;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::cidr::Cidr;
use crate::net::{self, AdapterInfo};
use crate::netsh;
use crate::store::{self, Session};

#[derive(Clone, Debug)]
pub struct AdapterRef {
    pub index: u32,
    pub mac: String,
    pub dhcp: bool,
}

impl From<&AdapterInfo> for AdapterRef {
    fn from(adapter: &AdapterInfo) -> Self {
        Self {
            index: adapter.if_index,
            mac: adapter.mac.clone(),
            dhcp: adapter.dhcp,
        }
    }
}

#[derive(Debug)]
pub enum Job {
    AddAlias {
        id: i32,
        adapter: AdapterRef,
        cidr: Cidr,
    },
    SetAlias {
        id: i32,
        adapter: AdapterRef,
        cidr: Cidr,
        enabled: bool,
    },
    RemoveAlias {
        id: i32,
        adapter: AdapterRef,
        cidr: Cidr,
    },
    ApplyStatic {
        adapter: AdapterRef,
        cidr: Cidr,
        gateway: Option<Ipv4Addr>,
        dns: Vec<Ipv4Addr>,
        aliases: Vec<Cidr>,
    },
    SetDhcp {
        adapter: AdapterRef,
    },
    Quit,
}

#[derive(Debug)]
pub enum Event {
    Busy(bool),
    Adapters(Vec<AdapterInfo>),
    Message { text: String, error: bool },
    AliasState { id: i32, enabled: bool },
    AliasGone { id: i32 },
    DisableAll,
    QuitReady,
    QuitFailed,
    Dirty(bool),
}

pub fn spawn(events: Sender<Event>) -> Sender<Job> {
    let (tx, rx) = std::sync::mpsc::channel::<Job>();
    thread::Builder::new()
        .name("netsh-worker".into())
        .spawn(move || {
            let session = Arc::new(Mutex::new(store::load_session().unwrap_or_default()));
            while let Ok(job) = rx.recv() {
                let _ = events.send(Event::Busy(true));
                handle(job, &session, &events);
                let dirty = !session.lock().unwrap().is_empty();
                let _ = events.send(Event::Dirty(dirty));
                let _ = events.send(Event::Busy(false));
            }
        })
        .expect("impossible de démarrer le thread netsh");
    tx
}

fn handle(job: Job, session: &Arc<Mutex<Session>>, tx: &Sender<Event>) {
    let result = match job {
        Job::AddAlias { id, adapter, cidr } => add_alias(id, &adapter, &cidr, session, tx, true),
        Job::SetAlias {
            id,
            adapter,
            cidr,
            enabled,
        } => {
            if enabled {
                add_alias(id, &adapter, &cidr, session, tx, false)
            } else {
                remove_alias_inner(id, &adapter, &cidr, session, tx, false)
            }
        }
        Job::RemoveAlias { id, adapter, cidr } => {
            remove_alias_inner(id, &adapter, &cidr, session, tx, true)
        }
        Job::ApplyStatic {
            adapter,
            cidr,
            gateway,
            dns,
            aliases,
        } => apply_static(&adapter, &cidr, gateway, &dns, &aliases, session, tx),
        Job::SetDhcp { adapter } => {
            let result = restore_defaults(adapter.index, session, tx);
            if result.is_ok() {
                let _ = tx.send(Event::Message {
                    text: crate::i18n::tr().dhcp_done().into(),
                    error: false,
                });
            }
            result
        }
        Job::Quit => {
            let snapshot = session.lock().unwrap().clone();
            if snapshot.is_empty() {
                let _ = tx.send(Event::QuitReady);
                Ok(())
            } else {
                match restore_defaults(snapshot.if_index, session, tx) {
                    Ok(()) => {
                        let _ = tx.send(Event::QuitReady);
                        Ok(())
                    }
                    Err(e) => {
                        let _ = tx.send(Event::QuitFailed);
                        Err(e)
                    }
                }
            }
        }
    };

    if let Err(message) = result {
        crate::log::write(&format!("ERREUR: {message}"));
        let _ = tx.send(Event::Message {
            text: message,
            error: true,
        });
    }
}

fn refresh(tx: &Sender<Event>) {
    match net::read_adapters() {
        Ok(adapters) => {
            let _ = tx.send(Event::Adapters(adapters));
        }
        Err(e) => {
            let _ = tx.send(Event::Message {
                text: crate::i18n::tr().read_config_failed(&e.to_string()),
                error: true,
            });
        }
    }
}

fn add_alias(
    id: i32,
    adapter: &AdapterRef,
    cidr: &Cidr,
    session: &Arc<Mutex<Session>>,
    tx: &Sender<Event>,
    is_new: bool,
) -> Result<(), String> {
    let previous = session.lock().unwrap().clone();

    {
        let mut s = session.lock().unwrap();
        s.adapter_mac = adapter.mac.clone();
        s.if_index = adapter.index;
        s.pending = Some(cidr.to_string());
        s.coexistence = true;
        store::save_session(&s);
    }

    let result = (|| -> Result<(), String> {
        netsh::set_coexistence(adapter.index, true)?;
        netsh::add_address(adapter.index, cidr)?;

        let adapters = net::read_adapters().unwrap_or_default();
        let current = adapters.iter().find(|a| a.if_index == adapter.index);

        if let Some(current) = current {
            if !current.has_address(cidr.addr) {
                return Err(crate::i18n::tr().add_not_visible().into());
            }
            if adapter.dhcp && (!current.dhcp || !current.has_dhcp_address()) {
                return Err(crate::i18n::tr().dhcp_disabled().into());
            }
        }

        let mut s = session.lock().unwrap();
        s.pending = None;
        s.note_alias(cidr);
        store::save_session(&s);

        let _ = tx.send(Event::Adapters(adapters));
        let _ = tx.send(Event::AliasState { id, enabled: true });
        let _ = tx.send(Event::Message {
            text: crate::i18n::tr().alias_enabled(*cidr),
            error: false,
        });
        Ok(())
    })();

    if result.is_err() {
        let _ = netsh::delete_address(adapter.index, cidr.addr);

        // DHCP a-t-il été coupé par Windows pendant l'ajout ?
        if adapter.dhcp {
            let dhcp_lost = net::read_adapters()
                .unwrap_or_default()
                .iter()
                .find(|a| a.if_index == adapter.index)
                .is_some_and(|a| !a.dhcp || !a.has_dhcp_address());
            if dhcp_lost {
                let _ = netsh::set_dhcp(adapter.index);
            }
        }
        if !previous.coexistence {
            let _ = netsh::set_coexistence(adapter.index, false);
        }

        {
            let mut s = session.lock().unwrap();
            *s = previous;
            if s.is_empty() {
                store::clear_session();
            } else {
                store::save_session(&s);
            }
        }

        if is_new {
            let _ = tx.send(Event::AliasGone { id });
        } else {
            // Réactivation d'une IP existante : on la remet simplement OFF.
            let _ = tx.send(Event::AliasState { id, enabled: false });
        }
        refresh(tx);
    }

    result
}

fn remove_alias_inner(
    id: i32,
    adapter: &AdapterRef,
    cidr: &Cidr,
    session: &Arc<Mutex<Session>>,
    tx: &Sender<Event>,
    remove_row: bool,
) -> Result<(), String> {
    if let Err(e) = netsh::delete_address(adapter.index, cidr.addr) {
        // La ligne reste dans la liste, mais on la débloque.
        let _ = tx.send(Event::AliasState { id, enabled: true });
        return Err(e);
    }

    {
        let mut s = session.lock().unwrap();
        s.forget_alias(cidr);

        if s.applied.is_empty() && s.pending.is_none() {
            s.coexistence = false;
            let _ = netsh::set_coexistence(adapter.index, false);
        }

        if s.is_empty() {
            *s = Session::default();
            store::clear_session();
        } else {
            store::save_session(&s);
        }
    }

    if remove_row {
        let _ = tx.send(Event::AliasGone { id });
    } else {
        let _ = tx.send(Event::AliasState { id, enabled: false });
    }
    let _ = tx.send(Event::Message {
        text: crate::i18n::tr().alias_disabled(*cidr),
        error: false,
    });
    refresh(tx);
    Ok(())
}

fn apply_static(
    adapter: &AdapterRef,
    cidr: &Cidr,
    gateway: Option<Ipv4Addr>,
    dns: &[Ipv4Addr],
    aliases: &[Cidr],
    session: &Arc<Mutex<Session>>,
    tx: &Sender<Event>,
) -> Result<(), String> {
    let previous = session.lock().unwrap().clone();

    {
        let mut s = session.lock().unwrap();
        s.adapter_mac = adapter.mac.clone();
        s.if_index = adapter.index;
        s.static_set = true;
        s.pending = Some(cidr.to_string());
        if !dns.is_empty() {
            s.dns_set = true;
        }
        store::save_session(&s);
    }

    if let Err(e) = netsh::set_static(adapter.index, cidr, gateway) {
        let mut s = session.lock().unwrap();
        *s = previous;
        if s.is_empty() {
            store::clear_session();
        } else {
            store::save_session(&s);
        }
        return Err(e);
    }

    let mut warnings = Vec::new();
    if !dns.is_empty()
        && let Err(e) = netsh::set_dns_static(adapter.index, dns)
    {
        warnings.push(format!("DNS : {e}"));
    }
    for alias in aliases {
        if let Err(e) = netsh::add_address(adapter.index, alias) {
            warnings.push(format!("{alias} : {e}"));
        }
    }

    {
        let mut s = session.lock().unwrap();
        s.pending = None;
        s.applied = aliases.iter().map(|a| a.to_string()).collect();
        store::save_session(&s);
    }

    let adapters = net::read_adapters().unwrap_or_default();
    let applied = adapters
        .iter()
        .find(|a| a.if_index == adapter.index)
        .is_some_and(|a| a.has_address(cidr.addr));

    let _ = tx.send(Event::Adapters(adapters));

    if !applied {
        return Err(crate::i18n::tr().static_not_visible().into());
    }
    if warnings.is_empty() {
        let _ = tx.send(Event::Message {
            text: crate::i18n::tr().static_applied(*cidr),
            error: false,
        });
    } else {
        let _ = tx.send(Event::Message {
            text: crate::i18n::tr().static_alias_failures(&warnings.join(" ; ")),
            error: true,
        });
    }
    Ok(())
}

fn restore_defaults(
    if_index: u32,
    session: &Arc<Mutex<Session>>,
    tx: &Sender<Event>,
) -> Result<(), String> {
    let snapshot = session.lock().unwrap().clone();

    let mut addresses = snapshot.applied.clone();
    if let Some(pending) = &snapshot.pending {
        addresses.push(pending.clone());
    }
    for item in addresses {
        if let Ok(cidr) = Cidr::parse(&item) {
            let _ = netsh::delete_address(if_index, cidr.addr);
        }
    }

    let already_dhcp = net::read_adapters()
        .ok()
        .and_then(|list| list.into_iter().find(|a| a.if_index == if_index))
        .map(|a| a.dhcp)
        .unwrap_or(false);

    let dhcp = if already_dhcp {
        Ok(())
    } else {
        netsh::set_dhcp(if_index)
    };
    if let Err(e) = dhcp {
        refresh(tx);
        return Err(e);
    }

    if snapshot.dns_set
        && let Err(e) = netsh::reset_dns_dhcp(if_index)
    {
        refresh(tx);
        return Err(e);
    }
    if snapshot.coexistence {
        let _ = netsh::set_coexistence(if_index, false);
    }

    {
        let mut s = session.lock().unwrap();
        *s = Session::default();
    }
    store::clear_session();

    let _ = tx.send(Event::DisableAll);
    refresh(tx);
    Ok(())
}

/// Au démarrage : nettoie une session laissée par un arrêt brutal.
pub fn recover_pending(adapters: &[AdapterInfo]) -> Option<String> {
    let session = store::load_session()?;
    if session.is_empty() {
        store::clear_session();
        return None;
    }

    let Some(target) = adapters.iter().find(|a| {
        if !session.adapter_mac.is_empty() {
            a.mac.eq_ignore_ascii_case(&session.adapter_mac)
        } else if session.if_index != 0 {
            a.if_index == session.if_index
        } else {
            false
        }
    }) else {
        store::clear_session();
        return Some(crate::i18n::tr().recover_missing_adapter().to_string());
    };

    let if_index = target.if_index;
    let already_dhcp = target.dhcp;

    crate::log::write(&format!(
        "Récupération de la session précédente sur l'interface {if_index}"
    ));

    let mut addresses = session.applied.clone();
    if let Some(pending) = &session.pending {
        addresses.push(pending.clone());
    }
    for item in addresses {
        if let Ok(cidr) = Cidr::parse(&item) {
            let _ = netsh::delete_address(if_index, cidr.addr);
        }
    }

    if !already_dhcp && let Err(e) = netsh::set_dhcp(if_index) {
        return Some(crate::i18n::tr().recover_partial(&e));
    }
    if session.dns_set
        && let Err(e) = netsh::reset_dns_dhcp(if_index)
    {
        return Some(crate::i18n::tr().recover_partial(&e));
    }
    if session.coexistence {
        let _ = netsh::set_coexistence(if_index, false);
    }
    store::clear_session();

    Some(crate::i18n::tr().recover_done().to_string())
}
