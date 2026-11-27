# Packaging Linux (.deb) — FastIpSwitch

Paquet Debian pour Debian, Ubuntu et Linux Mint. Les chemins installés sont
ceux attendus par le code Rust :

| Élément | Chemin installé |
|---|---|
| Binaire GUI | `/usr/bin/fastipswitch` |
| Helper privilégié | `/usr/libexec/fastipswitch/fastipswitch-helper` |
| Action polkit | `/usr/share/polkit-1/actions/org.talevinn.fastipswitch.policy` |
| Entrée de menu | `/usr/share/applications/org.talevinn.fastipswitch.desktop` |
| Icônes | `/usr/share/icons/hicolor/{16,24,32,48,64,128,256,512}x*/apps/fastipswitch.png` |
| AppStream | `/usr/share/metainfo/org.talevinn.fastipswitch.metainfo.xml` |
| Documentation | `/usr/share/doc/fastipswitch/{copyright,changelog.Debian.gz}` |
| Page de manuel | `/usr/share/man/man1/fastipswitch.1.gz` |

L'identifiant desktop (`org.talevinn.fastipswitch`), l'action polkit et le nom
d'icône (`fastipswitch`) doivent rester alignés avec le code et la policy
fournie dans `packaging/polkit/`.

## Prérequis de build

- Rust stable récent (≥ 1.88, édition 2024) et `cargo` ;
- `dpkg-deb` (paquet `dpkg-dev` sur Debian/Ubuntu) ;
- `gzip` ;
- outils de compilation Slint : `pkg-config`, en-têtes fontconfig/freetype,
  X11/Wayland (`libxkbcommon-dev`, `libwayland-dev`, `libx11-xcb-dev`,
  `libegl1-mesa-dev`) ;
- facultatif, pour la validation automatique :
  `desktop-file-utils` (`desktop-file-validate`) et `appstream`
  (`appstreamcli`).

Sous Debian/Ubuntu :

```sh
sudo apt install build-essential pkg-config dpkg-dev gzip \
    libfontconfig1-dev libfreetype6-dev libxkbcommon-dev \
    libwayland-dev libx11-xcb-dev libegl1-mesa-dev \
    desktop-file-utils appstream
```

## Construction

Depuis la racine du dépôt :

```sh
packaging/build-deb.sh
```

Le script :

1. lit la version dans `Cargo.toml` (surchargeable avec `VERSION=`) ;
2. lance `cargo build --release --bin fastipswitch --bin fastipswitch-helper` ;
3. assemble l'arborescence dans `packaging/.build/stage/` (ignorée par git) ;
4. produit `packaging/.build/dist/fastipswitch_<version>_<arch>.deb`.

Variables utiles :

| Variable | Effet |
|---|---|
| `SKIP_BUILD=1` | réutilise les binaires de `target/release/` (tests de staging) |
| `VERSION=x.y.z` | force la version du paquet |
| `ARCH=arm64` | force l'architecture (défaut : `dpkg --print-architecture`) |
| `DIST_DIR=/tmp/out` | répertoire de sortie du `.deb` |
| `SKIP_VALIDATE=1` | ignore `desktop-file-validate` / `appstreamcli` |
| `ALLOW_MISSING_POLKIT=1` | construit sans la policy polkit (paquet de test, élévation cassée) |
| `POLKIT_POLICY=…` | policy à installer (défaut : `packaging/polkit/`) |
| `CARGO_TARGET_DIR=…` | respecté, comme pour `cargo` |

Exemple de staging sans recompilation (binaires factices) :

```sh
mkdir -p /tmp/fake-target/release
printf '#!/bin/sh\nexit 0\n' >/tmp/fake-target/release/fastipswitch
cp /tmp/fake-target/release/fastipswitch /tmp/fake-target/release/fastipswitch-helper
chmod +x /tmp/fake-target/release/fastipswitch*
CARGO_TARGET_DIR=/tmp/fake-target SKIP_BUILD=1 ALLOW_MISSING_POLKIT=1 \
    DIST_DIR=/tmp/deb packaging/build-deb.sh
```

## Vérifications

```sh
dpkg-deb --info  packaging/.build/dist/fastipswitch_0.1.0_amd64.deb
dpkg-deb --contents packaging/.build/dist/fastipswitch_0.1.0_amd64.deb
# contrôle des droits/du propriétaire dans l'archive
dpkg-deb --contents packaging/.build/dist/fastipswitch_0.1.0_amd64.deb | grep libexec
```

Attendu : helper en `-rwxr-xr-x root/root`, policy en `0644 root/root`,
binaire GUI en `/usr/bin/fastipswitch`, icônes dans les huit tailles.

## Installation et désinstallation

```sh
sudo apt install ./fastipswitch_0.1.0_amd64.deb   # installe les dépendances
# ou, sans résolution de dépendances :
sudo dpkg -i fastipswitch_0.1.0_amd64.deb && sudo apt -f install

sudo apt remove fastipswitch       # retire les fichiers système
sudo apt purge fastipswitch        # idem (aucun fichier de configuration système)
```

La configuration utilisateur n'est **jamais** supprimée par le paquet :
`~/.config/fastipswitch/` (préférences, IP virtuelles),
`~/.local/state/fastipswitch/` (journal) et
`~/.config/autostart/fastipswitch.desktop` (si activé) restent en place.
Pour repartir de zéro, les supprimer manuellement.

## Dépendances runtime retenues

- `Depends` : `libc6`, `libgcc-s1`, `libfontconfig1`, `libfreetype6`,
  `libxkbcommon0`, `libwayland-client0 | libx11-6`, `libegl1 | libgl1`,
  `iproute2`, `pkexec | policykit-1`.
- `Recommends` : `network-manager`.

Justifications :

- **iproute2** est indispensable : lecture via `ip -j` et alias via `ip addr`.
- **libgcc-s1** figure dans `ldd` des deux binaires Rust.
- **network-manager** est en `Recommends` et non en `Depends` : les IP
  virtuelles fonctionnent avec `ip` seul et certaines installations utilisent
  `systemd-networkd`. `apt` installe les `Recommends` par défaut, donc `nmcli`
  est présent sur Ubuntu/Mint standard. Sans NetworkManager, l'adresse
  principale et les DNS (via `nmcli`) ne sont pas modifiables.
- **pkexec | policykit-1** : l'élévation passe par `pkexec`. Depuis Debian 12
  et Ubuntu 24.04, `/usr/bin/pkexec` est dans le paquet `pkexec` et non dans
  `polkitd` ; dépendre de `polkitd` seul laisserait l'application sans moyen
  d'élévation. `policykit-1` couvre Debian 11 / Ubuntu 22.04.
- Les bibliothèques graphiques correspondent au backend Slint winit +
  femtovg (Wayland ou X11, EGL ou GL).

## Limites connues

- **polkit obligatoire** : sans policy installée (ou sans `pkexec`), la GUI
  démarre mais toute modification réseau échoue avec un message d'erreur.
  La policy est fournie par `packaging/polkit/` (autre lot) ; le script refuse
  de construire sans elle sauf `ALLOW_MISSING_POLKIT=1`.
- **AppImage** : non traité ici. Une AppImage embarque le binaire mais pas la
  policy polkit ni le helper dans `/usr/libexec` ; il faudrait installer la
  policy séparément (ou embarquer un lanceur qui l'installe), ce qui dépasse
  le périmètre de ce paquet.
- **Une seule architecture par `.deb`** : relancer le script avec `ARCH=…`
  (ou un build croisé) pour chaque architecture visée.
- **AppStream** : pas de capture d'écran embarquée (les PNG de `docs/` ne sont
  pas installés). Ajouter une `<screenshot>` avec une URL stable (par ex.
  Codeberg) avant toute soumission à un dépôt public.
- `StartupWMClass=fastipswitch` suppose que Slint/winit expose ce nom de
  classe ; à vérifier sur une session réelle (Wayland et X11).
- Le paquet n'est pas signé : prévoir `debsigs`/`reprepro` côté dépôt.

## Contenu du répertoire

```
packaging/
├── build-deb.sh                 # script de construction
├── README.md                    # ce document
├── deb/
│   ├── DEBIAN/
│   │   ├── control              # modèle (substitutions @VERSION@, @ARCH@, @INSTALLED_SIZE@)
│   │   ├── postinst             # caches menu/icônes
│   │   ├── prerm                # ne touche pas à la config utilisateur
│   │   └── postrm               # rafraîchit les caches au retrait
│   ├── org.talevinn.fastipswitch.desktop
│   ├── changelog                # modèle, compressé en changelog.Debian.gz
│   └── copyright                # DEP-5
├── appstream/
│   └── org.talevinn.fastipswitch.metainfo.xml
└── man/
    └── fastipswitch.1
```
