**English** | [Français](cross-platform.fr.md)

# FastIpSwitch — cross-platform status

Status: **Windows and Linux are implemented**. The Windows backend and workflow are unchanged (netsh, UAC, scheduled task). Linux is an initial port, hardened after a security review (NetworkManager required for managed interfaces, `ip` fallback, `.deb` packaging). **macOS is not implemented.**

- Linux user guide: [linux.md](linux.md)
- Linux summary in the README: [README — Linux](../README.md#linux)

## Shared architecture

The former Windows-only code is now split behind a common facade:

- `src/platform/mod.rs` defines the shared data types (`AdapterInfo`, `IfaceId`, `IpEntry`, `AdapterRef`), the `NetworkBackend` trait, `backend()` and the system helpers (admin/instance, elevation, paths, installation, theme, message boxes).
- `src/platform/windows/` keeps the netsh/Win32 implementation.
- `src/platform/linux/` provides the Linux implementation (`read`, `ops`, `helper`, `exec`, `paths`, `installer`, `instance`, `theme`).
- The Slint UI, CIDR validation, i18n and the recovery session remain common.
- Any other target fails to compile with `compile_error!("FastIpSwitch prend en charge Windows et Linux uniquement")`.

## Platform mapping

| Area | Windows (implemented) | Linux (implemented) | macOS (not implemented) |
|---|---|---|---|
| Read interfaces | `GetIfTable2`, `GetUnicastIpAddressTable`, `GetAdaptersAddresses` | `ip -j addr/link/route show`, `nmcli connection show --active`, `nmcli -g IP4.DNS device show` | `getifaddrs`, `networksetup -listallnetworkservices` |
| Change IP | `netsh interface ipv4 set address` (`store=active`) | `nmcli device modify` (managed, `connected`) or `ip` snapshot/fallback; refused when NM runs but does not manage the interface | `networksetup -setmanual` / `-setdhcp` |
| Virtual IPs | `netsh add address` + `dhcpstaticipcoexistence` | `ip -4 addr add/del` (coexists with DHCP; no coexistence toggle needed; virtual interface names rejected) | `ifconfig <if> alias` |
| Gateway | netsh `gateway=` | `ipv4.gateway` via nmcli; fallback only if this interface has a default route or none exists (metric kept, new route `1000`) | `networksetup -setrouter` |
| DNS | `netsh set dnsservers` (persistent) | `nmcli ipv4.dns` + `ipv4.ignore-auto-dns`, or `resolvectl dns`; cleanup only if DNS was applied | `networksetup -setdnsservers` |
| Elevation | `asInvoker` manifest + `ShellExecuteW("runas")` (UAC) | unprivileged GUI + `pkexec` helper + polkit (`allow_any=no`, `allow_inactive=no`, `allow_active=auth_admin_keep`) | Authorization Services / privileged helper |
| Tray | `Shell_NotifyIcon` via Slint | Slint `system-tray` (StatusNotifierItem); GNOME needs AppIndicator | `NSStatusItem` via Slint |
| Installation | copy to `%LOCALAPPDATA%\Programs`, `.lnk` shortcuts, `schtasks` | `.deb` (system) + optional per-user install from the UI (`~/.local/bin`, XDG entries, autostart) | `.app` + `.dmg`, LaunchAgent / `SMAppService` |
| Data | `data/` next to the exe, otherwise `%APPDATA%`/`%LOCALAPPDATA%` | XDG (`~/.config`, `~/.local/state`), portable `data/` if writable | `~/Library/Application Support` |
| Session recovery | `session.toml` + targeted netsh cleanup | `session.toml` + alias removal, DHCP/DNS restore; session bound to one interface, kept if the interface is missing or unreadable; single instance via `flock` | — |

## Linux

The Linux backend is implemented, with one JSON request per privileged operation:

- **Reading** is unprivileged (`ip -j`, `nmcli` queries). `nmcli` failures are tolerated; `ip` failures are fatal. Child commands run with a C locale.
- **Writes** are delegated to `fastipswitch-helper` through `pkexec`. The helper revalidates the whole request (interface existence and type, `/8`-minimum CIDR, gateway, DNS bounds), caps stdin at 64 KiB and runs argument vectors without a shell.
- **Interface state** is classified three ways: NM managed and `connected` (nmcli); NM running but any other state, e.g. `connected (externally)`/`unmanaged`/`disconnected` (static and DHCP refused); no NM (fallback). If `nmcli device status` fails, the helper treats NM as absent.
- **Primary address / DNS / DHCP** use NetworkManager only when the device is `connected`; otherwise the `ip` fallback for addressing and `resolvectl` for DNS. NM changes use `device modify`, i.e. the active configuration only, matching the Windows `store=active` intent.
- **Fallback safety**: static addressing snapshots addresses/default routes and re-applies them on failure; DHCP restore requires `networkctl`, is refused before changes without it, and re-applies the snapshot if `networkctl reconfigure` fails (no false success).
- **Aliases** always use `ip -4 addr add/del`; they coexist with DHCP natively.
- **Policy**: `org.talevinn.fastipswitch.network` (`allow_any=no`, `allow_inactive=no`, `allow_active=auth_admin_keep`), with `exec.path` pinned to `/usr/libexec/fastipswitch/fastipswitch-helper`. Elevation is limited to the local active session; the ~5-minute keep applies to the calling process.
- **Packaging**: `packaging/build-deb.sh` produces a `.deb` (GUI, helper, policy, desktop entry, icons, man page, AppStream), validates the policy and requires `dpkg-deb --root-owner-group`. No AppImage: the helper and policy must be installed in system paths.

Known limits: NM-unmanaged interfaces cannot be changed (static/DHCP); without NM, DHCP restore needs networkd (`networkctl`); volatile aliases if the NM profile is reapplied; DNS changes via `resolvectl` only outside NM; GNOME tray needs AppIndicator; one instance; session bound to one interface; no static routes; no IPv6; `/8` minimum; no signed repository. Details: [linux.md](linux.md).

## macOS

Not implemented. The model still translates (`ifconfig en0 alias ...`), but everything goes through tools that require root (`networksetup`, `ifconfig`). Open points, unchanged:

- **Elevation**: Authorization Services or a signed privileged helper; `osascript` is possible but less clean.
- **Distribution**: `.app` + `.dmg`; Developer ID signing and Gatekeeper notarization are mandatory to avoid warnings.
- **Autostart**: LaunchAgent or `SMAppService` (macOS 13+).
- **Tray**: `NSStatusItem` handled by Slint.

## Refactoring plan — delivered and remaining

1. **`NetworkBackend` trait — done.** It is implemented by `WindowsBackend` and `LinuxBackend`, with a slightly broader surface than the original sketch:
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
   `set_coexistence` is a Windows-specific concern and a no-op on Linux.
2. **Platform isolation — done for Linux**: data paths, elevation, installation, single instance and theme live in `src/platform/linux/`; Windows keeps its module.
3. **Common UI, i18n, validation, session — done.**
4. **CI — partial**: Woodpecker runs `fmt`, tests and a release build on Linux (`rust:1.99-bookworm`, `--locked`). There is no Windows or macOS CI job, and the `.deb` is not built in CI.
5. **Packaging — partial**: `.deb` done; Windows installer unchanged; no AppImage or `.dmg`.

| Platform | Status | Remaining blockers |
|---|---|---|
| Windows | implemented, backend/workflow unchanged | — |
| Linux | implemented (initial port, security-hardened) | hardware/desktop validation matrix; NM-unmanaged interfaces refused; DHCP restore needs networkd; GNOME tray extension; no signed repository |
| macOS | not implemented | privileged helper, signing/notarization, packaging, CI |

## Conclusion

The Linux port validates the design: the **network backend** and **elevation** were the real work, while the Slint UI, validation and session logic stayed common. The same split is the starting point for macOS; the main remaining work there is signing/notarization and the privileged helper.
