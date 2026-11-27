**English** | [Français](README.fr.md)

# FastIpSwitch — V0.1

**Maintainer:** Valentin PICARD ([@Talevinn](https://codeberg.org/Talevinn))

## Download

**[Download FastIpSwitch V0.1 for Windows](https://codeberg.org/Talevinn/FastIpSwitch/releases/download/V0.1/FastIpSwitch-V0.1.exe)** — Windows 10/11, x64, run as administrator. [Release notes and checksum](https://codeberg.org/Talevinn/FastIpSwitch/releases/tag/V0.1).

Linux is built from source: `packaging/build-deb.sh` produces a `.deb` for Debian/Ubuntu/Mint (see [Linux](#linux)). No prebuilt Linux binary or AppImage is published.

FastIpSwitch (Rust + [Slint](https://slint.dev)) is a **Windows and Linux** utility to **switch the IPv4 address quickly** and **add or remove virtual IPs on the fly**, so you can commission or interrogate equipment that sits on a different subnet: brand-new devices still on their default IP, legacy gear with a hard-coded address, test benches, AV hardware, etc.

On Windows, all network changes go through **`netsh interface ipv4`**; the Windows layout and behaviour are unchanged. On Linux, they go through a small privileged helper (`nmcli` when NetworkManager manages the interface, `ip` otherwise). The UI follows the system light/dark theme and language (English/French).

| Dark theme | Light theme | Without elevation (Windows) |
|---|---|---|
| ![dark theme](docs/screenshot-dark.png) | ![light theme](docs/screenshot-light.png) | ![locked UI](docs/screenshot-locked.png) |

Screenshots show the Windows build; Linux uses the same Slint layout. There is no Linux screenshot in this repository.

## Why FastIpSwitch

- A device is on `192.168.0.10/24` while your PC is on `192.168.1.45/24`: add a virtual IP `192.168.0.50/24` in one click, without losing your connection.
- You need to put your PC on a lab network with a fixed address: enter `192.168.2.10/24`, the gateway and DNS, then **Apply**.
- You want to return to automatic configuration immediately: the **DHCP** switch or **Quit** restores the defaults.
- You move from one job to the next: the virtual IP list is remembered (disabled at startup), and temporary addresses do not stay behind.

## Features

- Network adapter selector (filter drivers such as Npcap/QoS/WFP are hidden).
- **CIDR** input: `192.168.2.10/24` (no `255.255.255.0` to type).
- Primary address: DHCP ↔ static, optional gateway and **DNS** (several servers separated by commas).
- Virtual IP list with a per-row enable switch, `+` to add, `×` to remove.
- Bidirectional **DHCP** switch: ON = DHCP, OFF = static mode using the displayed values.
- Notification-area icon: click to reopen, menu **Show / Back to DHCP / Quit**.
- **English/French** UI based on Windows (override available), light/dark theme with a rack-inspired variant.
- Tooltips on icons and switches.
- netsh command log and crash recovery.

## Why administrator rights are required on Windows

Windows does not expose a user-level API to change an interface's IP configuration: `netsh interface ipv4` (like `New-NetIPAddress`) requires an elevated token. Without elevation, the commands fail and nothing changes.

FastIpSwitch is therefore designed to run **elevated**, but it does not force UAC at startup:

- the manifest is `asInvoker`; without elevation, the app still opens, **fully greyed out**, with an “Administrator rights required” panel and a **Restart as administrator** button;
- shortcuts created by the installer pass `--elevate` to request UAC at launch;
- automatic startup uses a **scheduled task** `ONLOGON` with `/RL HIGHEST`, so it runs elevated at sign-in without a UAC prompt.

The app applies addressing (IP, mask, gateway, aliases) in **`store=active`**: the persistent configuration is not modified, and a reboot restores the configuration stored in Windows (usually DHCP). **DNS is the exception**: `netsh set dnsservers` has no `store=active`, so it is written persistently and restored on a clean Quit.

## How it works on Windows

- **Virtual IPs**: `netsh add address ... skipassource=true store=active`. `skipassource` keeps Windows from choosing those addresses as the source for outbound traffic, while still answering ARP/ping/HTTP.
- **DHCP coexistence**: Windows disables DHCP when a manual IP is added. FastIpSwitch enables `dhcpstaticipcoexistence` before the add to keep the DHCP connection, then checks that DHCP is still active; otherwise the operation is cancelled and DHCP restored.
- **Static primary address**: `set address source=static ... store=active`, then active aliases are re-added (this command clears manual addresses).
- **DNS**: `set dnsservers source=static` plus `add dnsservers` for additional servers; back to `source=dhcp` on a DHCP restore. These commands have no `store=active`, so static DNS is **persistent**. It is normally restored on exit, but after a crash followed by a reboot without relaunching the app it may remain static: relaunch FastIpSwitch or reset it manually.
- **Adapter refresh and APIPA**: selecting an adapter re-reads the live state. `169.254.x.x` (APIPA/link-local) is ignored: it means there is no DHCP lease. The IP field is left empty with a hint, and a 2-second background check fills it in when a lease arrives; if the current adapter goes down, the app switches to a newly usable adapter (unless you explicitly selected a disconnected adapter to configure it).
- **Validation**: `.0` and `.255` are rejected for a /24 (network/broadcast), the gateway must be in the same subnet, duplicates are rejected, DNS entries must be valid IPv4.

## Restart, quit and failure recovery on Windows

| Event | What happens |
|---|---|
| **Window close button** | In elevated mode, minimizes to the notification area; configuration stays active while the app runs. Without admin, it quits. |
| **Quit** (button or tray) | Removes aliases, switches back to DHCP, resets DNS, then exits. If restore fails, the app stays open and shows the error. |
| **Windows restart** | Returns to the persistent configuration (usually DHCP), even after a crash. Static DNS may remain if the app was not relaunched. |
| **Crash / kill** | `session.toml` is kept; on the next **elevated** launch, targeted cleanup + DHCP restore. |
| **Launch without admin** | Greyed UI + **Restart as administrator**; no network command, no session recovery. |
| **netsh failure** | Error message, rollback of the add, log in `data/fastipswitch.log` (or `%LOCALAPPDATA%`). |

### If something goes wrong on Windows

1. **The UI is greyed out**: click **Restart as administrator** (or right-click the exe → *Run as administrator*).
2. **A virtual IP will not go away**: reopen the app and switch it off; otherwise reboot, the persistent configuration returns.
3. **No network after a manipulation**: reboot, or run in an elevated terminal:
   ```cmd
   netsh interface ipv4 set address name="Ethernet 6" source=dhcp store=active
   netsh interface ipv4 set dnsservers name="Ethernet 6" source=dhcp
   netsh interface ipv4 set interface interface="Ethernet 6" dhcpstaticipcoexistence=disabled store=active
   ```
   (replace `Ethernet 6` with your interface name; `netsh interface ipv4 show interfaces` lists them. `set dnsservers` is persistent by nature; the other two use `store=active` so addressing is not frozen.)
4. **Understand what happened**: the log is in `data/fastipswitch.log` next to the exe, or `%LOCALAPPDATA%\FastIpSwitch\fastipswitch.log`. Every netsh command and its result is written there (with rotation).
5. **Reinstall**: launch the elevated release, click **installed** at the top right → **REINSTALL**. Installed settings and the autostart choice are preserved.

## Installation on Windows

### Portable mode (default)

The exe is self-contained (static CRT, no VC++ dependency). At startup, if it can create a `data` folder next to the executable, it stores `state.toml`, `session.toml` and the log there. Otherwise it uses `%APPDATA%\FastIpSwitch` and `%LOCALAPPDATA%\FastIpSwitch`.

### Built-in installer

Launch the app elevated, then click the install icon at the top right (or **installed** if already installed):

- copies to `%LOCALAPPDATA%\Programs\FastIpSwitch`;
- creates **Desktop** and **Start Menu** shortcuts with `--elevate`;
- optional **Start with Windows** (unchecked by default) → `FastIpSwitch-<user>` scheduled task, `ONLOGON`, `/RL HIGHEST`;
- preserves the existing virtual IP list and the autostart setting on reinstall.

Development shortcut:

```powershell
pwsh -File tools\install-shortcut.ps1
```

## Usage on Windows

1. Select the adapter.
2. **Primary address**: enter `192.168.2.10/24`, the gateway and DNS if needed, then **Apply**.
3. **Virtual IPs**: `+`, enter `192.168.3.10/24`, enable the switch. The IP is added without dropping DHCP.
4. The **DHCP** switch: ON = return to DHCP, OFF = freeze the displayed address as static.
5. **Quit** to restore everything.

`192.168.1.0/24` is rejected: `.0` is the network address; use a host address (`.1` to `.254`).

## Language and theme

- Language: English unless the Windows UI language is French; override with `FASTIPSWITCH_LANG=fr` or `en`. On Linux, the first non-empty value among `LC_ALL`, `LC_MESSAGES`, `LANG` selects French when it starts with `fr`.
- Theme: follows Windows (dark rack / light aluminium). Override with `FASTIPSWITCH_THEME=dark` or `light`. On Linux, detected from GNOME, Cinnamon, XFCE or KDE settings, falling back to light.

## Build on Windows

Requirements: recent stable Rust (≥ 1.88), Visual Studio Build Tools (MSVC), Windows 10/11.

```powershell
cargo build --release
```

Binary: `target\release\fastipswitch.exe`. `.cargo/config.toml` enables `+crt-static` for a self-contained exe. The manifest is `asInvoker` (DPI PerMonitorV2).

## Files on Windows

| Path | Purpose |
|---|---|
| `data\state.toml` / `%APPDATA%\FastIpSwitch\state.toml` | adapter, virtual IP list, last IP/gateway/DNS |
| `data\session.toml` / `%LOCALAPPDATA%\FastIpSwitch\session.toml` | crash-recovery trace |
| `data\fastipswitch.log` / `%LOCALAPPDATA%\FastIpSwitch\fastipswitch.log` | netsh command log (rotated) |
| `%LOCALAPPDATA%\Programs\FastIpSwitch` | per-user installation |

## Known limitations on Windows

- No static route management (devices must be on the same L2 segment).
- DNS is reset to automatic on a DHCP restore; the previous static value is not remembered.
- One instance at a time.
- Per-user installation only (no machine-wide install).

## Linux

The Linux port shares the same Slint UI and the same "primary address + virtual IPs" model. It is an initial port: covered by unit tests and a CI build, but without a published hardware/desktop test matrix. **Windows behaviour is unchanged.** A detailed guide (architecture, command matrix, development, CI) is in [`docs/linux.md`](docs/linux.md).

NetworkManager is recommended: Debian, Ubuntu and Linux Mint desktop installs use it by default. Without NetworkManager, reading and aliases still work through `ip`; the primary address and gateway then fall back to volatile `ip` commands, and DNS requires `resolvectl`.

### Install the `.deb` (Debian, Ubuntu, Linux Mint)

No prebuilt Linux package is published: build the `.deb` from the sources, then install it with `apt` so dependencies are resolved.

```sh
packaging/build-deb.sh
sudo apt install ./packaging/.build/dist/fastipswitch_0.1.0_amd64.deb
```

The package installs the GUI in `/usr/bin/fastipswitch`, the helper in `/usr/libexec/fastipswitch/fastipswitch-helper` (root:root, `0755`), the polkit action `/usr/share/polkit-1/actions/org.talevinn.fastipswitch.policy`, the desktop entry, hicolor icons, the man page and AppStream metadata.

Runtime dependencies:

- `iproute2` — required for reading (`ip -j`) and aliases (`ip addr`);
- `pkexec | policykit-1` — required for elevation (on Debian 12/Ubuntu 24.04, `pkexec` is its own package);
- `network-manager` — **Recommends**, installed by default by `apt`, needed for the `nmcli` path;
- the GUI libraries listed in `packaging/deb/DEBIAN/control` (fontconfig, freetype, xkbcommon, Wayland/X11, EGL/GL).

The `.deb` is **not signed** and no APT repository is provided. There is **no AppImage**: the helper and the polkit policy have to be installed system-wide, which a single portable file does not do.

### Build from source

Requirements: recent stable Rust (edition 2024, same ≥ 1.88 toolchain as Windows), `pkg-config`, and the Slint/winit development headers:

```sh
sudo apt install build-essential pkg-config \
    libfontconfig1-dev libfreetype-dev libxkbcommon-dev \
    libwayland-dev libegl1-mesa-dev libgl1-mesa-dev
cargo build --release
```

`libfreetype-dev` is available on Debian 12+ and Ubuntu 24.04; use `libfreetype6-dev` on older releases. Add `libx11-xcb-dev` to build and run under X11. Binaries: `target/release/fastipswitch` and `target/release/fastipswitch-helper`.

The helper must end up in a system path for normal use. The in-app install button only handles a per-user copy (`~/.local/bin`, icons, desktop entry, optional XDG autostart) and never installs the helper: use the `.deb` for a working elevation setup.

### Usage on Linux

1. Select the interface. Loopback and container/bridge interfaces (`docker*`, `veth*`, `virbr*`, `vmnet*`, `br-*`) are hidden; VPN interfaces (`tun*`, `wg*`, …) are kept.
2. **Primary address**: enter a CIDR, optional gateway and DNS, then **Apply**. With NetworkManager, the app runs `nmcli device modify <if> ipv4.method manual ipv4.addresses <cidr> ipv4.gateway <gw>`: this changes the **active** configuration only, the profile on disk is not rewritten.
3. **Virtual IPs**: `+`, enter a CIDR, enable the row. Aliases always use `ip -4 addr add <cidr> dev <if>` and coexist with DHCP.
4. **DNS**: applied separately from the address (a DNS failure does not cancel the address change). With NetworkManager: `ipv4.dns` and `ipv4.ignore-auto-dns yes`; otherwise `resolvectl dns <if> …`.
5. **DHCP**: switching back runs `nmcli connection up <uuid>`, which clears the transient overrides and reactivates the saved profile. Without NetworkManager, addresses and the default route are flushed and `networkctl reconfigure` is called when available.
6. **Quit** (or closing the window) removes the aliases recorded in the session, restores DHCP and resets DNS. If restoration fails, the app stays open and shows the error. Closing the window quits rather than hiding: the tray is not guaranteed on every desktop.
7. One instance at a time: a `flock` in `$XDG_RUNTIME_DIR`; a second launch exits and prints a message on stderr.

Short version of the commands used:

| Action | NetworkManager (recommended) | Fallback (not managed by NM) |
|---|---|---|
| Read | `ip -j addr/link/route`, `nmcli connection show --active`, `nmcli -g IP4.DNS device show <if>` | same `ip` reads; `connection`/DNS empty |
| Static address | `nmcli device modify <if> ipv4.method manual …` | `ip -4 addr flush … scope global`, `ip -4 addr add`, `ip route replace default via …` |
| DHCP | `nmcli connection up <uuid>` | flush address + default route, `networkctl reconfigure` |
| DNS | `nmcli device modify <if> ipv4.dns … ipv4.ignore-auto-dns yes` | `resolvectl dns` / `resolvectl revert` |
| Alias | `ip -4 addr add/del` (always) | `ip -4 addr add/del` (always) |

### Elevation on Linux

The GUI runs unprivileged and never obtains root itself, and there is no greyed-out administrator screen: the whole interface stays usable and each modification goes through the helper, with a single password prompt reused for a few minutes.

- The app serializes one request to JSON and launches `pkexec /usr/libexec/fastipswitch/fastipswitch-helper`.
- The polkit action is `org.talevinn.fastipswitch.network`; `allow_any` and `allow_inactive` are `auth_admin`, `allow_active` is `auth_admin_keep`: one password prompt is reused for about five minutes.
- The helper revalidates everything before touching the network: protocol version, interface exists in `/sys/class/net` and is not `lo`, name length/allowed characters, CIDR invariants, gateway in the same subnet, at most 4 unicast DNS servers. It never trusts the GUI.
- Commands are executed without a shell, with separate arguments.
- A cancelled prompt (exit code 126) or a refused authorization (127) changes nothing and is reported in the UI.
- In release builds, only `/usr/libexec/fastipswitch/fastipswitch-helper` is accepted; `FASTIPSWITCH_HELPER` and a helper next to the GUI are honoured in debug builds only (development).

### Files on Linux

| Path | Purpose |
|---|---|
| `~/.config/fastipswitch/state.toml` | selected adapter, virtual IP list, last IP/gateway/DNS |
| `~/.local/state/fastipswitch/session.toml` | crash-recovery trace |
| `~/.local/state/fastipswitch/fastipswitch.log` | command log (rotated to `.log.1` above 1 MB) |
| `$XDG_RUNTIME_DIR/fastipswitch-<uid>.{lock,pid,sock}` | single-instance lock and PID (socket reserved for future activation) |
| `~/.local/bin/fastipswitch` | per-user install from the UI, if used |
| `~/.config/autostart/fastipswitch.desktop` | XDG autostart, only if enabled in the install dialog |
| `data/` next to the executable | portable mode when that directory is writable (same as Windows) |

`XDG_CONFIG_HOME`, `XDG_STATE_HOME`, `XDG_DATA_HOME` and `XDG_RUNTIME_DIR` are honoured. `apt remove fastipswitch` removes system files only; the user files above are kept.

### Linux limitations

- **Tray on GNOME**: the icon uses the StatusNotifierItem/AppIndicator protocol; GNOME needs the AppIndicator extension. Without it, use the window (closing it quits cleanly).
- **NetworkManager recommended**: non-managed interfaces get volatile `ip` changes that the system may overwrite; DNS then requires `resolvectl` (systemd-resolved). There is no complete systemd-networkd integration.
- **Aliases are volatile**: they are added with `ip` even on NM interfaces, and can disappear when NetworkManager reapplies or changes the profile; re-enable them.
- **No AppImage**, no Flatpak/Snap, no signed repository; `.deb` only.
- No static route management and no IPv6 configuration (IPv4 only, as on Windows).
- One instance at a time; data is per user.
- Theme detection depends on `gsettings`, `xfconf-query` or `kreadconfig`; falls back to light.
- pkexec needs a polkit authentication agent, normally provided by the desktop session.

### Troubleshooting on Linux

1. **List what the app sees**: `fastipswitch --print-adapters` prints one line per interface (`name`, `index`, `mac`, `up`, `dhcp`, `gw`, `dns`, `conn`, addresses) and exits; it only reads.
2. **Read the log**: `~/.local/state/fastipswitch/fastipswitch.log` (or `data/fastipswitch.log` in portable mode). It records `ip`/`nmcli` calls and helper failures.
3. **Check NetworkManager**: `nmcli device status` (is the interface `connected` or `unmanaged`?) and `nmcli -t -f UUID,DEVICE connection show --active` (UUID used by the DHCP restore).
4. **Check elevation**: `pkexec /usr/libexec/fastipswitch/fastipswitch-helper </dev/null` should prompt and then return a JSON error (empty request). "Not authorized" or no prompt usually means the policy is missing or bypassed: reinstall the `.deb`. Depending on the distribution, polkit/pkexec refusals are in `journalctl -b` or the authentication log.
5. **Helper not found**: check `ls -l /usr/libexec/fastipswitch/fastipswitch-helper`; in development, set `FASTIPSWITCH_HELPER` to its path.
6. **Restore manually**:
   ```sh
   # NetworkManager: reactivate the saved profile (also clears transient overrides)
   nmcli connection up <uuid>
   # without NetworkManager
   sudo ip -4 addr flush dev <if> scope global
   sudo ip route flush default dev <if>
   sudo networkctl reconfigure <if>
   # DNS without NetworkManager, or to reset it
   sudo resolvectl revert <if>
   # remove one alias
   sudo ip -4 addr del <cidr> dev <if>
   ```
7. **Uninstall**: `sudo apt remove fastipswitch` (or `apt purge`). User configuration and logs are not deleted; remove `~/.config/fastipswitch` and `~/.local/state/fastipswitch` to start from scratch.

### See also

- [Detailed Linux guide](docs/linux.md) — architecture, protocol, full command matrix, development and CI.
- [Cross-platform status](docs/cross-platform.md) — what is shared, what is platform-specific, macOS status.
- [Packaging notes](packaging/README.md) — `.deb` contents, build variables, runtime dependencies.

## License

GPL-3.0-or-later — see `LICENSE` and `NOTICE`.
