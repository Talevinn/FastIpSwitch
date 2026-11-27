[English](linux.md) | **Français**

# FastIpSwitch sous Linux

État : **premier portage Linux**, durci après une revue de sécurité. La même interface Slint pilote un backend Linux fondé sur NetworkManager (`nmcli`) avec un repli `ip`/`resolvectl`. Le backend et le déroulé Windows sont inchangés. Le portage est couvert par des tests unitaires et d'intégration exécutés en local ; aucune matrice de validation matérielle/bureau n'est publiée à ce jour.

- Résumé utilisateur : [README — Linux](../README.fr.md#linux)
- Vue multiplateforme : [cross-platform.fr.md](cross-platform.fr.md)
- Détails du paquet : [packaging/README.md](../packaging/README.md)

## Prérequis

| Composant | Rôle | Remarques |
|---|---|---|
| `iproute2` (`ip`) | Lecture, alias, adressage de repli | Indispensable ; la lecture échoue sans lui |
| NetworkManager (`nmcli`) | Adresse principale, DHCP, DNS quand l'interface est gérée | Recommandé ; défaut Debian/Ubuntu/Mint |
| `pkexec` + polkit | Élévation du helper | Paquet `pkexec` sur Debian 12+/Ubuntu 24.04 |
| `resolvectl` (systemd-resolved) | DNS quand l'interface n'est pas gérée par NM | Optionnel ; sans lui, pas de changement DNS |
| `networkctl` (systemd-networkd) | Retour DHCP de repli | Obligatoire pour le retour DHCP sans NM |
| Bibliothèques GUI Slint/winit | Rendu de la fenêtre | Wayland ou X11, EGL ou GL |

La GUI est conçue pour tourner sans privilèges et n'obtient jamais root elle-même. Chaque modification réseau passe par un petit helper privilégié lancé avec `pkexec`.

## Installer le `.deb`

Aucun paquet précompilé n'est publié. Construis-le depuis la racine du dépôt :

```sh
packaging/build-deb.sh
sudo apt install ./packaging/.build/dist/fastipswitch_0.1.0_amd64.deb
```

`apt install ./fichier.deb` résout les `Depends` et installe les `Recommends` (dont `network-manager`) par défaut. Le paquet installe :

| Élément | Chemin |
|---|---|
| GUI | `/usr/bin/fastipswitch` |
| Helper | `/usr/libexec/fastipswitch/fastipswitch-helper` (`root:root`, `0755`) |
| Action polkit | `/usr/share/polkit-1/actions/org.talevinn.fastipswitch.policy` (`0644`) |
| Entrée de bureau | `/usr/share/applications/org.talevinn.fastipswitch.desktop` |
| Icônes | `/usr/share/icons/hicolor/<taille>x<taille>/apps/fastipswitch.png` (16 à 512) |
| Page de manuel | `/usr/share/man/man1/fastipswitch.1.gz` |
| AppStream | `/usr/share/metainfo/org.talevinn.fastipswitch.metainfo.xml` |

Le paquet n'active pas l'autostart. Le script de build valide la policy (chemin du helper, plus XML avec `xmllint` s'il est installé) et **exige** un `dpkg-deb` prenant en charge `--root-owner-group` ; il s'arrête sinon. Le `postinst` réapplique `root:root` et `0755` au helper. Les `Depends` d'exécution incluent à la fois Wayland et les bibliothèques clientes X11 (`libx11-6`, `libx11-xcb1`, `libxkbcommon-x11-0`, `libxcursor1`, `libxi6`, `libxrandr2`). Il n'y a **pas d'AppImage** : le helper et la policy polkit doivent être installés au niveau système, ce qu'un fichier portable unique ne fait pas. Le paquet n'est pas signé et aucun dépôt APT n'est fourni.

## Compiler depuis les sources

Rust stable (édition 2024, ≥ 1.88), `pkg-config` et les en-têtes Slint/winit :

```sh
sudo apt install build-essential pkg-config \
    libfontconfig1-dev libfreetype-dev libxkbcommon-dev \
    libwayland-dev libegl1-mesa-dev libgl1-mesa-dev
cargo build --release
```

- `libfreetype-dev` existe sur Debian 12+ et Ubuntu 24.04 ; utilise `libfreetype6-dev` sur les versions plus anciennes.
- Ajoute `libx11-xcb-dev` pour les builds/sessions X11.
- Sorties : `target/release/fastipswitch` et `target/release/fastipswitch-helper`.
- `cargo build --release` construit les deux binaires déclarés dans `Cargo.toml` ; le script `.deb` les construit explicitement avec `--bin`.

Pour l'usage normal, le helper doit être installé au niveau système. Le bouton **Installer** de l'application ne fait qu'une installation par utilisateur : il copie la GUI dans `~/.local/bin/fastipswitch`, écrit les icônes sous `~/.local/share/icons/hicolor/`, une entrée de bureau sous `~/.local/share/applications/`, et éventuellement `~/.config/autostart/fastipswitch.desktop`. Il n'installe jamais le helper ni la policy polkit (un helper modifiable par l'utilisateur casserait le modèle d'élévation). Si `/usr/bin/fastipswitch` existe (paquet système), il ne copie ni ne modifie `/usr/bin` et ne gère que l'entrée d'autostart. Les `%` des chemins `Exec=` sont échappés en `%%`, et les retours à la ligne remplacés par des espaces.

## Utilisation

1. **Choisir l'interface.** Le loopback et les liens de conteneurs/ponts (`docker*`, `veth*`, `virbr*`, `vmnet*`, `br-*`) sont masqués et refusés par le helper. Les liens VPN (`tun*`, `wg*`, …) sont conservés mais souvent signalés down, car `tun`/`wg` rapportent `operstate: UNKNOWN` ; une interface est up seulement si `operstate == UP` **et** le drapeau `UP` est présent.
2. **Adresse principale.** Saisir un CIDR, une passerelle et un DNS optionnels, puis **Appliquer**. La validation est la même qu'avant, avec un **préfixe minimum `/8`** : adresses réseau/broadcast (préfixes ≤ 30), préfixes `/0` à `/7`, passerelle hors sous-réseau et DNS non unicast sont refusés.
3. **IP virtuelles.** `+`, saisir un CIDR, activer l'interrupteur. Les alias sont ajoutés avec `ip -4 addr add` et retirés avec `ip -4 addr del`.
4. **DHCP.** Le retour réactive le profil NetworkManager enregistré quand l'interface est gérée. Dans les modes de repli, voir le tableau d'états ci-dessous.
5. **Quitter / fermer.** Retire les alias enregistrés dans `session.toml`, remet DHCP et réinitialise le DNS **seulement si le DNS a réellement été appliqué**. La fermeture de fenêtre quitte (elle ne masque pas) : `hides_on_close` est faux sous Linux car aucun tray n'est garanti. Si la restauration échoue, la fenêtre reste ouverte et affiche l'erreur.

Une seule instance tourne à la fois : un `flock` exclusif sur `$XDG_RUNTIME_DIR/fastipswitch-<uid>.lock`. Un second lancement s'arrête après un message sur stderr. Un nom de socket Unix est réservé pour une future demande d'activation « afficher la fenêtre », mais aucun serveur n'est démarré. Une session temporaire est liée à une interface : tant qu'elle est active, les opérations visant une autre interface sont refusées (termine par DHCP/Quitter ou sélectionne cette interface). La récupération conserve la session si l'interface est absente ou si la lecture des adaptateurs échoue.

### Ce qui est appliqué

Le helper classe d'abord l'interface avec `nmcli device status` :

| État | Adresse statique | Retour DHCP | Remarques |
|---|---|---|---|
| NM géré, exactement `connected` | `nmcli device modify` | `nmcli connection up <uuid>` | configuration active seulement |
| NM présent, tout autre état (`connected (externally)`, `unmanaged`, `disconnected`) | **refusée** | **refusé** | aucune commande destructive ; les alias passent par `ip`, les changements DNS par `resolvectl` |
| Aucun NetworkManager (`nmcli` absent ou statut en échec) | repli `ip` avec instantané/restauration | seulement avec `networkctl` ; refusé avant modification sinon | volatil, non persisté |

| Action | NM `connected` | NM présent, interface non gérée | Aucun NetworkManager |
|---|---|---|---|
| Lire les interfaces | `ip -j addr show`, `ip -j link show`, `ip -j route show default` | mêmes lectures `ip` | mêmes lectures `ip` |
| Lire connexion / DNS NM | `nmcli -t -f UUID,DEVICE connection show --active` ; `nmcli -g IP4.DNS device show <if>` | `connection` vide ; DNS lu vide (écriture via `resolvectl`) | aucune donnée NM |
| Adresse statique | `nmcli device modify <if> ipv4.method manual ipv4.addresses <cidr> ipv4.gateway <gw>` (passerelle vide pour l'effacer) | refusée | instantané, `ip -4 addr flush … scope global`, `ip -4 addr add`, route par défaut optionnelle ; restauration si échec |
| Passerelle | `ipv4.gateway` via nmcli | — | voir la règle de passerelle ci-dessous |
| DHCP | `nmcli connection up <uuid>` | refusé | `networkctl` obligatoire ; instantané, flush adresse + route par défaut, `networkctl reconfigure` ; restauration si échec |
| DNS statique | `nmcli device modify <if> ipv4.dns "<serveurs>" ipv4.ignore-auto-dns yes` | `resolvectl dns <if> <serveurs…>` ; erreur si absent | `resolvectl dns <if> <serveurs…>` ; erreur si absent |
| Réinitialiser le DNS | `nmcli device modify <if> ipv4.dns "" ipv4.ignore-auto-dns no` | `resolvectl revert <if>` ; sans effet sans lui | `resolvectl revert <if>` ; sans effet sans lui |
| Ajouter / retirer un alias | `ip -4 addr add/del <cidr> dev <if>` | idem | idem |
| Bascule de coexistence DHCP | sans objet (les alias coexistent nativement avec DHCP) | sans objet | sans objet |

**Sécurité du repli d'adresse.** Avant de vider, le helper capture les adresses IPv4 et les routes par défaut de l'interface. Si l'ajout de l'adresse ou de la route échoue, il réapplique les adresses et routes sauvegardées et renvoie une erreur. Le repli reste volatil : rien n'est persisté, et systemd-networkd peut l'écraser.

**Règle de passerelle en repli.** En repli `ip`, la passerelle n'est posée que si l'interface a déjà une route par défaut (métrique existante conservée) ou s'il n'existe aucune route par défaut (nouvelle route en métrique `1000`). Si une autre interface possède l'unique route par défaut, la passerelle est ignorée et le helper écrit un avertissement sur stderr ; l'adresse est quand même appliquée. La GUI n'affiche pas cet avertissement stderr.

**Sécurité du retour DHCP en repli.** Sans NetworkManager, le helper refuse le retour **avant** de toucher aux adresses si `networkctl` est absent. Sinon il sauvegarde adresses/routes, vide, lance `networkctl reconfigure`, et en cas d'échec réapplique les adresses et routes sauvegardées puis renvoie l'erreur. Un échec de reconfigure n'est jamais présenté comme un succès.

Points importants :

- `nmcli device modify` modifie la connexion **active** en mémoire ; le profil sur disque n'est pas persisté. C'est l'équivalent de l'intention `store=active` sous Windows.
- `nm_state` est une détection au mieux : si `nmcli device status` échoue lui-même (D-Bus indisponible, etc.), le helper considère NetworkManager comme absent et utilise le repli.
- Les alias passent toujours par `ip`, y compris sur les liens NM. NetworkManager peut les retirer quand il réapplique ou change le profil ; réactive-les.
- La lecture utilise `ip -j` ; un échec de `ip` est fatal pour la lecture, tandis qu'un échec de `nmcli` est toléré (`connection: None`, pas de DNS). Les commandes enfants tournent avec `LC_ALL=C` et `LANG=C`, donc l'analyse ne dépend pas de la locale.
- Le DNS est appliqué par une opération séparée après l'adresse. En cas d'échec, `dns_set` n'est **pas** armé : Quitter n'efface pas une configuration DNS existante.
- `dhcp` est déduit du drapeau `dynamic` de `ip addr`. Les adresses APIPA/link-local (`169.254.x.x`) sont ignorées pour choisir l'adresse principale, comme sous Windows.
- La passerelle affichée est la route par défaut IPv4 de plus faible métrique pour l'interface.

## Architecture

```
main.rs (UI Slint)
   │  Jobs / Events (mpsc)
   ▼
worker.rs ──► platform::backend() ──► platform::linux::LinuxBackend
   │                                      ├─ read.rs  : lectures ip -j + nmcli (non privilégié)
   │                                      └─ ops.rs   : construit helper::protocol::Request
   │                                                     │
   │                                                     ▼
   │                                          platform/linux/helper.rs
   │                                          pkexec /usr/libexec/.../fastipswitch-helper
   │                                                     │ JSON sur stdin
   │                                                     ▼
   │                                          src/bin/fastipswitch-helper.rs
   │                                          helper::validate  →  helper::exec
   │                                                     │ JSON sur stdout
   ▼                                                     ▼
mise à jour UI ◄────────────────────────────────────── Response { ok, message? }
```

- `src/platform/mod.rs` est la façade commune : types `AdapterInfo`, `IfaceId`, `IpEntry`, `AdapterRef`, trait `NetworkBackend` (implémenté par `platform::windows` et `platform::linux`), et fonctions système (élévation, instance, chemins, installation, thème).
- `src/platform/linux/read.rs` analyse la sortie `ip -j` avec `serde_json` ; les fonctions d'analyse pures sont testées avec les fixtures de `tests/fixtures/`, y compris les `:` échappés de `nmcli -t`.
- `src/platform/linux/ops.rs` ne touche jamais au réseau : il sérialise une requête et appelle le helper.
- `src/helper/protocol.rs` est le format figé (`VERSION = 1`, une opération par requête).
- `src/helper/validate.rs` revalide la requête dans le processus privilégié ; `src/helper/exec.rs` implémente les trois états NetworkManager, l'instantané/restauration du repli et la règle de route par défaut.
- Les commandes ne passent jamais par un shell : `Command::new(programme).args(...)`, avec `LC_ALL=C` et `LANG=C` pour les lectures.

### Protocole

Requête (stdin), un document JSON par processus :

```json
{"version":1,"op":"apply_static","ifname":"eth0","cidr":"192.168.3.10/24","gateway":"192.168.3.1","dns":[]}
```

Opérations : `apply_static`, `set_dhcp`, `set_dns`, `reset_dns`, `add_alias`, `remove_alias` (`op` en `snake_case` ; `cidr` est une chaîne `"a.b.c.d/préfixe"`). L'entrée sur stdin est plafonnée à **64 Kio** et doit être de l'UTF-8 valide ; un corps plus gros ou non UTF-8 est refusé par une erreur JSON avant la validation.

Réponse (stdout) :

```json
{"ok":true}
{"ok":false,"message":"…"}
```

Le helper sort avec le code 0 en cas de succès, 1 en cas d'erreur. Sans réponse JSON valide, la GUI associe le code de sortie `pkexec` 126 à « authentification annulée » et 127 à « authentification échouée » ; tout autre code non nul est signalé comme un échec du helper. Une requête n'est jamais groupée : chaque action de l'interface qui exige des privilèges démarre le helper une fois et réutilise donc l'autorisation polkit courante.

### Élévation et polkit

La policy installée est [`packaging/polkit/org.talevinn.fastipswitch.policy`](../packaging/polkit/org.talevinn.fastipswitch.policy) :

- action `org.talevinn.fastipswitch.network` ;
- `allow_any = no`, `allow_inactive = no`, `allow_active = auth_admin_keep` : seule la **session locale active** peut élever, et l'autorisation est conservée environ cinq minutes pour le processus appelant. L'élévation depuis SSH ou une session inactive est refusée ; il n'y a pas de demande de mot de passe dans ce cas.
- `org.freedesktop.policykit.exec.path` fixe le helper à `/usr/libexec/fastipswitch/fastipswitch-helper`.
- Le prompt est générique : polkit n'affiche ni l'interface ni l'IP modifiée.

Le helper revalide indépendamment de la GUI :

- version du protocole = 1 ;
- nom d'interface : non vide, ≤ 15 caractères (`IFNAMSIZ - 1`), premier caractère alphanumérique, caractères autorisés `[A-Za-z0-9_.:@-]`, différent de `lo`, `.` et `..`, pas de préfixe virtuel (`docker*`, `veth*`, `virbr*`, `vmnet*`, `br-*`), et existant dans `/sys/class/net` ;
- CIDR reconstruit via `Cidr::new` (préfixe entre `/8` et `/32`, pas unspecified/loopback/multicast/broadcast, pas réseau/broadcast pour les préfixes ≤ 30) ;
- passerelle unicast, différente de l'adresse locale/réseau/broadcast, et incluse dans le CIDR ;
- liste DNS non vide, au plus 4 serveurs IPv4 unicast.

En release, l'application n'accepte que le chemin système du helper. `FASTIPSWITCH_HELPER` et un binaire `fastipswitch-helper` voisin de la GUI ne sont pris en compte qu'en debug : un utilisateur non privilégié ne peut pas détourner l'appel `pkexec` d'une installation release. `pkexec` est pris dans `/usr/bin/pkexec` s'il existe, sinon dans le `PATH`.

## Fichiers et environnement

| Chemin | Rôle |
|---|---|
| `$XDG_CONFIG_HOME/fastipswitch/state.toml` (défaut `~/.config/...`) | interface choisie, liste d'alias, dernier IP/passerelle/DNS |
| `$XDG_STATE_HOME/fastipswitch/session.toml` (défaut `~/.local/state/...`) | trace de récupération des modifications temporaires |
| `$XDG_STATE_HOME/fastipswitch/fastipswitch.log` | journal des commandes ; rotation en `fastipswitch.log.1` au-delà de 1 Mo (un ancien fichier conservé) |
| `$XDG_RUNTIME_DIR/fastipswitch-<uid>.lock` | `flock` d'instance unique ; sans `XDG_RUNTIME_DIR` utilisable, `/tmp/fastipswitch-<uid>/` est créé (`0700`) |
| `$XDG_RUNTIME_DIR/fastipswitch-<uid>.pid` | PID du détenteur du verrou (`0600`) |
| `$XDG_RUNTIME_DIR/fastipswitch-<uid>.sock` | nom de socket réservé, non servi actuellement |
| `$XDG_DATA_HOME/applications/fastipswitch.desktop`, `$XDG_DATA_HOME/icons/hicolor/...` | installation par utilisateur depuis l'interface |
| `$XDG_CONFIG_HOME/autostart/fastipswitch.desktop` | autostart XDG, si activé |
| `data/` à côté de l'exécutable | mode portable si le dossier est accessible en écriture (prioritaire sur XDG) |
| `/usr/libexec/fastipswitch/fastipswitch-helper` | helper privilégié (paquet uniquement) |
| `/usr/share/polkit-1/actions/org.talevinn.fastipswitch.policy` | action polkit (paquet uniquement) |

Les valeurs XDG relatives sont ignorées (la spécification exige un chemin absolu), pour config/state/data et runtime. Les répertoires de configuration/état sont créés en `0700` ; `state.toml`, `session.toml` et le journal sont créés en `0600` (rotation en `fastipswitch.log.1` au-delà de 1 Mo).

Variables d'environnement :

| Variable | Effet |
|---|---|
| `FASTIPSWITCH_LANG=fr\|en` | force la langue (sinon `LC_ALL`, `LC_MESSAGES`, `LANG` ; français si la première valeur non vide commence par `fr`) |
| `FASTIPSWITCH_THEME=dark\|light` | force le thème (sinon détection GNOME/Cinnamon/XFCE/KDE, repli clair) |
| `FASTIPSWITCH_HELPER=/chemin` | chemin du helper, **builds debug uniquement** |
| `FASTIPSWITCH_DRY_RUN=1` | la GUI journalise la requête et renvoie un succès sans lancer le helper ; aucune modification réseau. **Builds debug uniquement** |

`--print-adapters` affiche une ligne par interface puis quitte, sans élévation :

```
nom  index=<ifindex>  mac=<mac>  up=<bool>  dhcp=<bool>  gw=<ipv4|vide>  dns=<ip,ip|vide>  conn=<uuid|vide>  <cidr[(dhcp)], …>
```

(les champs sont séparés par des tabulations ; `conn` est l'UUID NetworkManager actif).

## Développement

```sh
cargo build                 # debug : target/debug/{fastipswitch,fastipswitch-helper}
cargo test --all-targets
cargo fmt --check
```

En debug, le helper est cherché dans cet ordre : `FASTIPSWITCH_HELPER` s'il est défini et pointe un fichier, puis `/usr/libexec/fastipswitch/fastipswitch-helper`, puis un `fastipswitch-helper` voisin de la GUI en cours (c.-à-d. `target/debug/`). Après `cargo build`, lancer la GUI depuis `target/debug/` fonctionne donc sans rien installer.

Appel de diagnostic du helper (exécute réellement l'opération, root uniquement) :

```sh
printf '%s' '{"version":1,"op":"set_dhcp","ifname":"eth0"}' |
  sudo /usr/libexec/fastipswitch/fastipswitch-helper
```

Il affiche la réponse JSON. Utilise une opération de restauration (`set_dhcp`, `remove_alias`) pour tester ; en build debug, `FASTIPSWITCH_DRY_RUN` évite les modifications réelles dans la GUI.

Les tests ne demandent pas root et ne modifient pas le réseau de la machine : ils exercent les analyseurs `ip -j`/`nmcli` avec des fixtures, le round-trip du protocole, la construction des arguments, les règles de validation, la résolution des chemins XDG, l'écriture de l'installeur dans des répertoires temporaires et le chemin dry-run debug. Les tests d'intégration `tests/cli.rs` (la GUI se termine proprement en mode `--print-adapters` sans ouvrir de fenêtre) et `tests/helper_cli.rs` (les requêtes invalides/interdites sont refusées avec une seule réponse JSON et aucune commande réseau) tournent aussi sans root.

### Lancer les tests en local

Toutes les vérifications se font sur la machine de développement :

```sh
cargo fmt --check
cargo test --all-targets
cargo build --release
```

Le backend Windows ne peut pas être compilé sous Linux (il exige les API Windows) ; il se vérifie sur une machine Windows. Le `.deb` se construit séparément avec `packaging/build-deb.sh`.

### Construire le `.deb`

`packaging/build-deb.sh` :

1. lit la version dans `Cargo.toml` (`VERSION=` la surcharge) ;
2. lance `cargo build --release --bin fastipswitch --bin fastipswitch-helper` ;
3. valide la policy polkit : chemin du helper toujours, XML avec `xmllint` s'il est disponible ;
4. assemble l'arborescence sous `packaging/.build/stage/` (ignoré par git) et exige `dpkg-deb --root-owner-group` ;
5. écrit `packaging/.build/dist/fastipswitch_<version>_<arch>.deb`.

Variables utiles : `SKIP_BUILD=1`, `VERSION=`, `ARCH=` (défaut `dpkg --print-architecture`), `DIST_DIR=`, `CARGO_TARGET_DIR=`, `SKIP_VALIDATE=1`, `POLKIT_POLICY=`, `ALLOW_MISSING_POLKIT=1`. Le script refuse de construire sans la policy polkit sauf avec `ALLOW_MISSING_POLKIT=1` (paquet de test à l'élévation cassée), et s'arrête si `dpkg-deb` ne prend pas en charge `--root-owner-group`. `SKIP_VALIDATE=1` ne saute que `desktop-file-validate`/`appstreamcli`, pas les contrôles polkit. Voir [packaging/README.md](../packaging/README.md) pour le détail des dépendances et un exemple de staging.

## Limites et pistes

- **macOS n'est pas implémenté.** Linux est la seule plateforme ajoutée à ce jour.
- **Pas d'AppImage / Flatpak / Snap**, pas de dépôt signé : uniquement un `.deb` construit localement. Un format portable demanderait un chemin d'installation séparé pour le helper et la policy.
- **L'état NetworkManager est appliqué.** Si NM tourne mais que l'interface n'est pas exactement `connected` (`connected (externally)`, `unmanaged`, `disconnected`), l'adresse statique et le retour DHCP sont refusés pour protéger une configuration externe ; les alias fonctionnent toujours, les changements DNS passent par `resolvectl`. Si `nmcli device status` échoue lui-même, le helper considère NM comme absent.
- **Pas de backend systemd-networkd complet.** Sans NM, l'adressage statique est un instantané `ip` volatil (adresses/routes sauvegardées réappliquées en cas d'échec) ; le retour DHCP exige `networkctl` et est refusé avant modification sans lui. Un échec de `networkctl reconfigure` réapplique les adresses/routes sauvegardées et renvoie une erreur, jamais un faux succès.
- **La passerelle de repli est conditionnelle.** Elle n'est posée que si l'interface a déjà une route par défaut (métrique existante conservée) ou s'il n'en existe aucune (métrique `1000`) ; sinon elle est ignorée, avec un avertissement uniquement sur stderr du helper (non affiché dans la fenêtre).
- **Les alias ne sont pas persistés** par NetworkManager et peuvent disparaître quand le profil est réappliqué.
- **Pas de gestion des routes statiques** ni d'**IPv6** (IPv4 uniquement, comme sous Windows). Les préfixes plus courts que `/8` sont refusés ; les noms d'interfaces virtuelles sont refusés par le helper.
- **Tray** via StatusNotifierItem/AppIndicator ; GNOME demande l'extension AppIndicator. La fermeture de fenêtre quitte par conception.
- **Une instance et une interface active par session** : le socket d'activation est réservé mais non servi ; les opérations sur une autre interface sont refusées tant qu'une session temporaire est active. La récupération conserve la session si l'interface est absente ou si la lecture échoue.
- **Le nettoyage DNS est conditionnel** : `dns_set` n'est armé qu'après une application DNS réussie, donc un échec DNS à l'application n'efface pas la configuration DNS existante au Quitter.
- **Détection du thème** fondée sur des outils de bureau (`gsettings`, `xfconf-query`, `kreadconfig5/6`) avec repli clair.
- **Limites polkit** : `allow_any=no`, `allow_inactive=no`, `allow_active=auth_admin_keep`. L'élévation ne fonctionne que depuis la session locale active (aucun prompt SSH/inactive), l'autorisation est conservée environ cinq minutes pour le processus appelant, et le prompt est générique (sans interface/IP).
- **Matrice de validation matérielle/bureau non publiée** : le portage est testé par des tests unitaires et d'intégration exécutés en local, pas par une matrice automatisée de puces Wi-Fi/Ethernet et de bureaux.
- Quelques textes d'interface hérités mentionnent encore Windows ou les droits administrateur ; sous Linux l'écran de verrouillage n'est jamais affiché (`is_admin()` vaut vrai et l'élévation est par action), donc ils ne sont pas atteignables en usage normal.

## Dépannage

| Symptôme | Vérification |
|---|---|
| Aucune interface listée | `fastipswitch --print-adapters` ; `ip -j link show` ; les noms de conteneurs/ponts sont masqués et refusés par conception |
| Adresse non appliquée | `nmcli device status` : l'interface doit être exactement `connected` ; si elle est `unmanaged`/`connected (externally)`/`disconnected`, l'adresse statique est refusée ; sinon journal et `ip -4 addr show dev <if>` |
| Retour DHCP refusé | Même contrôle de l'état NM ; sans NM, `command -v networkctl` doit réussir ; si NM ne gère pas l'interface, utiliser l'outil externe (networkd/ifupdown) |
| DNS non appliqué | NM : `nmcli device show <if> \| grep IP4.DNS` ; sinon : `resolvectl status <if>` ; `resolvectl` doit exister |
| Passerelle non appliquée | Repli uniquement : `ip route show default` ; si une autre interface possède l'unique route par défaut, la passerelle est ignorée par conception (avertissement stderr du helper) |
| Aucune demande de mot de passe / « non autorisé » | Policy présente dans `/usr/share/polkit-1/actions/` ; `pkexec` installé ; agent polkit actif ; **session locale active** (SSH/inactive refusé par la policy) ; `journalctl -b` |
| Helper introuvable | `ls -l /usr/libexec/fastipswitch/fastipswitch-helper` ; en debug, définis `FASTIPSWITCH_HELPER` |
| Les changements disparaissent | NM a réappliqué le profil : les alias sont volatils ; les changements hors NM ne sont pas persistés |
| « Une configuration temporaire est active sur … » | Termine-la par DHCP/Quitter ou sélectionne cette interface |
| Un second lancement ne fait rien | verrou d'instance unique ; message sur stderr ; vérifie `$XDG_RUNTIME_DIR/fastipswitch-<uid>.lock` |

Restauration manuelle :

```sh
# NetworkManager : réactive le profil enregistré
nmcli connection up <uuid>
# sans NetworkManager (seulement si systemd-networkd gère l'interface)
sudo ip -4 addr flush dev <if> scope global
sudo ip route flush default dev <if>
sudo networkctl reconfigure <if>
# DNS
nmcli device modify <if> ipv4.dns "" ipv4.ignore-auto-dns no
sudo resolvectl revert <if>
# un alias
sudo ip -4 addr del <cidr> dev <if>
```

Si `networkctl reconfigure` échoue, réapplique les adresses/routes précédentes depuis le journal ou le profil de la distribution ; le helper les restaure automatiquement quand il a fait l'opération, mais une séquence manuelle ne le fait pas.

Désinstalle le paquet avec `sudo apt remove fastipswitch` (ou `apt purge`). L'état utilisateur sous `~/.config/fastipswitch` et `~/.local/state/fastipswitch` n'est jamais supprimé par le paquet.

## Voir aussi

- [README — section Linux](../README.fr.md#linux)
- [État multiplateforme](cross-platform.fr.md)
- [Packaging (.deb)](../packaging/README.md)
- [Source de la page de manuel](../packaging/man/fastipswitch.1)
