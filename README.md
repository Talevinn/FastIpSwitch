**English** | [Français](README.fr.md)

# FastIpSwitch — V0.1

**Maintainer:** Valentin PICARD ([@Talevinn](https://codeberg.org/Talevinn))

## Download

**[Download FastIpSwitch V0.1 for Windows](https://codeberg.org/Talevinn/FastIpSwitch/releases/download/V0.1/FastIpSwitch-V0.1.exe)** — Windows 10/11, x64, run as administrator. [Release notes and checksum](https://codeberg.org/Talevinn/FastIpSwitch/releases/tag/V0.1).

Windows utility (Rust + [Slint](https://slint.dev)) to **switch the IPv4 address quickly** and **add or remove virtual IPs on the fly**, so you can commission or interrogate equipment that sits on a different subnet: brand-new devices still on their default IP, legacy gear with a hard-coded address, test benches, AV hardware, etc.

All network changes go through **`netsh interface ipv4`**. The UI follows the Windows light/dark theme and the system language (English/French).

| Dark theme | Light theme | Without elevation |
|---|---|---|
| ![dark theme](docs/screenshot-dark.png) | ![light theme](docs/screenshot-light.png) | ![locked UI](docs/screenshot-locked.png) |

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

## Why administrator rights are required

Windows does not expose a user-level API to change an interface's IP configuration: `netsh interface ipv4` (like `New-NetIPAddress`) requires an elevated token. Without elevation, the commands fail and nothing changes.

FastIpSwitch is therefore designed to run **elevated**, but it does not force UAC at startup:

- the manifest is `asInvoker`; without elevation, the app still opens, **fully greyed out**, with an “Administrator rights required” panel and a **Restart as administrator** button;
- shortcuts created by the installer pass `--elevate` to request UAC at launch;
- automatic startup uses a **scheduled task** `ONLOGON` with `/RL HIGHEST`, so it runs elevated at sign-in without a UAC prompt.

The app applies addressing (IP, mask, gateway, aliases) in **`store=active`**: the persistent configuration is not modified, and a reboot restores the configuration stored in Windows (usually DHCP). **DNS is the exception**: `netsh set dnsservers` has no `store=active`, so it is written persistently and restored on a clean Quit.

## How it works

- **Virtual IPs**: `netsh add address ... skipassource=true store=active`. `skipassource` keeps Windows from choosing those addresses as the source for outbound traffic, while still answering ARP/ping/HTTP.
- **DHCP coexistence**: Windows disables DHCP when a manual IP is added. FastIpSwitch enables `dhcpstaticipcoexistence` before the add to keep the DHCP connection, then checks that DHCP is still active; otherwise the operation is cancelled and DHCP restored.
- **Static primary address**: `set address source=static ... store=active`, then active aliases are re-added (this command clears manual addresses).
- **DNS**: `set dnsservers source=static` plus `add dnsservers` for additional servers; back to `source=dhcp` on a DHCP restore. These commands have no `store=active`, so static DNS is **persistent**. It is normally restored on exit, but after a crash followed by a reboot without relaunching the app it may remain static: relaunch FastIpSwitch or reset it manually.
- **Adapter refresh and APIPA**: selecting an adapter re-reads the live state. `169.254.x.x` (APIPA/link-local) is ignored: it means there is no DHCP lease. The IP field is left empty with a hint, and a 2-second background check fills it in when a lease arrives; if the current adapter goes down, the app switches to a newly usable adapter (unless you explicitly selected a disconnected adapter to configure it).
- **Validation**: `.0` and `.255` are rejected for a /24 (network/broadcast), the gateway must be in the same subnet, duplicates are rejected, DNS entries must be valid IPv4.

## Restart, quit and failure recovery

| Event | What happens |
|---|---|
| **Window close button** | In elevated mode, minimizes to the notification area; configuration stays active while the app runs. Without admin, it quits. |
| **Quit** (button or tray) | Removes aliases, switches back to DHCP, resets DNS, then exits. If restore fails, the app stays open and shows the error. |
| **Windows restart** | Returns to the persistent configuration (usually DHCP), even after a crash. Static DNS may remain if the app was not relaunched. |
| **Crash / kill** | `session.toml` is kept; on the next **elevated** launch, targeted cleanup + DHCP restore. |
| **Launch without admin** | Greyed UI + **Restart as administrator**; no network command, no session recovery. |
| **netsh failure** | Error message, rollback of the add, log in `data/fastipswitch.log` (or `%LOCALAPPDATA%`). |

### If something goes wrong

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

## Installation

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

## Usage

1. Select the adapter.
2. **Primary address**: enter `192.168.2.10/24`, the gateway and DNS if needed, then **Apply**.
3. **Virtual IPs**: `+`, enter `192.168.3.10/24`, enable the switch. The IP is added without dropping DHCP.
4. The **DHCP** switch: ON = return to DHCP, OFF = freeze the displayed address as static.
5. **Quit** to restore everything.

`192.168.1.0/24` is rejected: `.0` is the network address; use a host address (`.1` to `.254`).

## Language and theme

- Language: English unless the Windows UI language is French; override with `FASTIPSWITCH_LANG=fr` or `en`.
- Theme: follows Windows (dark rack / light aluminium). Override with `FASTIPSWITCH_THEME=dark` or `light`.

## Build

Requirements: recent stable Rust (≥ 1.88), Visual Studio Build Tools (MSVC), Windows 10/11.

```powershell
cargo build --release
```

Binary: `target\release\fastipswitch.exe`. `.cargo/config.toml` enables `+crt-static` for a self-contained exe. The manifest is `asInvoker` (DPI PerMonitorV2).

## Files

| Path | Purpose |
|---|---|
| `data\state.toml` / `%APPDATA%\FastIpSwitch\state.toml` | adapter, virtual IP list, last IP/gateway/DNS |
| `data\session.toml` / `%LOCALAPPDATA%\FastIpSwitch\session.toml` | crash-recovery trace |
| `data\fastipswitch.log` / `%LOCALAPPDATA%\FastIpSwitch\fastipswitch.log` | netsh command log (rotated) |
| `%LOCALAPPDATA%\Programs\FastIpSwitch` | per-user installation |

## Known limitations

- No static route management (devices must be on the same L2 segment).
- DNS is reset to automatic on a DHCP restore; the previous static value is not remembered.
- One instance at a time.
- Per-user installation only (no machine-wide install).

## License

GPL-3.0-or-later — see `LICENSE` and `NOTICE`.
