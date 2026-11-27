// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::path::Path;

use super::protocol::{Operation, Request, VERSION};

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

    validate_ifname(request.op.ifname())
}

/// Le nom d'interface doit désigner une interface noyau existante et ne doit
/// jamais permettre de sortir de `/sys/class/net` ni d'injecter un argument.
pub fn validate_ifname(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("nom d'interface vide".into());
    }
    if name.len() > 15 {
        return Err(format!("nom d'interface trop long : {name}"));
    }
    if name == "lo" || name == "." || name == ".." {
        return Err(format!("interface non autorisée : {name}"));
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | ':' | '@'))
    {
        return Err(format!("nom d'interface invalide : {name}"));
    }
    if !Path::new("/sys/class/net").join(name).exists() {
        return Err(format!("interface inconnue : {name}"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
            long.as_str(),
        ] {
            assert!(
                validate_ifname(name).is_err(),
                "{name:?} aurait dû être refusé"
            );
        }
    }
}
