**English** | [Français](cross-platform.fr.md)

# FastIpSwitch — cross-platform assessment

Status: **V0.1 is Windows-only**. This document assesses what would be needed for Linux and macOS.

## What is conceptually portable

The current code is coupled to Windows (`win`, `installer`, `store`, `i18n` via the registry and `GetUserDefaultUILanguage`). What is portable, or will be after extraction:

- The **Slint** UI (window, theme, widgets, tray via `SystemTrayIcon`).
- Application logic (CIDR profiles, validation, alias list, English/French i18n, recovery session).
- The "primary address + virtual IPs" model exists on Linux and macOS too.

## Windows-specific parts

| Area | Current implementation | Linux equivalent | macOS equivalent |
|---|---|---|---|
| Read interfaces | `GetIfTable2`, `GetUnicastIpAddressTable`, `GetAdaptersAddresses` | `getifaddrs`, rtnetlink, or `nmcli` | `getifaddrs`, `networksetup -listallnetworkservices` |
| Change IP | `netsh interface ipv4 set address` | `ip addr` / `nmcli connection modify` | `networksetup -setmanual` / `-setdhcp` |
| Virtual IPs | `netsh add address` + `dhcpstaticipcoexistence` | `ip addr add` (coexists with DHCP natively) | `ifconfig <if> alias` (coexists with DHCP) |
| Gateway | netsh `gateway=` | `ip route replace default via` / nmcli | `networksetup -setrouter` |
| DNS | `netsh set dnsservers` | `resolvectl dns` / nmcli | `networksetup -setdnsservers` |
| Elevation | `asInvoker` manifest + `ShellExecuteW("runas")` (UAC) | `pkexec`, `sudo`, polkit helper | Authorization Services / `osascript ... with administrator privileges` |
| Tray | `Shell_NotifyIcon` (via Slint) | StatusNotifierItem (GNOME extension required) | `NSStatusItem` (via Slint) |
| Installation | copy to `%LOCALAPPDATA%\Programs` + `.lnk` shortcuts + `schtasks` | `.deb`, AppImage, `~/.local/bin`, `.desktop`, XDG autostart | `.app` + `.dmg`, `~/Applications`, LaunchAgent / `SMAppService` |
| Data | `data/` next to the exe, otherwise `%APPDATA%`/`%LOCALAPPDATA%` | XDG (`~/.config`, `~/.local/share`) | `~/Library/Application Support` |

## Linux

Feasible, and even simpler than Windows for aliases: `ip addr add 192.168.3.10/24 dev eth0` does not drop DHCP, and there is no `store=active` equivalent to manage (or use a temporary NetworkManager profile). Points to watch:

- **Elevation**: use `pkexec` or a polkit helper; a terminal `sudo` is not acceptable for a GUI app.
- **NetworkManager**: on desktop distributions, changes should ideally go through `nmcli` so the system UI stays consistent; `ip addr` works but is not persisted (which matches the temporary mode).
- **Tray**: StatusNotifierItem; GNOME requires the AppIndicator extension.
- **Packaging**: AppImage (portable, single file) plus `.deb`; autostart via `~/.config/autostart/*.desktop`.
- **Estimated effort**: 2 to 4 days for an `ip`/`nmcli` backend, elevation and AppImage packaging, excluding hardware testing.

## macOS

The "primary address + alias" model works (`ifconfig en0 alias ...`), but everything goes through tools that require root (`networksetup`, `ifconfig`). Points to watch:

- **Elevation**: `Authorization Services` or a signed privileged helper; `osascript` is possible but less clean.
- **Distribution**: `.app` + `.dmg`; Developer ID signing and Gatekeeper notarization are mandatory to avoid warnings.
- **Autostart**: LaunchAgent or `SMAppService` (macOS 13+).
- **Tray**: `NSStatusItem` handled by Slint.
- **Estimated effort**: 3 to 5 days of development, plus an Apple Developer account and notarization.

## Refactoring plan

1. Introduce a `NetworkBackend` trait:
   ```rust
   trait NetworkBackend {
       fn adapters(&self) -> Result<Vec<AdapterInfo>>;
       fn set_static(&self, iface: &Iface, cidr: Cidr, gw: Option<Ipv4Addr>, dns: &[Ipv4Addr]) -> Result<()>;
       fn set_dhcp(&self, iface: &Iface) -> Result<()>;
       fn add_alias(&self, iface: &Iface, cidr: Cidr) -> Result<()>;
       fn remove_alias(&self, iface: &Iface, ip: Ipv4Addr) -> Result<()>;
   }
   ```
   with `WindowsNetwork`, `LinuxNetwork`, `MacNetwork` implementations.
2. Isolate platform concerns in `platform/`: data paths, elevation, installation, autostart, notifications.
3. Keep the Slint UI, i18n, CIDR validation and session logic common.
4. CI matrix (Windows, Ubuntu, macOS) + separate builds.
5. Packaging: MSI/exe Windows, AppImage/deb Linux, dmg macOS.

| Platform | Difficulty | Indicative effort | Blockers |
|---|---|---|---|
| Windows | done | — | — |
| Linux | medium | 2–4 days | polkit, NetworkManager, GNOME tray |
| macOS | medium+ | 3–5 days | Apple signing/notarization |

## Conclusion

**Rust + Slint** is a good fit for porting the UI to Linux and macOS. The real work is the **network backend** and **elevation**, not the interface.
