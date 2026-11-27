// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Installation par utilisateur : binaire dans `~/.local/bin`, icônes et
//! entrée de bureau XDG, autostart GNOME/XDG.
//!
//! Le helper privilégié (`fastipswitch-helper`) n'est volontairement **pas**
//! copié ici : il doit rester dans un répertoire système (`/usr/libexec/`),
//! installé par le paquet `.deb` avec sa règle polkit. Le dupliquer dans un
//! répertoire utilisateur ouvrirait une voie d'élévation non maîtrisée.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use super::paths;

// Les `static` permettent l'élagage `&[u8; N]` -> `&[u8]` en contexte constant.
static ICON_16: &[u8] = include_bytes!("../../../assets/logo/ip-swap-16.png");
static ICON_24: &[u8] = include_bytes!("../../../assets/logo/ip-swap-24.png");
static ICON_32: &[u8] = include_bytes!("../../../assets/logo/ip-swap-32.png");
static ICON_48: &[u8] = include_bytes!("../../../assets/logo/ip-swap-48.png");
static ICON_64: &[u8] = include_bytes!("../../../assets/logo/ip-swap-64.png");
static ICON_128: &[u8] = include_bytes!("../../../assets/logo/ip-swap-128.png");
static ICON_256: &[u8] = include_bytes!("../../../assets/logo/ip-swap-256.png");
static ICON_512: &[u8] = include_bytes!("../../../assets/logo/ip-swap-512.png");

/// Icônes PNG embarquées, indexées par taille (carrée).
const ICONS: &[(u32, &[u8])] = &[
    (16, ICON_16),
    (24, ICON_24),
    (32, ICON_32),
    (48, ICON_48),
    (64, ICON_64),
    (128, ICON_128),
    (256, ICON_256),
    (512, ICON_512),
];

/// Racines de l'installation, isolées pour les tests.
#[derive(Debug, Clone)]
struct Roots {
    /// `~/.local/bin`
    install: PathBuf,
    /// `$XDG_DATA_HOME` (défaut `~/.local/share`)
    data: PathBuf,
    /// `$XDG_CONFIG_HOME` (défaut `~/.config`)
    config: PathBuf,
}

fn home() -> PathBuf {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn roots() -> Roots {
    Roots {
        install: home().join(".local").join("bin"),
        data: paths::data_home(),
        config: paths::config_home(),
    }
}

/// Répertoire d'installation par utilisateur.
pub fn install_dir() -> PathBuf {
    roots().install
}

pub fn installed_exe() -> PathBuf {
    install_dir().join("fastipswitch")
}

pub fn is_installed() -> bool {
    installed_exe().is_file()
}

pub fn autostart_enabled() -> bool {
    autostart_file().is_file()
}

fn autostart_file() -> PathBuf {
    roots()
        .config
        .join("autostart")
        .join("fastipswitch.desktop")
}

/// Installe l'application dans le répertoire utilisateur et renvoie ce
/// répertoire (`~/.local/bin`).
pub fn install(with_autostart: bool) -> Result<String, String> {
    let roots = roots();
    let source = std::env::current_exe()
        .map_err(|e| format!("Impossible de localiser l'exécutable courant : {e}"))?;

    install_with(&roots, &source, with_autostart)?;
    Ok(roots.install.display().to_string())
}

fn install_with(roots: &Roots, source: &Path, with_autostart: bool) -> Result<(), String> {
    fs::create_dir_all(&roots.install)
        .map_err(|e| io_error("création du répertoire", &roots.install, e))?;

    let target = roots.install.join("fastipswitch");
    if !same_file(source, &target) {
        fs::copy(source, &target)
            .map_err(|e| io_error("copie de l'exécutable vers", &target, e))?;
    }

    // Les exécutables installés restent accessibles à tous, comme le binaire
    // d'origine dans `target/`.
    fs::set_permissions(&target, fs::Permissions::from_mode(0o755))
        .map_err(|e| io_error("changement de permissions de", &target, e))?;

    install_icons(roots)?;
    write_file(
        &roots.data.join("applications").join("fastipswitch.desktop"),
        &desktop_entry(&target),
    )?;

    let autostart = roots.config.join("autostart").join("fastipswitch.desktop");
    if with_autostart {
        write_file(&autostart, &autostart_entry(&target))?;
    } else if autostart.is_file() {
        fs::remove_file(&autostart)
            .map_err(|e| io_error("suppression de l'autostart", &autostart, e))?;
    }

    Ok(())
}

/// Écrit les icônes dans le thème hicolor, une taille par répertoire.
fn install_icons(roots: &Roots) -> Result<(), String> {
    for &(size, bytes) in ICONS {
        let dir = roots
            .data
            .join("icons")
            .join("hicolor")
            .join(format!("{size}x{size}"))
            .join("apps");
        fs::create_dir_all(&dir)
            .map_err(|e| io_error("création du répertoire d'icônes", &dir, e))?;
        let file = dir.join("fastipswitch.png");
        fs::write(&file, bytes).map_err(|e| io_error("écriture de l'icône", &file, e))?;
    }
    Ok(())
}

/// Vrai si les deux chemins désignent le même fichier (même après résolution
/// des liens symboliques). Évite de copier l'exécutable sur lui-même.
fn same_file(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

fn write_file(path: &Path, contents: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| io_error("création du répertoire", parent, e))?;
    }
    fs::write(path, contents).map_err(|e| io_error("écriture de", path, e))
}

fn io_error(action: &str, path: &Path, error: std::io::Error) -> String {
    format!("{action} {} : {error}", path.display())
}

/// Échappe un chemin pour le champ `Exec=` d'un fichier `.desktop`.
fn desktop_exec(path: &Path) -> String {
    let raw = path.display().to_string();
    if raw
        .chars()
        .any(|c| c.is_whitespace() || c == '"' || c == '\'' || c == '\\')
    {
        let escaped = raw.replace('\\', "\\\\").replace('"', "\\\"");
        format!("\"{escaped}\"")
    } else {
        raw
    }
}

fn desktop_entry(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=FastIpSwitch\n\
         Comment=Changement rapide d'adresse IPv4 et gestion d'IP virtuelles\n\
         Exec={}\n\
         Icon=fastipswitch\n\
         Terminal=false\n\
         Categories=Network;Settings;\n\
         StartupWMClass=fastipswitch\n",
        desktop_exec(exe)
    )
}

fn autostart_entry(exe: &Path) -> String {
    format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=FastIpSwitch\n\
         Comment=Changement rapide d'adresse IPv4 et gestion d'IP virtuelles\n\
         Exec={}\n\
         Icon=fastipswitch\n\
         Terminal=false\n\
         X-GNOME-Autostart-enabled=true\n",
        desktop_exec(exe)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    fn temp_root(tag: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "fastipswitch-test-{}-{tag}-{n}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("répertoire de test");
        dir
    }

    fn test_roots(base: &Path) -> Roots {
        Roots {
            install: base.join("bin"),
            data: base.join("share"),
            config: base.join("config"),
        }
    }

    #[test]
    fn installe_binaire_icones_et_entree_de_bureau() {
        let base = temp_root("install");
        let roots = test_roots(&base);
        let source = base.join("source-fastipswitch");
        fs::write(&source, b"binaire factice").unwrap();

        install_with(&roots, &source, false).unwrap();

        let exe = roots.install.join("fastipswitch");
        assert_eq!(fs::read(&exe).unwrap(), b"binaire factice");
        let mode = fs::metadata(&exe).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o755);

        for (size, _) in ICONS {
            let icon = roots
                .data
                .join("icons/hicolor")
                .join(format!("{size}x{size}/apps/fastipswitch.png"));
            assert!(icon.is_file(), "icône manquante : {}", icon.display());
        }

        let desktop = fs::read_to_string(roots.data.join("applications/fastipswitch.desktop"))
            .expect("entrée de bureau");
        assert!(desktop.contains("Type=Application"));
        assert!(desktop.contains("Name=FastIpSwitch"));
        assert!(desktop.contains(&format!("Exec={}", exe.display())));
        assert!(desktop.contains("Icon=fastipswitch"));
        assert!(desktop.contains("Terminal=false"));
        assert!(desktop.contains("Categories=Network;Settings;"));
        assert!(desktop.contains("StartupWMClass=fastipswitch"));

        assert!(!roots.config.join("autostart/fastipswitch.desktop").exists());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn autostart_active_puis_desactive_sans_auto_copie() {
        let base = temp_root("autostart");
        let roots = test_roots(&base);
        let source = base.join("source-fastipswitch");
        fs::write(&source, b"binaire factice").unwrap();

        install_with(&roots, &source, false).unwrap();
        let exe = roots.install.join("fastipswitch");

        // Source = cible : la copie sur soi-même doit être évitée sans erreur.
        install_with(&roots, &exe, true).unwrap();
        assert_eq!(fs::read(&exe).unwrap(), b"binaire factice");

        let autostart = roots.config.join("autostart/fastipswitch.desktop");
        let contents = fs::read_to_string(&autostart).expect("fichier autostart");
        assert!(contents.contains("X-GNOME-Autostart-enabled=true"));
        assert!(contents.contains(&format!("Exec={}", exe.display())));

        install_with(&roots, &exe, false).unwrap();
        assert!(!autostart.exists());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn exec_avec_espaces_est_quote() {
        assert_eq!(desktop_exec(Path::new("/opt/app/bin")), "/opt/app/bin");
        assert_eq!(
            desktop_exec(Path::new("/home/a b/bin")),
            "\"/home/a b/bin\""
        );
        assert_eq!(
            desktop_exec(Path::new("/home/a\"b/bin")),
            "\"/home/a\\\"b/bin\""
        );
    }

    #[test]
    fn auto_copie_detectee() {
        let base = temp_root("same");
        let file = base.join("fichier");
        fs::write(&file, b"x").unwrap();
        assert!(same_file(&file, &file));
        assert!(!same_file(&file, &base.join("absent")));
        let _ = fs::remove_dir_all(&base);
    }
}
