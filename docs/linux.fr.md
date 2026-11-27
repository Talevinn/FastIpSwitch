[English](linux.md) | **Français**

# FastIpSwitch sous Linux

État : **premier portage Linux**. La même interface Slint pilote un backend Linux fondé sur NetworkManager (`nmcli`) avec un repli `ip`/`resolvectl`. Le comportement Windows est inchangé. Le portage est couvert par des tests unitaires et un build CI ; aucune matrice de validation matérielle/bureau n'est publiée à ce jour.

- Résumé utilisateur : [README — Linux](../README.fr.md#linux)
- Vue multiplateforme : [cross-platform.fr.md](cross-platform.fr.md)
- Détails du paquet : [packaging/README.md](../packaging/README.md)

## Prérequis

| Composant | Rôle | Remarques |
|---|---|---|
| `iproute2` (`ip`) | Lecture, alias, adressage de repli | Indispensable ; la lecture échoue sans lui |
| NetworkManager (`nmcli`) | Adresse principale, DHCP, DNS | Recommandé ; défaut Debian/Ubuntu/Mint |
| `pkexec` + polkit | Élévation du helper | Paquet `pkexec` sur Debian 12+/Ubuntu 24.04 |
| `resolvectl` (systemd-resolved) | DNS sur les interfaces non NM | Optionnel ; sans lui, pas de changement DNS |
| `networkctl` (systemd-networkd) | Reconfiguration après un retour DHCP de repli | Optionnel |
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

Le paquet n'active pas l'autostart. Il n'y a **pas d'AppImage** : le helper et la policy polkit doivent être installés au niveau système, ce qu'un fichier portable unique ne fait pas. Le paquet n'est pas signé et aucun dépôt APT n'est fourni.

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

Pour l'usage normal, le helper doit être installé au niveau système. Le bouton **Installer** de l'application ne fait qu'une installation par utilisateur : il copie la GUI dans `~/.local/bin/fastipswitch`, écrit les icônes sous `~/.local/share/icons/hicolor/`, une entrée de bureau sous `~/.local/share/applications/`, et éventuellement `~/.config/autostart/fastipswitch.desktop`. Il n'installe jamais le helper ni la policy polkit (un helper modifiable par l'utilisateur casserait le modèle d'élévation). Si `/usr/bin/fastipswitch` existe, le bouton ne gère que l'entrée d'autostart.

## Utilisation

1. **Choisir l'interface.** Le loopback et les liens de conteneurs/ponts (`docker*`, `veth*`, `virbr*`, `vmnet*`, `br-*`) sont masqués. Les liens VPN (`tun*`, `wg*`, …) sont conservés mais souvent signalés down, car `tun`/`wg` rapportent `operstate: UNKNOWN` ; une interface est up seulement si `operstate == UP` **et** le drapeau `UP` est présent.
2. **Adresse principale.** Saisir un CIDR, une passerelle et un DNS optionnels, puis **Appliquer**. La validation est la même que sous Windows : adresses réseau/broadcast refusées (préfixes ≤ 30), passerelle dans le même sous-réseau, DNS IPv4 unicast.
3. **IP virtuelles.** `+`, saisir un CIDR, activer l'interrupteur. Les alias sont ajoutés avec `ip -4 addr add` et retirés avec `ip -4 addr del`.
4. **DHCP.** Le retour réactive le profil NetworkManager enregistré, ou vide la configuration de repli.
5. **Quitter / fermer.** Retire les alias enregistrés dans `session.toml`, remet DHCP et réinitialise le DNS. La fermeture de fenêtre quitte (elle ne masque pas) : `hides_on_close` est faux sous Linux car aucun tray n'est garanti. Si la restauration échoue, la fenêtre reste ouverte et affiche l'erreur.

Une seule instance tourne à la fois : un `flock` exclusif sur `$XDG_RUNTIME_DIR/fastipswitch-<uid>.lock`. Un second lancement s'arrête après un message sur stderr. Un nom de socket Unix est réservé pour une future demande d'activation « afficher la fenêtre », mais aucun serveur n'est démarré.

### Ce qui est appliqué

| Action | NetworkManager (`nmcli device status` indique `connected`) | Repli (non géré par NM) |
|---|---|---|
| Lire les interfaces | `ip -j addr show`, `ip -j link show`, `ip -j route show default` | mêmes lectures `ip` |
| Lire connexion / DNS NM | `nmcli -t -f UUID,DEVICE connection show --active` ; `nmcli -g IP4.DNS device show <if>` | `connection` et `dns` restent vides |
| Adresse statique | `nmcli device modify <if> ipv4.method manual ipv4.addresses <cidr> ipv4.gateway <gw>` (passerelle vide pour l'effacer) | `ip -4 addr flush dev <if> scope global`, `ip -4 addr add <cidr> dev <if>`, puis `ip route replace default via <gw> dev <if>` si passerelle |
| DHCP | `nmcli connection up <uuid>` (réactive le profil, efface les surcharges transitoires) | `ip -4 addr flush … scope global`, `ip route flush default dev <if>`, puis `networkctl reconfigure <if>` s'il est présent |
| DNS statique | `nmcli device modify <if> ipv4.dns "<serveurs>" ipv4.ignore-auto-dns yes` | `resolvectl dns <if> <serveurs…>` ; erreur si `resolvectl` est absent |
| Réinitialiser le DNS | `nmcli device modify <if> ipv4.dns "" ipv4.ignore-auto-dns no` | `resolvectl revert <if>` ; sans effet silencieux sans `resolvectl` |
| Ajouter / retirer un alias | `ip -4 addr add/del <cidr> dev <if>` (toujours) | idem |
| Bascule de coexistence DHCP | sans objet (les alias coexistent nativement avec DHCP) | sans objet |

Points importants :

- `nmcli device modify` modifie la connexion **active** en mémoire ; le profil sur disque n'est pas persisté. C'est l'équivalent de l'intention `store=active` sous Windows.
- Les alias passent toujours par `ip`, y compris sur les liens NM. NetworkManager peut les retirer quand il réapplique ou change le profil ; réactive-les.
- La lecture utilise `ip -j` ; un échec de `ip` est fatal pour la lecture, tandis qu'un échec de `nmcli` est toléré (`connection: None`, pas de DNS).
- Le DNS est appliqué par une opération séparée après l'adresse : un échec DNS n'annule pas le changement d'adresse.
- `dhcp` est déduit du drapeau `dynamic` de `ip addr`. Les adresses APIPA/link-local (`169.254.x.x`) sont ignorées pour choisir l'adresse principale, comme sous Windows.
- La passerelle est la route par défaut IPv4 de plus faible métrique pour l'interface.

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
- `src/platform/linux/read.rs` analyse la sortie `ip -j` avec `serde_json` ; les fonctions d'analyse pures sont testées avec les fixtures de `tests/fixtures/`.
- `src/platform/linux/ops.rs` ne touche jamais au réseau : il sérialise une requête et appelle le helper.
- `src/helper/protocol.rs` est le format figé (`VERSION = 1`, une opération par requête).
- `src/helper/validate.rs` revalide la requête dans le processus privilégié ; `src/helper/exec.rs` construit les vecteurs d'arguments `nmcli`/`ip`/`resolvectl`.
- Les commandes ne passent jamais par un shell : `Command::new(programme).args(...)`.

### Protocole

Requête (stdin), un document JSON par processus :

```json
{"version":1,"op":"apply_static","ifname":"eth0","cidr":"192.168.3.10/24","gateway":"192.168.3.1","dns":[]}
```

Opérations : `apply_static`, `set_dhcp`, `set_dns`, `reset_dns`, `add_alias`, `remove_alias` (`op` en `snake_case` ; `cidr` est une chaîne `"a.b.c.d/préfixe"`).

Réponse (stdout) :

```json
{"ok":true}
{"ok":false,"message":"…"}
```

Le helper sort avec le code 0 en cas de succès, 1 en cas d'erreur. Sans réponse JSON valide, la GUI associe le code de sortie `pkexec` 126 à « authentification annulée » et 127 à « authentification échouée » ; tout autre code non nul est signalé comme un échec du helper. Une requête n'est jamais groupée : chaque action de l'interface qui exige des privilèges démarre le helper une fois et réutilise donc l'autorisation polkit courante.

### Élévation et polkit

La policy installée est [`packaging/polkit/org.talevinn.fastipswitch.policy`](../packaging/polkit/org.talevinn.fastipswitch.policy) :

- action `org.talevinn.fastipswitch.network` ;
- `allow_any = auth_admin`, `allow_inactive = auth_admin`, `allow_active = auth_admin_keep` : la session active s'authentifie une fois et l'autorisation est conservée environ cinq minutes ;
- `org.freedesktop.policykit.exec.path` fixe le helper à `/usr/libexec/fastipswitch/fastipswitch-helper`.

Le helper revalide indépendamment de la GUI :

- version du protocole = 1 ;
- nom d'interface : non vide, ≤ 15 caractères (`IFNAMSIZ - 1`), premier caractère alphanumérique, caractères autorisés `[A-Za-z0-9_.:@-]`, différent de `lo`, `.` et `..`, et existant dans `/sys/class/net` ;
- CIDR reconstruit via `Cidr::new` (préfixe ≤ 32, pas unspecified/loopback/multicast/broadcast, pas réseau/broadcast pour les préfixes ≤ 30) ;
- passerelle unicast, différente de l'adresse locale/réseau/broadcast, et incluse dans le CIDR ;
- liste DNS non vide, au plus 4 serveurs IPv4 unicast.

En release, l'application n'accepte que le chemin système du helper. `FASTIPSWITCH_HELPER` et un binaire `fastipswitch-helper` voisin de la GUI ne sont pris en compte qu'en debug : un utilisateur non privilégié ne peut pas détourner l'appel `pkexec` d'une installation release.

## Fichiers et environnement

| Chemin | Rôle |
|---|---|
| `$XDG_CONFIG_HOME/fastipswitch/state.toml` (défaut `~/.config/...`) | interface choisie, liste d'alias, dernier IP/passerelle/DNS |
| `$XDG_STATE_HOME/fastipswitch/session.toml` (défaut `~/.local/state/...`) | trace de récupération des modifications temporaires |
| `$XDG_STATE_HOME/fastipswitch/fastipswitch.log` | journal des commandes ; rotation en `fastipswitch.log.1` au-delà de 1 Mo (un ancien fichier conservé) |
| `$XDG_RUNTIME_DIR/fastipswitch-<uid>.lock` | `flock` d'instance unique (défaut `/tmp` si la variable est absente) |
| `$XDG_RUNTIME_DIR/fastipswitch-<uid>.pid` | PID du détenteur du verrou (`0600`) |
| `$XDG_RUNTIME_DIR/fastipswitch-<uid>.sock` | nom de socket réservé, non servi actuellement |
| `$XDG_DATA_HOME/applications/fastipswitch.desktop`, `$XDG_DATA_HOME/icons/hicolor/...` | installation par utilisateur depuis l'interface |
| `$XDG_CONFIG_HOME/autostart/fastipswitch.desktop` | autostart XDG, si activé |
| `data/` à côté de l'exécutable | mode portable si le dossier est accessible en écriture (prioritaire sur XDG) |
| `/usr/libexec/fastipswitch/fastipswitch-helper` | helper privilégié (paquet uniquement) |
| `/usr/share/polkit-1/actions/org.talevinn.fastipswitch.policy` | action polkit (paquet uniquement) |

Variables d'environnement :

| Variable | Effet |
|---|---|
| `FASTIPSWITCH_LANG=fr\|en` | force la langue (sinon `LC_ALL`, `LC_MESSAGES`, `LANG` ; français si la première valeur non vide commence par `fr`) |
| `FASTIPSWITCH_THEME=dark\|light` | force le thème (sinon détection GNOME/Cinnamon/XFCE/KDE, repli clair) |
| `FASTIPSWITCH_HELPER=/chemin` | chemin du helper, **builds debug uniquement** |
| `FASTIPSWITCH_DRY_RUN=1` | la GUI journalise la requête et renvoie un succès sans lancer le helper ; aucune modification réseau |

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

Il affiche la réponse JSON. Utilise une opération de restauration (`set_dhcp`, `remove_alias`) pour tester ; `FASTIPSWITCH_DRY_RUN` évite les modifications réelles dans la GUI.

Les tests ne demandent pas root et ne modifient pas le réseau de la machine : ils exercent les analyseurs `ip -j`/`nmcli` avec des fixtures, le round-trip du protocole, la construction des arguments, les règles de validation, la résolution des chemins XDG, l'écriture de l'installeur dans des répertoires temporaires, et le chemin dry-run (`FASTIPSWITCH_DRY_RUN` renvoie avant de résoudre ou lancer le helper).

### CI

Woodpecker (`.woodpecker.yml`) s'exécute sur push et pull request avec `rust:1.99-bookworm` :

1. installation de `pkg-config libfontconfig1-dev libfreetype-dev libxkbcommon-dev libwayland-dev libegl1-mesa-dev libgl1-mesa-dev` ;
2. `cargo fmt --check` ;
3. `cargo test --all-targets` ;
4. `cargo build --release`.

Le backend Windows n'y est pas compilé (il exige les API Windows) ; il n'y a pas de job macOS. Le build du `.deb` n'est pas lancé en CI.

### Construire le `.deb`

`packaging/build-deb.sh` :

1. lit la version dans `Cargo.toml` (`VERSION=` la surcharge) ;
2. lance `cargo build --release --bin fastipswitch --bin fastipswitch-helper` ;
3. assemble l'arborescence sous `packaging/.build/stage/` (ignoré par git) ;
4. écrit `packaging/.build/dist/fastipswitch_<version>_<arch>.deb`.

Variables utiles : `SKIP_BUILD=1`, `VERSION=`, `ARCH=` (défaut `dpkg --print-architecture`), `DIST_DIR=`, `CARGO_TARGET_DIR=`, `SKIP_VALIDATE=1`, `POLKIT_POLICY=`, `ALLOW_MISSING_POLKIT=1`. Le script refuse de construire sans la policy polkit sauf avec `ALLOW_MISSING_POLKIT=1` (paquet de test à l'élévation cassée). Voir [packaging/README.md](../packaging/README.md) pour le détail des dépendances et un exemple de staging.

## Limites et pistes

- **macOS n'est pas implémenté.** Linux est la seule plateforme ajoutée à ce jour.
- **Pas d'AppImage / Flatpak / Snap**, pas de dépôt signé : uniquement un `.deb` construit localement. Un format portable demanderait un chemin d'installation séparé pour le helper et la policy.
- **NetworkManager est le chemin nominal.** Sans lui, l'adresse principale et la passerelle passent par des commandes `ip` volatiles et le DNS exige `resolvectl` ; il n'y a pas de backend systemd-networkd complet ni de persistance de profil.
- **Les alias ne sont pas persistés** par NetworkManager et peuvent disparaître quand le profil est réappliqué.
- **Pas de gestion des routes statiques** ni d'**IPv6** (IPv4 uniquement, comme sous Windows).
- **Tray** via StatusNotifierItem/AppIndicator ; GNOME demande l'extension AppIndicator. La fermeture de fenêtre quitte par conception.
- **Une seule instance** ; le socket d'activation est réservé mais non servi.
- **Détection du thème** fondée sur des outils de bureau (`gsettings`, `xfconf-query`, `kreadconfig5/6`) avec repli clair.
- **Matrice de validation matérielle/bureau non publiée** : le portage est testé par des tests unitaires et la compilation CI, pas par une matrice automatisée de puces Wi-Fi/Ethernet et de bureaux.
- Quelques textes d'interface hérités mentionnent encore Windows ou les droits administrateur ; sous Linux l'écran de verrouillage n'est jamais affiché (`is_admin()` vaut vrai et l'élévation est par action), donc ils ne sont pas atteignables en usage normal.

## Dépannage

| Symptôme | Vérification |
|---|---|
| Aucune interface listée | `fastipswitch --print-adapters` ; `ip -j link show` ; les noms de conteneurs/ponts sont masqués par conception |
| Adresse non appliquée | `nmcli device status` (interface `connected` ?), journal, puis `ip -4 addr show dev <if>` |
| DNS non appliqué | NM : `nmcli device show <if> \| grep IP4.DNS` ; repli : `resolvectl status <if>` ; `resolvectl` doit exister |
| Aucune demande de mot de passe / « non autorisé » | policy présente dans `/usr/share/polkit-1/actions/` ; `pkexec` installé ; agent polkit actif ; `journalctl -b` |
| Helper introuvable | `ls -l /usr/libexec/fastipswitch/fastipswitch-helper` ; en debug, définis `FASTIPSWITCH_HELPER` |
| Les changements disparaissent | NM a réappliqué le profil : les alias sont volatils ; les changements hors NM ne sont pas persistés |
| Un second lancement ne fait rien | verrou d'instance unique ; message sur stderr ; vérifie `$XDG_RUNTIME_DIR/fastipswitch-<uid>.lock` |

Restauration manuelle :

```sh
# NetworkManager : réactive le profil enregistré
nmcli connection up <uuid>
# sans NetworkManager
sudo ip -4 addr flush dev <if> scope global
sudo ip route flush default dev <if>
sudo networkctl reconfigure <if>
# DNS
nmcli device modify <if> ipv4.dns "" ipv4.ignore-auto-dns no
sudo resolvectl revert <if>
# un alias
sudo ip -4 addr del <cidr> dev <if>
```

Désinstalle le paquet avec `sudo apt remove fastipswitch` (ou `apt purge`). L'état utilisateur sous `~/.config/fastipswitch` et `~/.local/state/fastipswitch` n'est jamais supprimé par le paquet.

## Voir aussi

- [README — section Linux](../README.fr.md#linux)
- [État multiplateforme](cross-platform.fr.md)
- [Packaging (.deb)](../packaging/README.md)
- [Source de la page de manuel](../packaging/man/fastipswitch.1)
