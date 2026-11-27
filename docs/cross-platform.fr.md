[English](cross-platform.md) | **Français**

# FastIpSwitch — état multiplateforme

État : **Windows et Linux sont implémentés**. Le backend et le déroulé Windows sont inchangés (netsh, UAC, tâche planifiée). Linux est un premier portage, durci après une revue de sécurité (NetworkManager requis pour les interfaces gérées, repli `ip`, paquet `.deb`). **macOS n'est pas implémenté.**

- Guide utilisateur Linux : [linux.fr.md](linux.fr.md)
- Résumé Linux dans le README : [README — Linux](../README.fr.md#linux)

## Architecture commune

L'ancien code spécifiquement Windows est désormais découpé derrière une façade commune :

- `src/platform/mod.rs` définit les types partagés (`AdapterInfo`, `IfaceId`, `IpEntry`, `AdapterRef`), le trait `NetworkBackend`, `backend()` et les fonctions système (admin/instance, élévation, chemins, installation, thème, boîtes de message).
- `src/platform/windows/` conserve l'implémentation netsh/Win32.
- `src/platform/linux/` fournit l'implémentation Linux (`read`, `ops`, `helper`, `exec`, `paths`, `installer`, `instance`, `theme`).
- L'interface Slint, la validation CIDR, l'i18n et la session de récupération restent communes.
- Toute autre cible échoue à la compilation avec `compile_error!("FastIpSwitch prend en charge Windows et Linux uniquement")`.

## Correspondance des plateformes

| Domaine | Windows (implémenté) | Linux (implémenté) | macOS (non implémenté) |
|---|---|---|---|
| Lire les interfaces | `GetIfTable2`, `GetUnicastIpAddressTable`, `GetAdaptersAddresses` | `ip -j addr/link/route show`, `nmcli connection show --active`, `nmcli -g IP4.DNS device show` | `getifaddrs`, `networksetup -listallnetworkservices` |
| Changer l'IP | `netsh interface ipv4 set address` (`store=active`) | `nmcli device modify` (gérée, `connected`) ou instantané/repli `ip` ; refusé si NM tourne mais ne gère pas l'interface | `networksetup -setmanual` / `-setdhcp` |
| IP virtuelles | `netsh add address` + `dhcpstaticipcoexistence` | `ip -4 addr add/del` (coexiste avec DHCP ; pas de bascule de coexistence ; noms d'interfaces virtuelles refusés) | `ifconfig <if> alias` |
| Passerelle | `gateway=` de netsh | `ipv4.gateway` via nmcli ; en repli seulement si l'interface a une route par défaut ou s'il n'en existe aucune (métrique conservée, nouvelle route `1000`) | `networksetup -setrouter` |
| DNS | `netsh set dnsservers` (persistant) | `nmcli ipv4.dns` + `ipv4.ignore-auto-dns`, ou `resolvectl dns` ; nettoyage seulement si le DNS a été appliqué | `networksetup -setdnsservers` |
| Élévation | manifeste `asInvoker` + `ShellExecuteW("runas")` (UAC) | GUI non privilégiée + helper `pkexec` + polkit (`allow_any=no`, `allow_inactive=no`, `allow_active=auth_admin_keep`) | Authorization Services / helper privilégié |
| Tray | `Shell_NotifyIcon` via Slint | `system-tray` Slint (StatusNotifierItem) ; GNOME demande AppIndicator | `NSStatusItem` via Slint |
| Installation | copie dans `%LOCALAPPDATA%\Programs`, raccourcis `.lnk`, `schtasks` | `.deb` (système) + installation par utilisateur optionnelle depuis l'UI (`~/.local/bin`, entrées XDG, autostart) | `.app` + `.dmg`, LaunchAgent / `SMAppService` |
| Données | `data/` à côté de l'exe, sinon `%APPDATA%`/`%LOCALAPPDATA%` | XDG (`~/.config`, `~/.local/state`), `data/` portable si accessible en écriture | `~/Library/Application Support` |
| Récupération de session | `session.toml` + nettoyage netsh ciblé | `session.toml` + retrait des alias, retour DHCP/DNS ; session liée à une interface, conservée si elle est absente ou illisible ; instance unique via `flock` | — |

## Linux

Le backend Linux est implémenté, avec une requête JSON par opération privilégiée :

- **La lecture** est non privilégiée (`ip -j`, requêtes `nmcli`). Les échecs de `nmcli` sont tolérés ; ceux de `ip` sont fatals. Les commandes enfants tournent en locale C.
- **Les écritures** sont déléguées à `fastipswitch-helper` via `pkexec`. Le helper revalide toute la requête (existence et type d'interface, CIDR avec minimum `/8`, passerelle, bornes DNS), plafonne stdin à 64 Kio et exécute des vecteurs d'arguments sans shell.
- **L'état de l'interface** est classé en trois cas : NM géré et `connected` (nmcli) ; NM présent mais tout autre état, par ex. `connected (externally)`/`unmanaged`/`disconnected` (statique et DHCP refusés) ; aucun NM (repli). Si `nmcli device status` échoue, le helper considère NM comme absent.
- **Adresse principale / DNS / DHCP** passent par NetworkManager seulement quand l'interface est `connected` ; sinon repli `ip` pour l'adressage et `resolvectl` pour le DNS. Les changements NM utilisent `device modify`, c'est-à-dire la configuration active uniquement, ce qui correspond à l'intention `store=active` de Windows.
- **Sécurité du repli** : l'adressage statique sauvegarde adresses/routes par défaut et les réapplique en cas d'échec ; le retour DHCP exige `networkctl`, est refusé avant modification sans lui, et réapplique l'instantané si `networkctl reconfigure` échoue (pas de faux succès).
- **Les alias** passent toujours par `ip -4 addr add/del` ; ils coexistent nativement avec DHCP.
- **Policy** : `org.talevinn.fastipswitch.network` (`allow_any=no`, `allow_inactive=no`, `allow_active=auth_admin_keep`), avec `exec.path` fixé à `/usr/libexec/fastipswitch/fastipswitch-helper`. L'élévation est limitée à la session locale active ; le keep d'environ cinq minutes s'applique au processus appelant.
- **Packaging** : `packaging/build-deb.sh` produit un `.deb` (GUI, helper, policy, entrée de bureau, icônes, manuel, AppStream), valide la policy et exige `dpkg-deb --root-owner-group`. Pas d'AppImage : le helper et la policy doivent être installés dans des chemins système.

Limites connues : une interface non gérée par NM ne peut pas être modifiée (statique/DHCP) ; sans NM, le retour DHCP exige networkd (`networkctl`) ; alias volatils si le profil NM est réappliqué ; changements DNS via `resolvectl` seulement hors NM ; tray GNOME avec extension AppIndicator ; une instance ; session liée à une interface ; pas de routes statiques ; pas d'IPv6 ; minimum `/8` ; pas de dépôt signé. Détails : [linux.fr.md](linux.fr.md).

## macOS

Non implémenté. Le modèle reste transposable (`ifconfig en0 alias ...`), mais tout passe par des outils qui exigent root (`networksetup`, `ifconfig`). Points ouverts, inchangés :

- **Élévation** : Authorization Services ou un helper privilégié signé ; `osascript` est possible mais moins propre.
- **Distribution** : `.app` + `.dmg` ; signature Developer ID et notarisation Gatekeeper obligatoires pour éviter les avertissements.
- **Autostart** : LaunchAgent ou `SMAppService` (macOS 13+).
- **Tray** : `NSStatusItem` géré par Slint.

## Plan de refactorisation — réalisé et restant

1. **Trait `NetworkBackend` — réalisé.** Il est implémenté par `WindowsBackend` et `LinuxBackend`, avec une surface un peu plus large que l'esquisse initiale :
   ```rust
   fn read(&self) -> Result<Vec<AdapterInfo>, String>;
   fn set_static(&self, id: &IfaceId, cidr: &Cidr, gateway: Option<Ipv4Addr>) -> Result<(), String>;
   fn set_dhcp(&self, id: &IfaceId) -> Result<(), String>;
   fn reset_dns_dhcp(&self, id: &IfaceId) -> Result<(), String>;
   fn set_dns_static(&self, id: &IfaceId, servers: &[Ipv4Addr]) -> Result<(), String>;
   fn add_address(&self, id: &IfaceId, cidr: &Cidr) -> Result<(), String>;
   fn delete_address(&self, id: &IfaceId, cidr: &Cidr) -> Result<(), String>;
   fn set_coexistence(&self, id: &IfaceId, enabled: bool) -> Result<(), String>;
   ```
   `set_coexistence` est spécifique à Windows et sans effet sous Linux.
2. **Isolation de la plateforme — réalisée pour Linux** : chemins de données, élévation, installation, instance unique et thème vivent dans `src/platform/linux/` ; Windows conserve son module.
3. **UI, i18n, validation et session communes — réalisé.**
4. **Tests — en local uniquement** : `cargo fmt --check`, `cargo test --all-targets` et `cargo build --release` sont lancés sur la machine de développement ; Windows et macOS n'y sont pas compilés.
5. **Packaging — partiel** : `.deb` fait ; installateur Windows inchangé ; pas d'AppImage ni de `.dmg`.

| Plateforme | État | Bloquants restants |
|---|---|---|
| Windows | implémenté, backend/déroulé inchangés | — |
| Linux | implémenté (premier portage, durci) | matrice de validation matérielle/bureau ; interfaces non gérées par NM refusées ; retour DHCP nécessitant networkd ; extension tray GNOME ; pas de dépôt signé |
| macOS | non implémenté | helper privilégié, signature/notarisation, packaging, tests locaux |

## Conclusion

Le portage Linux valide la conception : le **backend réseau** et l'**élévation** étaient le vrai travail, tandis que l'UI Slint, la validation et la session sont restées communes. Le même découpage est le point de départ pour macOS ; le travail restant y est surtout la signature/notarisation et le helper privilégié.
