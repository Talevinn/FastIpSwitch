[English](README.md) | **Français**

# FastIpSwitch — V0.1

**Mainteneur :** Valentin PICARD ([@Talevinn](https://codeberg.org/Talevinn))

## Téléchargement

**[Télécharger FastIpSwitch V0.1 pour Windows](https://codeberg.org/Talevinn/FastIpSwitch/releases/download/V0.1/FastIpSwitch-V0.1.exe)** — Windows 10/11, x64, à lancer en administrateur. [Notes de version et SHA-256](https://codeberg.org/Talevinn/FastIpSwitch/releases/tag/V0.1).

Utilitaire Windows (Rust + [Slint](https://slint.dev)) pour **changer rapidement l'adresse IPv4** d'un PC et **ajouter ou retirer à la volée des IP virtuelles**, afin de configurer ou d'interroger du matériel situé sur un autre plan d'adressage : équipements neufs en IP par défaut, anciens équipements en IP fixe, baies de test, matériel AV, etc.

Toutes les modifications réseau passent par **`netsh interface ipv4`**. L'interface suit le thème clair/sombre de Windows et la langue du système (français/anglais).

| Thème sombre | Thème clair | Sans élévation |
|---|---|---|
| ![thème sombre](docs/screenshot-dark.png) | ![thème clair](docs/screenshot-light.png) | ![interface verrouillée](docs/screenshot-locked.png) |

## Pourquoi FastIpSwitch

- Un équipement est en `192.168.0.10/24` et ton PC en `192.168.1.45/24` : ajoute une IP virtuelle `192.168.0.50/24` en un clic, sans perdre ta connexion.
- Tu dois basculer ton PC sur un réseau de labo en IP fixe : saisis `192.168.2.10/24`, la passerelle et le DNS, puis **Appliquer**.
- Tu veux revenir immédiatement à la configuration automatique : l'interrupteur **DHCP** ou **Quitter** remet les valeurs par défaut.
- Tu enchaînes les interventions : la liste d'IP virtuelles est mémorisée (désactivée au démarrage) et les adresses temporaires ne restent pas sur l'interface.

## Fonctionnalités

- Sélection de l'adaptateur réseau (les pilotes de filtrage Npcap/QoS/WFP sont masqués).
- Saisie **CIDR** : `192.168.2.10/24` (pas de `255.255.255.0` à taper).
- Adresse principale : DHCP ↔ statique, passerelle et **DNS** optionnels (plusieurs serveurs séparés par des virgules).
- Liste d'IP virtuelles avec interrupteur par ligne, `+` pour ajouter, `×` pour supprimer.
- Interrupteur **DHCP** bidirectionnel : ON = DHCP, OFF = passage en statique avec les valeurs affichées.
- Icône dans la zone de notification : clic pour rouvrir, menu **Afficher / Revenir en DHCP / Quitter**.
- Interface **français/anglais** selon Windows (surcharge possible), thème clair/sombre avec variante « rack ».
- Infobulles sur les icônes et les interrupteurs.
- Journal des commandes netsh et récupération après un arrêt brutal.

## Pourquoi les droits administrateur sont nécessaires

Windows n'expose pas d'API utilisateur pour modifier l'adressage IP d'une interface : `netsh interface ipv4` (comme `New-NetIPAddress`) exige un jeton élevé. Sans élévation, les commandes échouent et rien ne change.

FastIpSwitch est donc conçu pour être **lancé élevé**, mais il ne force pas l'UAC au démarrage :

- le manifeste est en `asInvoker` ; sans élévation, l'application s'ouvre quand même, **entièrement grisée**, avec un panneau « Droits administrateur requis » et un bouton **Relancer en administrateur** ;
- les raccourcis créés par l'installateur passent `--elevate` pour demander l'UAC au lancement ;
- le démarrage automatique utilise une **tâche planifiée** `ONLOGON` avec `/RL HIGHEST`, donc élevée au logon, sans prompt UAC.

L'adressage (IP, masque, passerelle, alias) est appliqué en **`store=active`** : la configuration persistante n'est pas modifiée, et un redémarrage rend la configuration enregistrée dans Windows (généralement DHCP). Le **DNS** est l'exception : `netsh set dnsservers` n'accepte pas `store=active`, il est donc écrit de façon persistante puis restauré lors d'un Quitter propre.

## Fonctionnement

- **IP virtuelles** : `netsh add address ... skipassource=true store=active`. `skipassource` empêche Windows de choisir ces adresses comme source du trafic sortant, tout en répondant en ARP/ping/HTTP.
- **Coexistence DHCP** : Windows désactive DHCP quand on ajoute une IP manuelle. FastIpSwitch active `dhcpstaticipcoexistence` avant l'ajout pour conserver la connexion DHCP, puis vérifie que DHCP est toujours actif ; sinon l'opération est annulée et DHCP restauré.
- **Adresse principale fixe** : `set address source=static ... store=active`, puis les alias actifs sont réajoutés (cette commande efface les adresses manuelles).
- **DNS** : `set dnsservers source=static` puis `add dnsservers` pour les suivants ; retour à `source=dhcp` lors d'un retour DHCP. Ces commandes n'ont pas de `store=active` : le DNS statique est **persistant**. Il est normalement restauré à la fermeture, mais après un crash suivi d'un reboot sans relancer l'application, il peut rester statique : relance FastIpSwitch ou remets-le manuellement.
- **Rafraîchissement et APIPA** : la sélection d'un adaptateur relit l'état réel. Les adresses `169.254.x.x` (APIPA/link-local) sont ignorées : elles signifient « pas de bail DHCP ». Le champ IP reste vide avec un message, et une vérification toutes les 2 s le remplit quand le bail arrive ; si l'adaptateur courant tombe, l'app bascule vers un adaptateur nouvellement exploitable (sauf si tu as explicitement choisi un adaptateur déconnecté pour le configurer).
- **Validation** : `.0` et `.255` refusés pour un /24 (réseau/diffusion), passerelle obligatoirement dans le même sous-réseau, doublons refusés, DNS IPv4 valides.

## Redémarrage, fermeture et récupération

| Événement | Ce qui se passe |
|---|---|
| **Croix de la fenêtre** | En mode élevé, réduit dans la zone de notification ; la configuration reste active tant que l'app tourne. Sans admin, la croix ferme l'application. |
| **Quitter** (bouton ou tray) | Supprime les alias, repasse en DHCP, remet le DNS en automatique, puis ferme. Si la restauration échoue, l'app reste ouverte et affiche l'erreur. |
| **Redémarrage Windows** | Retour à la configuration persistante (généralement DHCP), même après un crash. Le DNS statique peut subsister si l'app n'a pas été relancée. |
| **Crash / kill** | `session.toml` est conservé ; au prochain lancement **élevé**, nettoyage ciblé + DHCP. |
| **Lancement sans admin** | Interface grisée + **Relancer en administrateur** ; aucune commande réseau, pas de récupération de session. |
| **Échec d'une commande netsh** | Message d'erreur, rollback de l'ajout, journal dans `data/fastipswitch.log` (ou `%LOCALAPPDATA%`). |

### En cas de problème

1. **L'interface est grisée** : clique sur **Relancer en administrateur** (ou clic droit sur l'exe → *Exécuter en tant qu'administrateur*).
2. **Une IP virtuelle ne part pas** : rouvre l'app et désactive l'interrupteur ; sinon redémarre le PC, la configuration persistante revient.
3. **Plus de réseau après une manipulation** : redémarre, ou exécute dans un terminal admin :
   ```cmd
   netsh interface ipv4 set address name="Ethernet 6" source=dhcp store=active
   netsh interface ipv4 set dnsservers name="Ethernet 6" source=dhcp
   netsh interface ipv4 set interface interface="Ethernet 6" dhcpstaticipcoexistence=disabled store=active
   ```
   (remplace `Ethernet 6` par le nom de ton interface ; `netsh interface ipv4 show interfaces` les liste. `set dnsservers` est persistant par nature ; les deux autres commandes utilisent `store=active` pour ne pas figer l'adressage.)
4. **Comprendre ce qui s'est passé** : le journal se trouve dans `data/fastipswitch.log` à côté de l'exe, ou `%LOCALAPPDATA%\FastIpSwitch\fastipswitch.log`. Chaque commande netsh et son résultat y sont écrits (avec rotation).
5. **Réinstaller** : lance la release élevée, clique sur **installé** en haut à droite → **RÉINSTALLER**. Les réglages installés et le choix d'autostart sont préservés.

## Installation

### Mode portable (par défaut)

L'exe est autonome (CRT statique, aucune dépendance VC++). Au lancement, s'il peut créer un dossier `data` à côté de l'exécutable, il y stocke `state.toml`, `session.toml` et le journal. Sinon, il utilise `%APPDATA%\FastIpSwitch` et `%LOCALAPPDATA%\FastIpSwitch`.

### Installateur intégré

Lance l'application élevée, puis clique sur l'icône d'installation en haut à droite (ou sur **installé** si déjà installée) :

- copie dans `%LOCALAPPDATA%\Programs\FastIpSwitch` ;
- crée les raccourcis **Bureau** et **Menu Démarrer** avec `--elevate` ;
- option **Lancer au démarrage** (décochée par défaut) → tâche planifiée `FastIpSwitch-<user>`, `ONLOGON`, `/RL HIGHEST` ;
- conserve la liste d'IP virtuelles et le réglage d'autostart lors d'une réinstallation.

Raccourci de développement :

```powershell
pwsh -File tools\install-shortcut.ps1
```

## Utilisation

1. Choisir l'adaptateur.
2. **Adresse principale** : saisir `192.168.2.10/24`, la passerelle et le DNS si besoin, puis **Appliquer**.
3. **IP virtuelles** : `+`, saisir `192.168.3.10/24`, activer l'interrupteur. L'IP est ajoutée sans couper DHCP.
4. L'interrupteur **DHCP** : ON = revenir en DHCP, OFF = figer l'adresse affichée en statique.
5. **Quitter** pour tout remettre à zéro.

`192.168.1.0/24` est refusé : `.0` est l'adresse réseau ; il faut une adresse hôte (`.1` à `.254`).

## Langue et thème

- Langue : français si la langue d'interface de Windows est le français, anglais sinon. Surcharge : `FASTIPSWITCH_LANG=fr` ou `en`.
- Thème : suit Windows (rack sombre / aluminium clair). Surcharge : `FASTIPSWITCH_THEME=dark` ou `light`.

## Compilation

Prérequis : Rust stable récent (≥ 1.88), Visual Studio Build Tools (MSVC), Windows 10/11.

```powershell
cargo build --release
```

Binaire : `target\release\fastipswitch.exe`. `.cargo/config.toml` active `+crt-static` pour un exe autonome. Le manifeste est en `asInvoker` (DPI PerMonitorV2).

## Fichiers

| Chemin | Rôle |
|---|---|
| `data\state.toml` / `%APPDATA%\FastIpSwitch\state.toml` | adaptateur, liste d'IP virtuelles, dernier IP/passerelle/DNS |
| `data\session.toml` / `%LOCALAPPDATA%\FastIpSwitch\session.toml` | trace de récupération après crash |
| `data\fastipswitch.log` / `%LOCALAPPDATA%\FastIpSwitch\fastipswitch.log` | journal des commandes netsh (rotation) |
| `%LOCALAPPDATA%\Programs\FastIpSwitch` | installation par utilisateur |

## Limites connues

- Pas de gestion des routes statiques (les équipements doivent être sur le même segment L2).
- Le DNS est remis en automatique lors d'un retour DHCP ; la valeur statique précédente n'est pas mémorisée.
- Une seule instance à la fois.
- Installation par utilisateur uniquement (pas d'installation machine-wide).

## Licence

GPL-3.0-or-later — voir `LICENSE` et `NOTICE`.
