// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::net::Ipv4Addr;
use std::os::windows::process::CommandExt;
use std::process::{Command, Stdio};

use crate::cidr::Cidr;

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

fn run(args: &[String]) -> Result<(), String> {
    let mut command = Command::new("netsh");
    command
        .arg("interface")
        .arg("ipv4")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .creation_flags(CREATE_NO_WINDOW);

    let printable = format!("netsh interface ipv4 {}", args.join(" "));
    crate::log::write(&printable);

    let output = command
        .output()
        .map_err(|e| crate::i18n::tr().netsh_launch_failed(&e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let details = format!("{}{}", stdout.trim(), stderr.trim());
    let details = details.trim();

    if output.status.success() {
        crate::log::write("  -> ok");
        Ok(())
    } else {
        let code = output
            .status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "?".into());
        crate::log::write(&format!("  -> échec ({code}) {details}"));
        if details.is_empty() {
            Err(crate::i18n::tr().netsh_failed_admin(&code))
        } else {
            Err(crate::i18n::tr().netsh_failed(&code, details))
        }
    }
}

pub fn set_static(if_index: u32, cidr: &Cidr, gateway: Option<Ipv4Addr>) -> Result<(), String> {
    let gateway = gateway
        .map(|g| g.to_string())
        .unwrap_or_else(|| "none".to_string());
    run(&[
        "set".into(),
        "address".into(),
        format!("name={if_index}"),
        "source=static".into(),
        format!("address={}", cidr.addr),
        format!("mask={}", cidr.mask()),
        format!("gateway={gateway}"),
        "store=active".into(),
    ])
}

pub fn set_dhcp(if_index: u32) -> Result<(), String> {
    run(&[
        "set".into(),
        "address".into(),
        format!("name={if_index}"),
        "source=dhcp".into(),
        "store=active".into(),
    ])
}

pub fn reset_dns_dhcp(if_index: u32) -> Result<(), String> {
    run(&[
        "set".into(),
        "dnsservers".into(),
        format!("name={if_index}"),
        "source=dhcp".into(),
    ])
}

pub fn set_dns_static(if_index: u32, servers: &[Ipv4Addr]) -> Result<(), String> {
    let Some(first) = servers.first() else {
        return Ok(());
    };

    run(&[
        "set".into(),
        "dnsservers".into(),
        format!("name={if_index}"),
        "source=static".into(),
        format!("address={first}"),
        "register=primary".into(),
        "validate=no".into(),
    ])?;

    for (position, server) in servers.iter().enumerate().skip(1) {
        run(&[
            "add".into(),
            "dnsservers".into(),
            format!("name={if_index}"),
            format!("address={server}"),
            format!("index={}", position + 1),
        ])?;
    }

    Ok(())
}

pub fn add_address(if_index: u32, cidr: &Cidr) -> Result<(), String> {
    run(&[
        "add".into(),
        "address".into(),
        format!("name={if_index}"),
        format!("address={}", cidr.addr),
        format!("mask={}", cidr.mask()),
        "store=active".into(),
        "skipassource=true".into(),
    ])
}

pub fn delete_address(if_index: u32, addr: Ipv4Addr) -> Result<(), String> {
    run(&[
        "delete".into(),
        "address".into(),
        format!("name={if_index}"),
        format!("address={addr}"),
        "store=active".into(),
    ])
}

pub fn set_coexistence(if_index: u32, enabled: bool) -> Result<(), String> {
    run(&[
        "set".into(),
        "interface".into(),
        format!("interface={if_index}"),
        format!(
            "dhcpstaticipcoexistence={}",
            if enabled { "enabled" } else { "disabled" }
        ),
        "store=active".into(),
    ])
}
