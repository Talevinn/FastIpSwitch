// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::fmt;
use std::net::Ipv4Addr;
use std::str::FromStr;

/// Adresse IPv4 avec longueur de préfixe, au format `192.168.2.10/24`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Cidr {
    pub addr: Ipv4Addr,
    pub prefix: u8,
}

impl Cidr {
    pub fn new(addr: Ipv4Addr, prefix: u8) -> Result<Self, String> {
        let cidr = Self { addr, prefix };
        cidr.validate()?;
        Ok(cidr)
    }

    /// Accepte `a.b.c.d/24`, `a.b.c.d` (préfixe 24 par défaut), avec espaces tolérés.
    pub fn parse(input: &str) -> Result<Self, String> {
        let raw = input.trim();
        if raw.is_empty() {
            return Err(crate::i18n::tr().cidr_enter().into());
        }
        if raw.contains(char::is_whitespace) {
            return Err(crate::i18n::tr().cidr_format().into());
        }

        let (ip_part, prefix_part) = match raw.split_once('/') {
            Some((ip, prefix)) => (ip, Some(prefix)),
            None => (raw, None),
        };

        let addr =
            Ipv4Addr::from_str(ip_part).map_err(|_| crate::i18n::tr().cidr_invalid_ip(ip_part))?;

        let prefix = match prefix_part {
            Some("") => return Err(crate::i18n::tr().cidr_missing_prefix().into()),
            Some(p) => p
                .parse::<u8>()
                .map_err(|_| crate::i18n::tr().cidr_invalid_prefix(p))?,
            None => 24,
        };

        Self::new(addr, prefix)
    }

    fn validate(&self) -> Result<(), String> {
        if self.prefix > 32 {
            return Err(crate::i18n::tr().cidr_prefix_max(self.prefix));
        }
        if self.addr.is_unspecified() || self.addr.octets()[0] == 0 {
            return Err(crate::i18n::tr().cidr_unspecified().into());
        }
        if self.addr.is_loopback() {
            return Err(crate::i18n::tr().cidr_loopback().into());
        }
        if self.addr.is_multicast() || self.addr.is_broadcast() {
            return Err(crate::i18n::tr().cidr_not_host(self.addr));
        }

        if self.prefix <= 30 {
            if self.addr == self.network() {
                let host_min = Ipv4Addr::from(self.network_u32() + 1);
                let host_max = Ipv4Addr::from(self.broadcast_u32() - 1);
                return Err(crate::i18n::tr().cidr_network(
                    self.addr,
                    self.prefix,
                    host_min,
                    host_max,
                ));
            }
            if self.addr == self.broadcast() {
                return Err(crate::i18n::tr().cidr_broadcast(self.addr, self.prefix));
            }
        }

        Ok(())
    }

    pub fn mask_u32(&self) -> u32 {
        if self.prefix == 0 {
            0
        } else {
            u32::MAX << (32 - self.prefix)
        }
    }

    pub fn mask(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.mask_u32())
    }

    pub fn network_u32(&self) -> u32 {
        u32::from(self.addr) & self.mask_u32()
    }

    pub fn broadcast_u32(&self) -> u32 {
        self.network_u32() | !self.mask_u32()
    }

    pub fn network(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.network_u32())
    }

    pub fn broadcast(&self) -> Ipv4Addr {
        Ipv4Addr::from(self.broadcast_u32())
    }

    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        (u32::from(ip) & self.mask_u32()) == self.network_u32()
    }
}

impl fmt::Display for Cidr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.addr, self.prefix)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cidr_and_defaults_to_24() {
        let a = Cidr::parse("192.168.2.10/24").unwrap();
        assert_eq!(a.addr, Ipv4Addr::new(192, 168, 2, 10));
        assert_eq!(a.prefix, 24);

        let b = Cidr::parse(" 10.0.0.5 ").unwrap();
        assert_eq!(b.prefix, 24);
        assert_eq!(b.to_string(), "10.0.0.5/24");
    }

    #[test]
    fn rejects_network_and_broadcast() {
        assert!(Cidr::parse("192.168.1.0/24").is_err());
        assert!(Cidr::parse("192.168.1.255/24").is_err());
        assert!(Cidr::parse("192.168.1.1/24").is_ok());
    }

    #[test]
    fn allows_31_and_32() {
        assert!(Cidr::parse("10.0.0.0/31").is_ok());
        assert!(Cidr::parse("10.0.0.7/32").is_ok());
    }

    #[test]
    fn rejects_reserved_ranges() {
        assert!(Cidr::parse("0.0.0.0/24").is_err());
        assert!(Cidr::parse("127.0.0.1/8").is_err());
        assert!(Cidr::parse("224.0.0.1/4").is_err());
    }

    #[test]
    fn mask_and_contains() {
        let c = Cidr::parse("192.168.2.10/24").unwrap();
        assert_eq!(c.mask(), Ipv4Addr::new(255, 255, 255, 0));
        assert!(c.contains(Ipv4Addr::new(192, 168, 2, 1)));
        assert!(!c.contains(Ipv4Addr::new(192, 168, 3, 1)));

        let big = Cidr::parse("10.1.2.3/16").unwrap();
        assert!(big.contains(Ipv4Addr::new(10, 1, 255, 254)));
        assert!(!big.contains(Ipv4Addr::new(10, 2, 0, 1)));
    }

    #[test]
    fn rejects_bad_input() {
        assert!(Cidr::parse("").is_err());
        assert!(Cidr::parse("192.168.1").is_err());
        assert!(Cidr::parse("192.168.1.1/33").is_err());
        assert!(Cidr::parse("192.168.1.1/").is_err());
        assert!(Cidr::parse("192.168.1.1 /24").is_err());
    }
}
