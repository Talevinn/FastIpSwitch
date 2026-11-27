**English** | [Français](linux.fr.md)

# FastIpSwitch on Linux

Status: **initial Linux port**. The same Slint UI drives a Linux backend built on NetworkManager (`nmcli`) with an `ip`/`resolvectl` fallback. Windows behaviour is unchanged. The port is covered by unit tests and a CI build; a hardware/desktop validation matrix is not published yet.

- User-facing summary: [README — Linux](../README.md#linux)
- Cross-platform layout: [cross-platform.md](cross-platform.md)
- Package details: [packaging/README.md](../packaging/README.md)

## Requirements

| Component | Role | Notes |
|---|---|---|
| `iproute2` (`ip`) | Reading, aliases, fallback addressing | Required; reading fails without it |
| NetworkManager (`nmcli`) | Primary address, DHCP, DNS | Recommended; Debian/Ubuntu/Mint default |
| `pkexec` + polkit | Elevation of the helper | `pkexec` package on Debian 12+/Ubuntu 24.04 |
| `resolvectl` (systemd-resolved) | DNS on non-NM interfaces | Optional; no DNS change without it |
| `networkctl` (systemd-networkd) | Reconfigure after a fallback DHCP restore | Optional |
| Slint/winit GUI libraries | Window rendering | Wayland or X11, EGL or GL |

The GUI is designed to run unprivileged and never obtains root itself. Every network change goes through a small privileged helper launched with `pkexec`.

## Install the `.deb`

No prebuilt package is published. Build it from the repository root:

```sh
packaging/build-deb.sh
sudo apt install ./packaging/.build/dist/fastipswitch_0.1.0_amd64.deb
```

`apt install ./file.deb` resolves `Depends` and installs `Recommends` (including `network-manager`) by default. The package installs:

| Item | Path |
|---|---|
| GUI | `/usr/bin/fastipswitch` |
| Helper | `/usr/libexec/fastipswitch/fastipswitch-helper` (`root:root`, `0755`) |
| polkit action | `/usr/share/polkit-1/actions/org.talevinn.fastipswitch.policy` (`0644`) |
| Desktop entry | `/usr/share/applications/org.talevinn.fastipswitch.desktop` |
| Icons | `/usr/share/icons/hicolor/<size>x<size>/apps/fastipswitch.png` (16 to 512) |
| Man page | `/usr/share/man/man1/fastipswitch.1.gz` |
| AppStream | `/usr/share/metainfo/org.talevinn.fastipswitch.metainfo.xml` |

The package does not enable autostart. There is **no AppImage**: the helper and the polkit policy must be installed system-wide, which a single portable file does not do. The package is not signed and no APT repository is provided.

## Build from source

Rust stable (edition 2024, ≥ 1.88) plus `pkg-config` and the Slint/winit headers:

```sh
sudo apt install build-essential pkg-config \
    libfontconfig1-dev libfreetype-dev libxkbcommon-dev \
    libwayland-dev libegl1-mesa-dev libgl1-mesa-dev
cargo build --release
```

- `libfreetype-dev` exists on Debian 12+ and Ubuntu 24.04; use `libfreetype6-dev` on older releases.
- Add `libx11-xcb-dev` for X11 builds/sessions.
- Output: `target/release/fastipswitch` and `target/release/fastipswitch-helper`.
- `cargo build --release` builds both binaries declared in `Cargo.toml`; the `.deb` script builds them explicitly with `--bin`.

For normal use the helper must be installed system-wide. The in-app **Install** button is a per-user installation only: it copies the GUI to `~/.local/bin/fastipswitch`, writes icons under `~/.local/share/icons/hicolor/`, a desktop entry under `~/.local/share/applications/`, and optionally `~/.config/autostart/fastipswitch.desktop`. It never installs the helper or the polkit policy (a user-writable helper would defeat the elevation model). If `/usr/bin/fastipswitch` exists, the button only manages the autostart entry.

## Usage

1. **Select the interface.** Loopback and container/bridge links (`docker*`, `veth*`, `virbr*`, `vmnet*`, `br-*`) are hidden. VPN links (`tun*`, `wg*`, …) are kept but often reported as down because `tun`/`wg` report `operstate: UNKNOWN`; an interface counts as up only when `operstate == UP` **and** the `UP` flag is set.
2. **Primary address.** Enter a CIDR, an optional gateway and optional DNS, then **Apply**. Validation is the same as on Windows: network/broadcast addresses rejected (for prefixes ≤ 30), gateway must be in the same subnet, DNS must be unicast IPv4.
3. **Virtual IPs.** `+`, enter a CIDR, enable the switch. Aliases are added with `ip -4 addr add` and removed with `ip -4 addr del`.
4. **DHCP.** Switching back reactivates the saved NetworkManager profile, or flushes the fallback configuration.
5. **Quit / close.** Removes the aliases recorded in `session.toml`, restores DHCP and resets DNS. Closing the window quits (it does not hide): `hides_on_close` is false on Linux because no tray is guaranteed. If restoration fails, the window stays open and shows the error.

Only one instance runs at a time: an exclusive `flock` on `$XDG_RUNTIME_DIR/fastipswitch-<uid>.lock`. A second launch exits after printing a message on stderr. A Unix socket name is reserved for a future "show the window" activation request, but no server is started yet.

### What is applied

| Action | NetworkManager (`nmcli device status` says `connected`) | Fallback (not managed by NM) |
|---|---|---|
| Read interfaces | `ip -j addr show`, `ip -j link show`, `ip -j route show default` | same `ip` reads |
| Read NM connection / DNS | `nmcli -t -f UUID,DEVICE connection show --active`; `nmcli -g IP4.DNS device show <if>` | `connection` and `dns` stay empty |
| Static address | `nmcli device modify <if> ipv4.method manual ipv4.addresses <cidr> ipv4.gateway <gw>` (empty gateway clears it) | `ip -4 addr flush dev <if> scope global`, `ip -4 addr add <cidr> dev <if>`, optional `ip route replace default via <gw> dev <if>` |
| DHCP | `nmcli connection up <uuid>` (reactivates the profile, clears transient overrides) | `ip -4 addr flush … scope global`, `ip route flush default dev <if>`, then `networkctl reconfigure <if>` if present |
| Static DNS | `nmcli device modify <if> ipv4.dns "<servers>" ipv4.ignore-auto-dns yes` | `resolvectl dns <if> <servers…>`; error if `resolvectl` is absent |
| Reset DNS | `nmcli device modify <if> ipv4.dns "" ipv4.ignore-auto-dns no` | `resolvectl revert <if>`; silently no-op without `resolvectl` |
| Add / remove alias | `ip -4 addr add/del <cidr> dev <if>` (always) | same |
| DHCP coexistence toggle | no-op (aliases coexist with DHCP natively) | no-op |

Important details:

- `nmcli device modify` changes the **active** connection in memory; the on-disk profile is not persisted. This is the equivalent of the Windows `store=active` intent.
- Aliases are always applied with `ip`, including on NM links. NetworkManager may drop them when it reapplies or switches the profile; re-enable them.
- Reading uses `ip -j`; an `ip` failure is fatal for the read, while an `nmcli` failure is tolerated (`connection: None`, no DNS).
- DNS is applied by a separate operation after the address, so a DNS failure does not roll back the address change.
- `dhcp` is derived from the `dynamic` flag of `ip addr`. APIPA/link-local addresses (`169.254.x.x`) are ignored when choosing the primary address, as on Windows.
- The gateway is the lowest-metric IPv4 default route for that device.

## Architecture

```
main.rs (Slint UI)
   │  Jobs / Events (mpsc)
   ▼
worker.rs ──► platform::backend() ──► platform::linux::LinuxBackend
   │                                      ├─ read.rs  : ip -j + nmcli queries (unprivileged)
   │                                      └─ ops.rs   : builds helper::protocol::Request
   │                                                     │
   │                                                     ▼
   │                                          platform/linux/helper.rs
   │                                          pkexec /usr/libexec/.../fastipswitch-helper
   │                                                     │ JSON on stdin
   │                                                     ▼
   │                                          src/bin/fastipswitch-helper.rs
   │                                          helper::validate  →  helper::exec
   │                                                     │ JSON on stdout
   ▼                                                     ▼
UI update ◄────────────────────────────────────────── Response { ok, message? }
```

- `src/platform/mod.rs` is the shared facade: common `AdapterInfo`, `IfaceId`, `IpEntry`, `AdapterRef`, the `NetworkBackend` trait (implemented by `platform::windows` and `platform::linux`), and the system helpers (elevation, instance, paths, install, theme).
- `src/platform/linux/read.rs` parses `ip -j` output with `serde_json`; pure parsing functions are unit-tested against fixtures in `tests/fixtures/`.
- `src/platform/linux/ops.rs` never touches the network: it serializes one request and calls the helper.
- `src/helper/protocol.rs` is the frozen wire format (`VERSION = 1`, one operation per request).
- `src/helper/validate.rs` revalidates the request in the privileged process; `src/helper/exec.rs` builds the actual `nmcli`/`ip`/`resolvectl` argument vectors.
- Commands never go through a shell: `Command::new(program).args(...)`.

### Protocol

Request (stdin), one JSON document per process:

```json
{"version":1,"op":"apply_static","ifname":"eth0","cidr":"192.168.3.10/24","gateway":"192.168.3.1","dns":[]}
```

Operations: `apply_static`, `set_dhcp`, `set_dns`, `reset_dns`, `add_alias`, `remove_alias` (`op` is `snake_case`; `cidr` is a `"a.b.c.d/prefix"` string).

Response (stdout):

```json
{"ok":true}
{"ok":false,"message":"…"}
```

The helper exits with status 0 on success and 1 on error. When no valid JSON response is available, the GUI maps `pkexec` exit code 126 to "authentication cancelled" and 127 to "authentication failed"; any other non-zero code is reported as a helper failure. A request is never batched: each UI action that needs privileges starts the helper once and therefore reuses the current polkit authorization.

### Elevation and polkit

The installed policy is [`packaging/polkit/org.talevinn.fastipswitch.policy`](../packaging/polkit/org.talevinn.fastipswitch.policy):

- action `org.talevinn.fastipswitch.network`;
- `allow_any = auth_admin`, `allow_inactive = auth_admin`, `allow_active = auth_admin_keep`: the active session authenticates once and the authorization is kept for about five minutes;
- `org.freedesktop.policykit.exec.path` pins the helper to `/usr/libexec/fastipswitch/fastipswitch-helper`.

The helper revalidates independently of the GUI:

- protocol version must be 1;
- interface name: non-empty, ≤ 15 characters (`IFNAMSIZ - 1`), first character alphanumeric, allowed characters `[A-Za-z0-9_.:@-]`, not `lo`, `.` or `..`, and must exist in `/sys/class/net`;
- CIDR rebuilt through `Cidr::new` (prefix ≤ 32, not unspecified/loopback/multicast/broadcast, not network/broadcast for prefixes ≤ 30);
- gateway unicast, different from the local address/network/broadcast, and inside the CIDR;
- DNS list non-empty, at most 4 unicast IPv4 servers.

In release builds the app only accepts the system helper path. `FASTIPSWITCH_HELPER` and a `fastipswitch-helper` binary next to the GUI are honoured in debug builds only, so an unprivileged user cannot redirect the `pkexec` call in a release install.

## Files and environment

| Path | Purpose |
|---|---|
| `$XDG_CONFIG_HOME/fastipswitch/state.toml` (default `~/.config/...`) | selected adapter, alias list, last IP/gateway/DNS |
| `$XDG_STATE_HOME/fastipswitch/session.toml` (default `~/.local/state/...`) | crash-recovery trace of temporary changes |
| `$XDG_STATE_HOME/fastipswitch/fastipswitch.log` | command log; rotated to `fastipswitch.log.1` when it exceeds 1 MB (one old file kept) |
| `$XDG_RUNTIME_DIR/fastipswitch-<uid>.lock` | single-instance `flock` (default `/tmp` if the variable is unset) |
| `$XDG_RUNTIME_DIR/fastipswitch-<uid>.pid` | PID of the lock holder (`0600`) |
| `$XDG_RUNTIME_DIR/fastipswitch-<uid>.sock` | reserved socket name, not served yet |
| `$XDG_DATA_HOME/applications/fastipswitch.desktop`, `$XDG_DATA_HOME/icons/hicolor/...` | per-user install from the UI |
| `$XDG_CONFIG_HOME/autostart/fastipswitch.desktop` | XDG autostart, if enabled |
| `data/` next to the executable | portable mode when the directory is writable (takes precedence over XDG) |
| `/usr/libexec/fastipswitch/fastipswitch-helper` | privileged helper (package only) |
| `/usr/share/polkit-1/actions/org.talevinn.fastipswitch.policy` | polkit action (package only) |

Environment variables:

| Variable | Effect |
|---|---|
| `FASTIPSWITCH_LANG=fr\|en` | forces the UI language (otherwise `LC_ALL`, `LC_MESSAGES`, `LANG`; French when the first non-empty value starts with `fr`) |
| `FASTIPSWITCH_THEME=dark\|light` | forces the theme (otherwise GNOME/Cinnamon/XFCE/KDE detection, fallback light) |
| `FASTIPSWITCH_HELPER=/path` | helper path override, **debug builds only** |
| `FASTIPSWITCH_DRY_RUN=1` | the GUI logs the helper request and returns success without launching the helper; no network change |

`--print-adapters` prints one line per interface and exits without elevation:

```
name  index=<ifindex>  mac=<mac>  up=<bool>  dhcp=<bool>  gw=<ipv4|empty>  dns=<ip,ip|empty>  conn=<uuid|empty>  <cidr[(dhcp)], …>
```

(fields are tab-separated; `conn` is the active NetworkManager UUID).

## Development

```sh
cargo build                 # debug: target/debug/{fastipswitch,fastipswitch-helper}
cargo test --all-targets
cargo fmt --check
```

In debug, the helper is looked up in this order: `FASTIPSWITCH_HELPER` if set and a file, then `/usr/libexec/fastipswitch/fastipswitch-helper`, then a `fastipswitch-helper` sibling of the running GUI (i.e. `target/debug/`). After `cargo build`, running the GUI from `target/debug/` therefore works without installing anything.

Diagnostic helper call (runs the real operation, root only):

```sh
printf '%s' '{"version":1,"op":"set_dhcp","ifname":"eth0"}' |
  sudo /usr/libexec/fastipswitch/fastipswitch-helper
```

It prints the JSON response. Use a restore-only operation (`set_dhcp`, `remove_alias`) when testing; `FASTIPSWITCH_DRY_RUN` avoids real changes in the GUI.

Tests are unprivileged and do not modify the host network: they exercise the `ip -j`/`nmcli` parsers with fixtures, the protocol round-trip, argument construction, validation rules, XDG path resolution, installer writes into temporary directories, and the dry-run path (`FASTIPSWITCH_DRY_RUN` returns before resolving or launching the helper).

### CI

Woodpecker (`.woodpecker.yml`) runs on push and pull requests with `rust:1.99-bookworm`:

1. install `pkg-config libfontconfig1-dev libfreetype-dev libxkbcommon-dev libwayland-dev libegl1-mesa-dev libgl1-mesa-dev`;
2. `cargo fmt --check`;
3. `cargo test --all-targets`;
4. `cargo build --release`.

The Windows backend is not compiled there (it needs Windows APIs); there is no macOS job. The `.deb` build is not run in CI.

### Building the `.deb`

`packaging/build-deb.sh`:

1. reads the version from `Cargo.toml` (`VERSION=` overrides);
2. runs `cargo build --release --bin fastipswitch --bin fastipswitch-helper`;
3. stages the tree under `packaging/.build/stage/` (gitignored);
4. writes `packaging/.build/dist/fastipswitch_<version>_<arch>.deb`.

Useful variables: `SKIP_BUILD=1`, `VERSION=`, `ARCH=` (default `dpkg --print-architecture`), `DIST_DIR=`, `CARGO_TARGET_DIR=`, `SKIP_VALIDATE=1`, `POLKIT_POLICY=`, `ALLOW_MISSING_POLKIT=1`. The script refuses to build without the polkit policy unless `ALLOW_MISSING_POLKIT=1` is set (test package with broken elevation). See [packaging/README.md](../packaging/README.md) for the full dependency rationale and staging example.

## Limitations and open tracks

- **macOS is not implemented.** Linux is the only platform added so far.
- **No AppImage / Flatpak / Snap**, no signed repository: only `.deb`, built locally. A portable format would need a separate installation path for the helper and policy.
- **NetworkManager is the happy path.** Without it, the primary address and gateway use volatile `ip` commands and DNS requires `resolvectl`; there is no complete systemd-networkd backend and no profile persistence.
- **Aliases are not persisted** by NetworkManager and can vanish when the profile is reapplied.
- **No static route management** and **no IPv6** configuration (IPv4 only, as on Windows).
- **Tray** uses StatusNotifierItem/AppIndicator; GNOME needs the AppIndicator extension. Closing the window quits by design.
- **One instance**; the activation socket is reserved but not served.
- **Theme detection** relies on desktop tools (`gsettings`, `xfconf-query`, `kreadconfig5/6`) and falls back to light.
- **Hardware/desktop validation matrix not published**: the port is tested by unit tests and CI compilation, not by an automated matrix of Wi-Fi/Ethernet chipsets and desktops.
- Some inherited UI strings still mention Windows or administrator rights; on Linux the lock screen is never shown (`is_admin()` is true and elevation is per-action), so they are not reachable in normal use.

## Troubleshooting

| Symptom | Check |
|---|---|
| No interface listed | `fastipswitch --print-adapters`; `ip -j link show`; container/bridge names are hidden by design |
| Address not applied | `nmcli device status` (interface `connected`?), log, then `ip -4 addr show dev <if>` |
| DNS not applied | NM: `nmcli device show <if> \| grep IP4.DNS`; fallback: `resolvectl status <if>`; `resolvectl` must exist |
| Prompt does not appear / "not authorized" | policy present in `/usr/share/polkit-1/actions/`; `pkexec` installed; polkit agent running; `journalctl -b` |
| Helper not found | `ls -l /usr/libexec/fastipswitch/fastipswitch-helper`; in debug set `FASTIPSWITCH_HELPER` |
| Changes disappear | NM reapplied the profile: aliases are volatile; non-NM changes are not persisted |
| Second launch does nothing | single-instance lock; message on stderr; check `$XDG_RUNTIME_DIR/fastipswitch-<uid>.lock` |

Manual restore:

```sh
# NetworkManager: reactivate the saved profile
nmcli connection up <uuid>
# without NetworkManager
sudo ip -4 addr flush dev <if> scope global
sudo ip route flush default dev <if>
sudo networkctl reconfigure <if>
# DNS
nmcli device modify <if> ipv4.dns "" ipv4.ignore-auto-dns no
sudo resolvectl revert <if>
# one alias
sudo ip -4 addr del <cidr> dev <if>
```

Uninstall the package with `sudo apt remove fastipswitch` (or `apt purge`). User state under `~/.config/fastipswitch` and `~/.local/state/fastipswitch` is never removed by the package.

## See also

- [README — Linux section](../README.md#linux)
- [Cross-platform status](cross-platform.md)
- [Packaging (.deb)](../packaging/README.md)
- [Man page source](../packaging/man/fastipswitch.1)
