// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::net::Ipv4Addr;
use std::ptr::null_mut;

use anyhow::{Result, bail};
use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, WIN32_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST, GET_ADAPTERS_ADDRESSES_FLAGS,
    GetAdaptersAddresses, GetIfTable2, GetIpForwardTable2, GetUnicastIpAddressTable,
    IP_ADAPTER_ADDRESSES_LH, IP_ADAPTER_DHCP_ENABLED, MIB_IF_TABLE2, MIB_IPFORWARD_TABLE2,
    MIB_UNICASTIPADDRESS_TABLE,
};
use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR, SOCKADDR_IN, SOCKADDR_INET};
use winreg::RegKey;
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WOW64_64KEY};

use crate::cidr::Cidr;

#[derive(Clone, Debug)]
pub struct IpEntry {
    pub cidr: Cidr,
    /// Adresse issue de DHCP (par opposition à une adresse manuelle).
    pub from_dhcp: bool,
}

#[derive(Clone, Debug)]
pub struct AdapterInfo {
    pub name: String,
    pub description: String,
    pub mac: String,
    pub guid: String,
    pub if_index: u32,
    pub is_up: bool,
    pub dhcp: bool,
    pub addresses: Vec<IpEntry>,
    pub gateway: Option<Ipv4Addr>,
    pub dns: Vec<Ipv4Addr>,
}

impl AdapterInfo {
    /// Adresse « principale » : celle du sous-réseau de la passerelle, sinon
    /// la première adresse DHCP, sinon la première tout court. Les adresses
    /// APIPA (169.254.x.x) sont ignorées : elles signifient « pas de bail ».
    pub fn primary(&self) -> Option<&IpEntry> {
        let usable = || {
            self.addresses
                .iter()
                .filter(|entry| !entry.cidr.addr.is_link_local())
        };

        usable()
            .find(|entry| self.gateway.is_some_and(|gw| entry.cidr.contains(gw)))
            .or_else(|| usable().find(|entry| entry.from_dhcp))
            .or_else(|| usable().next())
    }

    pub fn has_dhcp_address(&self) -> bool {
        self.addresses.iter().any(|e| e.from_dhcp)
    }

    pub fn has_address(&self, ip: Ipv4Addr) -> bool {
        self.addresses.iter().any(|e| e.cidr.addr == ip)
    }
}

const BLOCKLIST: &[&str] = &[
    "loopback",
    "bluetooth",
    "tap-windows",
    "openvpn",
    "wan miniport",
    "kernel debug",
    "pseudo-interface",
    "teredo",
    "isatap",
    "wi-fi direct",
    "npcap",
    "packet scheduler",
    "lightweight filter",
    "native wifi filter",
    "virtual wifi filter",
    "zenprom",
    "filter driver",
    "remote ndis",
    "hyper-v",
    "virtual ethernet",
];

pub fn read_adapters() -> Result<Vec<AdapterInfo>> {
    let mut adapters = unsafe { read_if_table()? };

    let unicast = unsafe { read_unicast_table() };
    for adapter in &mut adapters {
        adapter.addresses = unicast
            .iter()
            .filter(|(idx, _)| *idx == adapter.if_index)
            .map(|(_, entry)| entry.clone())
            .collect();
    }

    unsafe { read_gateways(&mut adapters) };

    let meta = unsafe { read_adapter_meta() };
    for adapter in &mut adapters {
        if let Some((_, dhcp, dns)) = meta.iter().find(|(idx, _, _)| *idx == adapter.if_index) {
            adapter.dhcp = *dhcp;
            adapter.dns = dns.clone();
        } else {
            adapter.dhcp = dhcp_enabled(&adapter.guid);
        }
        adapter.addresses.sort_by_key(|e| u32::from(e.cidr.addr));
    }

    adapters.retain(|a| !a.name.trim().is_empty());
    adapters.retain(|a| !is_blocked(&a.name, &a.description));

    // Adaptateurs connectés d'abord, puis ordre alphabétique.
    adapters.sort_by(|a, b| b.is_up.cmp(&a.is_up).then_with(|| a.name.cmp(&b.name)));
    Ok(adapters)
}

fn is_blocked(name: &str, description: &str) -> bool {
    let haystack = format!("{} {}", name, description).to_lowercase();
    BLOCKLIST.iter().any(|word| haystack.contains(word))
}

unsafe fn read_if_table() -> Result<Vec<AdapterInfo>> {
    let mut table: *mut MIB_IF_TABLE2 = null_mut();
    check(unsafe { GetIfTable2(&mut table) }, "GetIfTable2")?;
    if table.is_null() {
        bail!("GetIfTable2 a renvoyé une table nulle");
    }

    let rows = unsafe {
        std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize)
    };
    let mut adapters = Vec::with_capacity(rows.len());

    for row in rows {
        let if_type = row.Type;
        // 24 = logiciel loopback, 131 = tunnel
        if if_type == 24 || if_type == 131 {
            continue;
        }

        let mac_len = (row.PhysicalAddressLength as usize).min(row.PhysicalAddress.len());
        let mac = if mac_len > 0 {
            row.PhysicalAddress[..mac_len]
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join("-")
        } else {
            String::new()
        };

        adapters.push(AdapterInfo {
            name: wide_to_string(&row.Alias),
            description: wide_to_string(&row.Description),
            mac,
            guid: guid_to_string(&row.InterfaceGuid),
            if_index: row.InterfaceIndex,
            is_up: row.OperStatus.0 == 1, // IfOperStatusUp
            dhcp: false,
            addresses: Vec::new(),
            gateway: None,
            dns: Vec::new(),
        });
    }

    unsafe { FreeMibTable(table as *const _) };
    Ok(adapters)
}

unsafe fn read_unicast_table() -> Vec<(u32, IpEntry)> {
    let mut table: *mut MIB_UNICASTIPADDRESS_TABLE = null_mut();
    let ret = unsafe { GetUnicastIpAddressTable(AF_INET, &mut table) };
    if ret.0 != 0 || table.is_null() {
        crate::log::write(&format!(
            "GetUnicastIpAddressTable a échoué (code {})",
            ret.0
        ));
        return Vec::new();
    }

    let rows = unsafe {
        std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize)
    };
    let mut out = Vec::with_capacity(rows.len());

    for row in rows {
        if let Some(ip) = unsafe { v4_from_sockaddr(&row.Address) } {
            if ip.is_loopback() {
                continue;
            }
            out.push((
                row.InterfaceIndex,
                IpEntry {
                    cidr: Cidr {
                        addr: ip,
                        prefix: row.OnLinkPrefixLength.min(32),
                    },
                    from_dhcp: row.PrefixOrigin.0 == 3, // NlPrefixOrigin_Dhcp
                },
            ));
        }
    }

    unsafe { FreeMibTable(table as *const _) };
    out
}

unsafe fn read_gateways(adapters: &mut [AdapterInfo]) {
    let mut table: *mut MIB_IPFORWARD_TABLE2 = null_mut();
    let ret = unsafe { GetIpForwardTable2(AF_INET, &mut table) };
    if ret.0 != 0 || table.is_null() {
        crate::log::write(&format!("GetIpForwardTable2 a échoué (code {})", ret.0));
        return;
    }

    let rows = unsafe {
        std::slice::from_raw_parts((*table).Table.as_ptr(), (*table).NumEntries as usize)
    };

    let mut best: Vec<(u32, Ipv4Addr, u32)> = Vec::new();
    for row in rows {
        // Route par défaut : 0.0.0.0/0
        if row.DestinationPrefix.PrefixLength != 0 {
            continue;
        }
        let Some(dest) = (unsafe { v4_from_sockaddr(&row.DestinationPrefix.Prefix) }) else {
            continue;
        };
        if !dest.is_unspecified() {
            continue;
        }
        let Some(next_hop) = (unsafe { v4_from_sockaddr(&row.NextHop) }) else {
            continue;
        };
        if next_hop.is_unspecified() {
            continue;
        }
        best.push((row.InterfaceIndex, next_hop, row.Metric));
    }

    unsafe { FreeMibTable(table as *const _) };

    for adapter in adapters.iter_mut() {
        adapter.gateway = best
            .iter()
            .filter(|(idx, _, _)| *idx == adapter.if_index)
            .min_by_key(|(_, _, metric)| *metric)
            .map(|(_, gw, _)| *gw);
    }
}

unsafe fn read_adapter_meta() -> Vec<(u32, bool, Vec<Ipv4Addr>)> {
    let flags = GET_ADAPTERS_ADDRESSES_FLAGS(GAA_FLAG_SKIP_ANYCAST.0 | GAA_FLAG_SKIP_MULTICAST.0);
    let mut size: u32 = 15_000;
    let mut buf = vec![0u8; size as usize];

    loop {
        let ret = unsafe {
            GetAdaptersAddresses(
                AF_INET.0 as u32,
                flags,
                None,
                Some(buf.as_mut_ptr() as *mut IP_ADAPTER_ADDRESSES_LH),
                &mut size,
            )
        };
        if ret == ERROR_BUFFER_OVERFLOW.0 {
            buf.resize(size as usize, 0);
            continue;
        }
        if ret != 0 {
            crate::log::write(&format!("GetAdaptersAddresses a échoué (code {ret})"));
            return Vec::new();
        }
        break;
    }

    let mut out = Vec::new();
    let mut ptr = buf.as_ptr() as *const IP_ADAPTER_ADDRESSES_LH;
    while !ptr.is_null() {
        let adapter = unsafe { &*ptr };
        let index = unsafe { adapter.Anonymous1.Anonymous.IfIndex };
        let adapter_flags = unsafe { adapter.Anonymous2.Flags };

        let mut dns = Vec::new();
        let mut dns_ptr = adapter.FirstDnsServerAddress;
        while !dns_ptr.is_null() {
            let entry = unsafe { &*dns_ptr };
            if let Some(ip) = unsafe { sockaddr_to_v4(entry.Address.lpSockaddr) }
                && !ip.is_unspecified()
            {
                dns.push(ip);
            }
            dns_ptr = entry.Next;
        }

        out.push((index, adapter_flags & IP_ADAPTER_DHCP_ENABLED != 0, dns));
        ptr = adapter.Next;
    }
    out
}

unsafe fn sockaddr_to_v4(sockaddr: *const SOCKADDR) -> Option<Ipv4Addr> {
    if sockaddr.is_null() {
        return None;
    }
    let sa = unsafe { &*(sockaddr as *const SOCKADDR_IN) };
    if sa.sin_family != AF_INET {
        return None;
    }
    let raw = unsafe { sa.sin_addr.S_un.S_addr };
    Some(Ipv4Addr::from(raw.to_ne_bytes()))
}

fn dhcp_enabled(guid: &str) -> bool {
    let path =
        format!("SYSTEM\\CurrentControlSet\\Services\\Tcpip\\Parameters\\Interfaces\\{guid}");
    RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey_with_flags(&path, KEY_READ | KEY_WOW64_64KEY)
        .and_then(|key| key.get_value::<u32, _>("EnableDHCP"))
        .map(|value| value != 0)
        .unwrap_or(false)
}

unsafe fn v4_from_sockaddr(sa: &SOCKADDR_INET) -> Option<Ipv4Addr> {
    if unsafe { sa.si_family } != AF_INET {
        return None;
    }
    let raw = unsafe { sa.Ipv4.sin_addr.S_un.S_addr };
    Some(Ipv4Addr::from(raw.to_ne_bytes()))
}

fn wide_to_string(buf: &[u16]) -> String {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len]).trim().to_string()
}

fn guid_to_string(guid: &windows::core::GUID) -> String {
    let d4 = guid.data4;
    format!(
        "{{{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}}}",
        guid.data1, guid.data2, guid.data3, d4[0], d4[1], d4[2], d4[3], d4[4], d4[5], d4[6], d4[7]
    )
}

fn check(ret: WIN32_ERROR, what: &str) -> Result<()> {
    if ret.0 != 0 {
        bail!("{what} a échoué (code {})", ret.0);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adapter_with(addresses: Vec<IpEntry>, gateway: Option<Ipv4Addr>) -> AdapterInfo {
        AdapterInfo {
            name: "test".into(),
            description: "test".into(),
            mac: "AA-BB-CC-DD-EE-FF".into(),
            guid: String::new(),
            if_index: 1,
            is_up: true,
            dhcp: true,
            addresses,
            gateway,
            dns: Vec::new(),
        }
    }

    #[test]
    fn primary_ignores_apipa() {
        let entry = |addr: [u8; 4], prefix: u8, from_dhcp: bool| IpEntry {
            cidr: Cidr {
                addr: Ipv4Addr::from(addr),
                prefix,
            },
            from_dhcp,
        };

        let apipa = entry([169, 254, 10, 20], 16, false);
        let lease = entry([192, 168, 1, 50], 24, true);

        let adapter = adapter_with(
            vec![apipa.clone(), lease],
            Some(Ipv4Addr::new(192, 168, 1, 1)),
        );
        assert_eq!(
            adapter.primary().map(|entry| entry.cidr.addr),
            Some(Ipv4Addr::new(192, 168, 1, 50))
        );

        let apipa_only = adapter_with(vec![apipa], None);
        assert!(apipa_only.primary().is_none());
    }

    #[test]
    fn reads_adapters_on_this_machine() {
        let adapters = read_adapters().expect("lecture de la configuration");
        for a in &adapters {
            println!(
                "{} | desc={} | idx={} | up={} | dhcp={} | gw={:?} | dns={:?} | mac={} | {:?}",
                a.name,
                a.description,
                a.if_index,
                a.is_up,
                a.dhcp,
                a.gateway,
                a.dns,
                a.mac,
                a.addresses
                    .iter()
                    .map(|e| format!("{}{}", e.cidr, if e.from_dhcp { " (dhcp)" } else { "" }))
                    .collect::<Vec<_>>()
            );
        }
        assert!(!adapters.is_empty(), "aucun adaptateur détecté");
        assert!(
            adapters.iter().any(|a| !a.mac.is_empty()),
            "aucune adresse MAC lue"
        );
    }
}
