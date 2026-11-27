[English](cross-platform.md) | **Français**

# FastIpSwitch — évaluation multiplateforme

État : **V0.1 est Windows uniquement**. Ce document évalue ce qu'il faudrait pour Linux et macOS.

## Ce qui est conceptuellement portable

Dans l'état actuel, le code est couplé à Windows (`win`, `installer`, `store`, `i18n` via le registre et `GetUserDefaultUILanguage`). Ce qui est portable, ou le deviendra après extraction :

- L'interface **Slint** (fenêtre, thème, widgets, tray via `SystemTrayIcon`).
- La logique applicative (profils CIDR, validation, liste d'alias, i18n FR/EN, session de récupération).
- Le modèle « adresse principale + IP virtuelles » existe aussi sous Linux et macOS.

## Ce qui est spécifique à Windows

| Domaine | Implémentation actuelle | Équivalent Linux | Équivalent macOS |
|---|---|---|---|
| Lire les interfaces | `GetIfTable2`, `GetUnicastIpAddressTable`, `GetAdaptersAddresses` | `getifaddrs`, rtnetlink, ou `nmcli` | `getifaddrs`, `networksetup -listallnetworkservices` |
| Changer l'IP | `netsh interface ipv4 set address` | `ip addr` / `nmcli connection modify` | `networksetup -setmanual` / `-setdhcp` |
| IP virtuelles | `netsh add address` + `dhcpstaticipcoexistence` | `ip addr add` (coexiste nativement avec DHCP) | `ifconfig <if> alias` (coexiste avec DHCP) |
| Passerelle | `gateway=` de netsh | `ip route replace default via` / nmcli | `networksetup -setrouter` |
| DNS | `netsh set dnsservers` | `resolvectl dns` / nmcli | `networksetup -setdnsservers` |
| Élévation | manifeste `asInvoker` + `ShellExecuteW("runas")` (UAC) | `pkexec`, `sudo`, helper polkit | Authorization Services / `osascript ... with administrator privileges` |
| Tray | `Shell_NotifyIcon` (via Slint) | StatusNotifierItem (extension GNOME requise) | `NSStatusItem` (via Slint) |
| Installation | copie dans `%LOCALAPPDATA%\Programs` + raccourcis `.lnk` + `schtasks` | `.deb`, AppImage, `~/.local/bin`, `.desktop`, autostart XDG | `.app` + `.dmg`, `~/Applications`, LaunchAgent / `SMAppService` |
| Données | `data/` à côté de l'exe, sinon `%APPDATA%`/`%LOCALAPPDATA%` | XDG (`~/.config`, `~/.local/share`) | `~/Library/Application Support` |

## Linux

Faisable, et même plus simple que Windows pour les alias : `ip addr add 192.168.3.10/24 dev eth0` ne coupe pas DHCP, et il n'y a pas d'équivalent `store=active` à gérer (ou via un profil NetworkManager temporaire). Points d'attention :

- **Élévation** : passer par `pkexec` ou un helper polkit ; `sudo` en terminal n'est pas acceptable pour une app GUI.
- **NetworkManager** : sur les distributions de bureau, les modifications doivent idéalement passer par `nmcli` pour être visibles/cohérentes dans l'UI système ; `ip addr` fonctionne mais n'est pas persisté (ce qui correspond bien au mode temporaire).
- **Tray** : StatusNotifierItem ; GNOME demande l'extension AppIndicator.
- **Packaging** : AppImage (portable, un seul fichier) + `.deb`, autostart via `~/.config/autostart/*.desktop`.
- **Effort estimé** : 2 à 4 jours pour un backend `ip`/`nmcli`, l'élévation et le packaging AppImage, hors tests matériels.

## macOS

Le modèle « adresse principale + alias » fonctionne (`ifconfig en0 alias ...`), mais tout passe par des outils qui demandent les droits root (`networksetup`, `ifconfig`). Points d'attention :

- **Élévation** : `Authorization Services` ou un helper privilégié signé ; `osascript` est possible mais moins propre.
- **Distribution** : `.app` + `.dmg`, signature Developer ID + notarisation Gatekeeper obligatoires pour éviter les avertissements.
- **Autostart** : LaunchAgent ou `SMAppService` (macOS 13+).
- **Tray** : `NSStatusItem` géré par Slint.
- **Effort estimé** : 3 à 5 jours de développement, plus le compte Apple Developer et la notarisation.

## Plan de refactorisation

1. Introduire un trait `NetworkBackend` :
   ```rust
   trait NetworkBackend {
       fn adapters(&self) -> Result<Vec<AdapterInfo>>;
       fn set_static(&self, iface: &Iface, cidr: Cidr, gw: Option<Ipv4Addr>, dns: &[Ipv4Addr]) -> Result<()>;
       fn set_dhcp(&self, iface: &Iface) -> Result<()>;
       fn add_alias(&self, iface: &Iface, cidr: Cidr) -> Result<()>;
       fn remove_alias(&self, iface: &Iface, ip: Ipv4Addr) -> Result<()>;
   }
   ```
   avec `WindowsNetwork`, `LinuxNetwork`, `MacNetwork`.
2. Isoler la plateforme dans `platform/` : chemins de données, élévation, installation, autostart, notifications.
3. Garder l'UI Slint, l'i18n, la validation CIDR et la session communs.
4. CI matrice (Windows, Ubuntu, macOS) + builds séparés.
5. Packaging : MSI/exe Windows, AppImage/deb Linux, dmg macOS.

| Plateforme | Difficulté | Délai indicatif | Bloquants |
|---|---|---|---|
| Windows | fait | — | — |
| Linux | moyenne | 2–4 j | polkit, NetworkManager, tray GNOME |
| macOS | moyenne+ | 3–5 j | signature/notarisation Apple |

## Conclusion

Le couple **Rust + Slint** est un bon choix pour porter l'UI sur Linux et macOS. Le travail réel est le **backend réseau** et l'**élévation**, pas l'interface.
