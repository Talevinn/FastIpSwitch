// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
    CoTaskMemFree, CoUninitialize, IPersistFile,
};
use windows::Win32::UI::Shell::{
    FOLDERID_Desktop, FOLDERID_Programs, IShellLinkW, KF_FLAG_DEFAULT, SHGetKnownFolderPath,
    ShellLink,
};
use windows::core::{HSTRING, Interface};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

pub fn install_dir() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Programs")
        .join("FastIpSwitch")
}

pub fn installed_exe() -> PathBuf {
    install_dir().join("fastipswitch.exe")
}

pub fn is_installed() -> bool {
    installed_exe().is_file()
}

pub fn autostart_enabled() -> bool {
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".to_string());
    let task = format!("FastIpSwitch-{user}");

    Command::new("schtasks")
        .args(["/Query", "/TN", &task])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

/// Copie l'exécutable courant, crée les raccourcis et (dés)active le démarrage.
pub fn install(with_autostart: bool) -> Result<String, String> {
    let current = std::env::current_exe()
        .map_err(|e| format!("chemin de l'exécutable : {e}"))?
        .canonicalize()
        .map_err(|e| format!("canonicalisation de l'exécutable : {e}"))?;
    let dir = install_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("création de {} : {e}", dir.display()))?;

    let dest = installed_exe();
    let already_there = current == dest.canonicalize().unwrap_or_else(|_| dest.clone());

    if !already_there {
        fs::copy(&current, &dest).map_err(|e| {
            format!(
                "copie de {} vers {} : {e}",
                current.display(),
                dest.display()
            )
        })?;

        // Récupère la liste d'IP virtuelles (state.toml) pour la première
        // installation. Ne remplace pas un state déjà installé.
        let mut candidates = Vec::new();
        if let Some(parent) = current.parent() {
            candidates.push(parent.join("data").join("state.toml"));
        }
        if let Some(appdata) = std::env::var_os("APPDATA") {
            let fallback = PathBuf::from(appdata).join("FastIpSwitch");
            candidates.push(fallback.join("state.toml"));
        }

        if let Some(source) = candidates.into_iter().find(|path| path.is_file()) {
            let dst_data = dir.join("data");
            let _ = fs::create_dir_all(&dst_data);
            let destination = dst_data.join("state.toml");
            if !destination.exists()
                && let Err(e) = fs::copy(&source, &destination)
            {
                crate::log::write(&format!("state installé non copié : {e}"));
            }
        }
    }

    create_shortcuts(&dest)?;
    set_autostart(with_autostart, &dest)?;

    Ok(dir.display().to_string())
}

fn create_shortcuts(exe: &Path) -> Result<(), String> {
    let desktop = known_folder(&FOLDERID_Desktop)?;
    create_shortcut(&desktop.join("FastIpSwitch.lnk"), exe)?;

    let programs = known_folder(&FOLDERID_Programs)?;
    let menu_dir = programs.join("FastIpSwitch");
    create_shortcut(&menu_dir.join("FastIpSwitch.lnk"), exe)?;
    Ok(())
}

fn known_folder(id: &windows::core::GUID) -> Result<PathBuf, String> {
    unsafe {
        let ptr = SHGetKnownFolderPath(id, KF_FLAG_DEFAULT, None)
            .map_err(|e| format!("dossier système introuvable : {e}"))?;
        let text = ptr.to_string().map_err(|e| e.to_string())?;
        CoTaskMemFree(Some(ptr.0 as *const _));
        Ok(PathBuf::from(text))
    }
}

fn create_shortcut(lnk: &Path, exe: &Path) -> Result<(), String> {
    if let Some(parent) = lnk.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("création de {} : {e}", parent.display()))?;
    }

    let exe_str = exe.to_string_lossy().to_string();
    let workdir = exe
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let lnk_str = lnk.to_string_lossy().to_string();

    unsafe {
        let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let must_uninit = init.is_ok();
        let result = (|| -> Result<(), String> {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)
                .map_err(|e| format!("création du raccourci : {e}"))?;

            link.SetPath(&HSTRING::from(exe_str.as_str()))
                .map_err(|e| e.to_string())?;
            link.SetArguments(&HSTRING::from("--elevate"))
                .map_err(|e| e.to_string())?;
            link.SetWorkingDirectory(&HSTRING::from(workdir.as_str()))
                .map_err(|e| e.to_string())?;
            link.SetIconLocation(&HSTRING::from(exe_str.as_str()), 0)
                .map_err(|e| e.to_string())?;
            link.SetDescription(&HSTRING::from("FastIpSwitch"))
                .map_err(|e| e.to_string())?;

            let file: IPersistFile = link.cast().map_err(|e| e.to_string())?;
            file.Save(&HSTRING::from(lnk_str.as_str()), true)
                .map_err(|e| e.to_string())?;
            Ok(())
        })();
        if must_uninit {
            CoUninitialize();
        }
        result
    }
}

fn set_autostart(enabled: bool, exe: &Path) -> Result<(), String> {
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "user".to_string());
    let task = format!("FastIpSwitch-{user}");

    let mut command = Command::new("schtasks");
    if enabled {
        command.args([
            "/Create",
            "/F",
            "/SC",
            "ONLOGON",
            "/RL",
            "HIGHEST",
            "/TN",
            &task,
            "/TR",
            &exe.to_string_lossy(),
        ]);
    } else {
        command.args(["/Delete", "/F", "/TN", &task]);
    }
    command.creation_flags(CREATE_NO_WINDOW);

    let output = command.output().map_err(|e| format!("schtasks : {e}"))?;

    if enabled && !output.status.success() {
        let details = String::from_utf8_lossy(&output.stdout);
        let details = details.trim();
        return Err(if details.is_empty() {
            "création de la tâche planifiée impossible".to_string()
        } else {
            details.to_string()
        });
    }

    Ok(())
}
