// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::net::Ipv4Addr;
use std::sync::OnceLock;

use crate::cidr::Cidr;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    Fr,
    En,
}

static LANG: OnceLock<Lang> = OnceLock::new();

pub fn lang() -> Lang {
    *LANG.get_or_init(detect)
}

fn detect() -> Lang {
    if let Ok(value) = std::env::var("FASTIPSWITCH_LANG") {
        if value.eq_ignore_ascii_case("fr") || value.eq_ignore_ascii_case("français") {
            return Lang::Fr;
        }
        if value.eq_ignore_ascii_case("en") || value.eq_ignore_ascii_case("english") {
            return Lang::En;
        }
    }

    #[cfg(windows)]
    {
        let id = unsafe { windows::Win32::Globalization::GetUserDefaultUILanguage() };
        if (id & 0x3ff) == 0x0c {
            return Lang::Fr;
        }
    }

    Lang::En
}

pub struct T;

pub fn tr() -> T {
    T
}

impl T {
    fn pick(&self, fr: &'static str, en: &'static str) -> &'static str {
        match lang() {
            Lang::Fr => fr,
            Lang::En => en,
        }
    }

    // --- Libellés statiques ---
    pub fn address_label(&self) -> &'static str {
        self.pick("ADRESSE PRINCIPALE", "PRIMARY ADDRESS")
    }
    pub fn virtual_label(&self) -> &'static str {
        self.pick("IP VIRTUELLES", "VIRTUAL IPs")
    }
    pub fn apply(&self) -> &'static str {
        self.pick("APPLIQUER", "APPLY")
    }
    pub fn gateway_placeholder(&self) -> &'static str {
        self.pick("192.168.1.1", "192.168.1.1")
    }
    pub fn gateway_label(&self) -> &'static str {
        self.pick("PASSERELLE", "GATEWAY")
    }
    pub fn dns_label(&self) -> &'static str {
        "DNS"
    }
    pub fn dns_placeholder(&self) -> &'static str {
        "1.1.1.1, 8.8.8.8"
    }
    pub fn add_tip(&self) -> &'static str {
        self.pick("Ajouter une IP virtuelle", "Add a virtual IP")
    }
    pub fn remove_tip(&self) -> &'static str {
        self.pick("Supprimer cette IP virtuelle", "Remove this virtual IP")
    }
    pub fn toggle_tip(&self) -> &'static str {
        self.pick("Activer / désactiver cette IP", "Enable / disable this IP")
    }
    pub fn dhcp_tip(&self) -> &'static str {
        self.pick("Activer / désactiver DHCP", "Enable / disable DHCP")
    }
    pub fn install_tip(&self) -> &'static str {
        self.pick(
            "Installer / gérer FastIpSwitch",
            "Install / manage FastIpSwitch",
        )
    }
    pub fn installed_tag(&self) -> &'static str {
        self.pick("installé", "installed")
    }
    pub fn autostart_tip(&self) -> &'static str {
        self.pick(
            "Démarrer FastIpSwitch à l'ouverture de session, en administrateur",
            "Start FastIpSwitch at sign-in, elevated",
        )
    }
    pub fn add(&self) -> &'static str {
        self.pick("AJOUTER", "ADD")
    }
    pub fn quit(&self) -> &'static str {
        self.pick("QUITTER", "QUIT")
    }
    pub fn no_aliases(&self) -> &'static str {
        self.pick("AUCUNE IP VIRTUELLE", "NO VIRTUAL IP")
    }
    pub fn no_adapter(&self) -> &'static str {
        self.pick("Aucun adaptateur", "No adapter")
    }
    pub fn lock_title(&self) -> &'static str {
        self.pick(
            "DROITS ADMINISTRATEUR REQUIS",
            "ADMINISTRATOR RIGHTS REQUIRED",
        )
    }
    pub fn lock_body(&self) -> &'static str {
        self.pick(
            "FastIpSwitch doit être lancé en privilège élevé pour lire et modifier la configuration réseau de Windows.",
            "FastIpSwitch must be launched with elevated privileges to read and modify the Windows network configuration.",
        )
    }
    pub fn elevate(&self) -> &'static str {
        self.pick("RELANCER EN ADMINISTRATEUR", "RESTART AS ADMINISTRATOR")
    }
    pub fn close(&self) -> &'static str {
        self.pick("FERMER", "CLOSE")
    }
    pub fn ready(&self) -> &'static str {
        self.pick("Prêt", "Ready")
    }
    pub fn menu_show(&self) -> &'static str {
        self.pick("Afficher FastIpSwitch", "Show FastIpSwitch")
    }
    pub fn menu_dhcp(&self) -> &'static str {
        self.pick("Revenir en DHCP", "Back to DHCP")
    }
    pub fn menu_quit(&self) -> &'static str {
        self.pick("Quitter", "Quit")
    }
    pub fn mode_dhcp(&self) -> &'static str {
        "DHCP"
    }
    pub fn mode_static(&self) -> &'static str {
        self.pick("STATIQUE", "STATIC")
    }

    // --- Installateur ---
    pub fn install_title(&self) -> &'static str {
        self.pick("INSTALLER FASTIPSWITCH", "INSTALL FASTIPSWITCH")
    }
    pub fn install_dest(&self) -> &'static str {
        self.pick("Destination :", "Destination:")
    }
    pub fn install_autostart(&self) -> &'static str {
        self.pick("LANCER AU DÉMARRAGE", "START WITH WINDOWS")
    }
    pub fn install_autostart_hint(&self) -> &'static str {
        self.pick(
            "Tâche planifiée élevée, sans UAC au logon.",
            "Elevated scheduled task, no UAC at logon.",
        )
    }
    pub fn install_button(&self) -> &'static str {
        self.pick("INSTALLER", "INSTALL")
    }
    pub fn reinstall_button(&self) -> &'static str {
        self.pick("RÉINSTALLER", "REINSTALL")
    }
    pub fn cancel(&self) -> &'static str {
        self.pick("ANNULER", "CANCEL")
    }
    pub fn installed_ok(&self, path: &str) -> String {
        match lang() {
            Lang::Fr => format!("Installé dans {path}. Raccourcis créés."),
            Lang::En => format!("Installed to {path}. Shortcuts created."),
        }
    }
    pub fn install_failed(&self, error: &str) -> String {
        match lang() {
            Lang::Fr => format!("Installation impossible : {error}"),
            Lang::En => format!("Installation failed: {error}"),
        }
    }

    // --- Messages dynamiques ---
    pub fn tooltip(&self, status: &str, mode: &str, active: usize) -> String {
        match lang() {
            Lang::Fr => format!("FastIpSwitch — {status} · {mode} · {active} alias actif(s)"),
            Lang::En => format!("FastIpSwitch — {status} · {mode} · {active} active alias(es)"),
        }
    }

    pub fn already_running(&self) -> &'static str {
        self.pick(
            "FastIpSwitch est déjà lancé.\nRegarde la zone de notification Windows.",
            "FastIpSwitch is already running.\nCheck the Windows notification area.",
        )
    }
    pub fn already_running_admin(&self) -> &'static str {
        self.pick(
            "FastIpSwitch est déjà lancé en administrateur.",
            "FastIpSwitch is already running as administrator.",
        )
    }
    pub fn admin_instance_running(&self) -> &'static str {
        self.pick(
            "Une instance administrateur est déjà lancée.",
            "An administrator instance is already running.",
        )
    }
    pub fn elevate_failed(&self, error: &str) -> String {
        match lang() {
            Lang::Fr => {
                format!("Impossible de relancer FastIpSwitch en administrateur.\n\n{error}")
            }
            Lang::En => format!("Unable to restart FastIpSwitch as administrator.\n\n{error}"),
        }
    }
    pub fn elevation_cancelled(&self, error: &str) -> String {
        match lang() {
            Lang::Fr => format!("Élévation annulée ou impossible ({error})."),
            Lang::En => format!("Elevation cancelled or unavailable ({error})."),
        }
    }
    pub fn adapter_selected(&self, name: &str) -> String {
        match lang() {
            Lang::Fr => format!("Adaptateur : {name}"),
            Lang::En => format!("Adapter: {name}"),
        }
    }
    pub fn adapter_not_found(&self, name: &str) -> String {
        match lang() {
            Lang::Fr => format!("Adaptateur introuvable : {name}"),
            Lang::En => format!("Adapter not found: {name}"),
        }
    }
    pub fn no_usable_address(&self, name: &str) -> String {
        match lang() {
            Lang::Fr => {
                format!("Aucune adresse exploitable sur {name} (bail DHCP en attente ?)")
            }
            Lang::En => format!("No usable address on {name} (DHCP lease pending?)"),
        }
    }
    pub fn disable_aliases_first(&self) -> &'static str {
        self.pick(
            "Désactive les IP virtuelles actives avant de changer d'adaptateur.",
            "Disable active virtual IPs before switching adapter.",
        )
    }
    pub fn finish_changes_first(&self) -> &'static str {
        self.pick(
            "Termine les modifications en cours (reviens en DHCP) avant de changer d'adaptateur.",
            "Finish current changes (return to DHCP) before switching adapter.",
        )
    }
    pub fn no_adapter_selected(&self) -> &'static str {
        self.pick("Aucun adaptateur sélectionné.", "No adapter selected.")
    }
    pub fn gateway_is_address(&self) -> &'static str {
        self.pick(
            "La passerelle ne peut pas être l'adresse elle-même.",
            "The gateway cannot be the address itself.",
        )
    }
    pub fn gateway_not_in_subnet(&self, gateway: Ipv4Addr, cidr: Cidr) -> String {
        match lang() {
            Lang::Fr => format!("La passerelle {gateway} n'est pas dans le sous-réseau {cidr}."),
            Lang::En => format!("Gateway {gateway} is not in subnet {cidr}."),
        }
    }
    pub fn gateway_not_host(&self, gateway: Ipv4Addr) -> String {
        match lang() {
            Lang::Fr => format!(
                "La passerelle {gateway} est l'adresse réseau ou de diffusion de ce sous-réseau."
            ),
            Lang::En => {
                format!("Gateway {gateway} is the network or broadcast address of this subnet.")
            }
        }
    }
    pub fn invalid_gateway(&self, raw: &str) -> String {
        match lang() {
            Lang::Fr => format!("« {raw} » n'est pas une adresse IPv4 valide."),
            Lang::En => format!("\"{raw}\" is not a valid IPv4 address."),
        }
    }
    pub fn invalid_dns(&self, raw: &str) -> String {
        match lang() {
            Lang::Fr => format!("« {raw} » n'est pas un serveur DNS IPv4 valide."),
            Lang::En => format!("\"{raw}\" is not a valid IPv4 DNS server."),
        }
    }
    pub fn applying(&self, cidr: Cidr) -> String {
        match lang() {
            Lang::Fr => format!("Application de {cidr}…"),
            Lang::En => format!("Applying {cidr}…"),
        }
    }
    pub fn returning_dhcp(&self) -> &'static str {
        self.pick("Retour en DHCP…", "Returning to DHCP…")
    }
    pub fn alias_exists(&self, ip: Ipv4Addr) -> String {
        match lang() {
            Lang::Fr => {
                format!("{ip} est déjà configurée sur cette interface.")
            }
            Lang::En => format!("{ip} is already configured on this interface."),
        }
    }
    pub fn enabling(&self, cidr: Cidr) -> String {
        match lang() {
            Lang::Fr => format!("Activation de {cidr}…"),
            Lang::En => format!("Enabling {cidr}…"),
        }
    }
    pub fn disabling(&self, cidr: Cidr) -> String {
        match lang() {
            Lang::Fr => format!("Désactivation de {cidr}…"),
            Lang::En => format!("Disabling {cidr}…"),
        }
    }
    pub fn removing(&self, cidr: Cidr) -> String {
        match lang() {
            Lang::Fr => format!("Suppression de {cidr}…"),
            Lang::En => format!("Removing {cidr}…"),
        }
    }
    pub fn alias_removed(&self) -> &'static str {
        self.pick(
            "IP virtuelle retirée de la liste.",
            "Virtual IP removed from the list.",
        )
    }
    pub fn restoring_quit(&self) -> &'static str {
        self.pick(
            "Restauration DHCP puis fermeture…",
            "Restoring DHCP, then closing…",
        )
    }
    pub fn refreshed(&self) -> &'static str {
        self.pick("Configuration actualisée.", "Configuration refreshed.")
    }
    pub fn read_failed(&self, error: &str) -> String {
        match lang() {
            Lang::Fr => format!("Lecture impossible : {error}"),
            Lang::En => format!("Read failed: {error}"),
        }
    }

    // --- Worker ---
    pub fn dhcp_done(&self) -> &'static str {
        self.pick("Retour en DHCP effectué.", "Back to DHCP.")
    }
    pub fn add_not_visible(&self) -> &'static str {
        self.pick(
            "L'adresse n'apparaît pas dans la configuration après l'ajout. Windows a refusé l'opération.",
            "The address does not appear in the configuration after adding. Windows refused the operation.",
        )
    }
    pub fn dhcp_disabled(&self) -> &'static str {
        self.pick(
            "Windows a désactivé DHCP lors de l'ajout : opération annulée pour préserver ta connexion.",
            "Windows disabled DHCP during the add: operation cancelled to protect your connection.",
        )
    }
    pub fn alias_enabled(&self, cidr: Cidr) -> String {
        match lang() {
            Lang::Fr => format!("{cidr} activée."),
            Lang::En => format!("{cidr} enabled."),
        }
    }
    pub fn alias_disabled(&self, cidr: Cidr) -> String {
        match lang() {
            Lang::Fr => format!("{cidr} désactivée."),
            Lang::En => format!("{cidr} disabled."),
        }
    }
    pub fn static_applied(&self, cidr: Cidr) -> String {
        match lang() {
            Lang::Fr => format!("Adresse principale appliquée : {cidr}."),
            Lang::En => format!("Primary address applied: {cidr}."),
        }
    }
    pub fn static_alias_failures(&self, failures: &str) -> String {
        match lang() {
            Lang::Fr => {
                format!("Adresse appliquée, mais certains réglages ont échoué : {failures}")
            }
            Lang::En => format!("Address applied, but some settings failed: {failures}"),
        }
    }
    pub fn static_not_visible(&self) -> &'static str {
        self.pick(
            "La nouvelle adresse n'apparaît pas dans la configuration.",
            "The new address does not appear in the configuration.",
        )
    }
    pub fn read_config_failed(&self, error: &str) -> String {
        match lang() {
            Lang::Fr => format!("Lecture de la configuration impossible : {error}"),
            Lang::En => format!("Unable to read configuration: {error}"),
        }
    }
    pub fn recover_missing_adapter(&self) -> &'static str {
        self.pick(
            "Trace d'une session précédente ignorée : l'adaptateur n'existe plus.",
            "Previous session trace ignored: the adapter no longer exists.",
        )
    }
    pub fn recover_done(&self) -> &'static str {
        self.pick(
            "Configuration temporaire précédente nettoyée : retour en DHCP.",
            "Previous temporary configuration cleaned up: back to DHCP.",
        )
    }
    pub fn recover_partial(&self, error: &str) -> String {
        match lang() {
            Lang::Fr => format!("Nettoyage partiel : le retour DHCP a échoué ({error})."),
            Lang::En => format!("Partial cleanup: the DHCP restore failed ({error})."),
        }
    }

    // --- netsh ---
    pub fn netsh_failed_admin(&self, code: &str) -> String {
        match lang() {
            Lang::Fr => format!(
                "netsh a échoué (code {code}). Vérifie que l'application est lancée en administrateur."
            ),
            Lang::En => format!(
                "netsh failed (code {code}). Check that the application is running as administrator."
            ),
        }
    }
    pub fn netsh_failed(&self, code: &str, details: &str) -> String {
        match lang() {
            Lang::Fr => format!("netsh a échoué (code {code}) : {details}"),
            Lang::En => format!("netsh failed (code {code}): {details}"),
        }
    }
    pub fn netsh_launch_failed(&self, error: &str) -> String {
        match lang() {
            Lang::Fr => format!("Impossible de lancer netsh : {error}"),
            Lang::En => format!("Unable to start netsh: {error}"),
        }
    }

    // --- CIDR ---
    pub fn cidr_enter(&self) -> &'static str {
        self.pick(
            "Saisis une adresse, par exemple 192.168.2.10/24",
            "Enter an address, for example 192.168.2.10/24",
        )
    }
    pub fn cidr_format(&self) -> &'static str {
        self.pick(
            "Format attendu : 192.168.2.10/24 (sans espace)",
            "Expected format: 192.168.2.10/24 (no space)",
        )
    }
    pub fn cidr_invalid_ip(&self, ip: &str) -> String {
        match lang() {
            Lang::Fr => format!("« {ip} » n'est pas une adresse IPv4 valide"),
            Lang::En => format!("\"{ip}\" is not a valid IPv4 address"),
        }
    }
    pub fn cidr_missing_prefix(&self) -> &'static str {
        self.pick("Préfixe manquant après le /", "Missing prefix after /")
    }
    pub fn cidr_invalid_prefix(&self, p: &str) -> String {
        match lang() {
            Lang::Fr => format!("Préfixe « {p} » invalide (0 à 32 attendu)"),
            Lang::En => format!("Invalid prefix \"{p}\" (0 to 32 expected)"),
        }
    }
    pub fn cidr_prefix_max(&self, prefix: u8) -> String {
        match lang() {
            Lang::Fr => format!("Préfixe /{prefix} invalide : maximum /32"),
            Lang::En => format!("Invalid prefix /{prefix}: maximum /32"),
        }
    }
    pub fn cidr_unspecified(&self) -> &'static str {
        self.pick(
            "0.0.0.0 et le réseau 0.0.0.0/8 ne sont pas utilisables",
            "0.0.0.0 and the 0.0.0.0/8 network are not usable",
        )
    }
    pub fn cidr_loopback(&self) -> &'static str {
        self.pick(
            "Les adresses 127.x.x.x sont réservées au loopback",
            "127.x.x.x addresses are reserved for loopback",
        )
    }
    pub fn cidr_not_host(&self, addr: Ipv4Addr) -> String {
        match lang() {
            Lang::Fr => format!("{addr} n'est pas une adresse hôte utilisable"),
            Lang::En => format!("{addr} is not a usable host address"),
        }
    }
    pub fn cidr_network(&self, addr: Ipv4Addr, prefix: u8, min: Ipv4Addr, max: Ipv4Addr) -> String {
        match lang() {
            Lang::Fr => format!(
                "{addr} est l'adresse réseau de ce /{prefix} — utilise une adresse hôte ({min} à {max})"
            ),
            Lang::En => format!(
                "{addr} is the network address of this /{prefix} — use a host address ({min} to {max})"
            ),
        }
    }
    pub fn cidr_broadcast(&self, addr: Ipv4Addr, prefix: u8) -> String {
        match lang() {
            Lang::Fr => format!(
                "{addr} est l'adresse de diffusion de ce /{prefix} — utilise une adresse hôte"
            ),
            Lang::En => {
                format!("{addr} is the broadcast address of this /{prefix} — use a host address")
            }
        }
    }
}
