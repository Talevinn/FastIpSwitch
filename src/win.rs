// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use windows::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, ERROR_ALREADY_EXISTS, GetLastError,
};
use windows::Win32::System::Threading::{CreateMutexW, OpenMutexW, SYNCHRONIZATION_SYNCHRONIZE};
use windows::Win32::UI::Shell::{IsUserAnAdmin, ShellExecuteW};
use windows::Win32::UI::WindowsAndMessaging::{
    MB_ICONERROR, MB_ICONINFORMATION, MB_OK, MessageBoxW, SW_SHOWNORMAL,
};
use windows::core::{HSTRING, PCWSTR};

const ADMIN_MUTEX: &str = "Global\\FastIpSwitch";
const USER_MUTEX: &str = "Global\\FastIpSwitch-User";

/// Une instance élevée tourne-t-elle déjà ?
pub fn admin_instance_running() -> bool {
    unsafe {
        let name = HSTRING::from(ADMIN_MUTEX);
        match OpenMutexW(SYNCHRONIZATION_SYNCHRONIZE, false, &name) {
            Ok(handle) => {
                let _ = CloseHandle(handle);
                true
            }
            // Accès refusé = le mutex existe mais appartient à un process élevé.
            Err(_) => GetLastError() == ERROR_ACCESS_DENIED,
        }
    }
}

/// `false` si une instance du même niveau (admin ou non) tourne déjà.
pub fn single_instance(admin: bool) -> bool {
    if !admin && admin_instance_running() {
        return false;
    }

    unsafe {
        let name = HSTRING::from(if admin { ADMIN_MUTEX } else { USER_MUTEX });
        match CreateMutexW(None, false, &name) {
            Ok(_handle) => GetLastError() != ERROR_ALREADY_EXISTS,
            Err(_) => true,
        }
    }
}

pub fn is_admin() -> bool {
    unsafe { IsUserAnAdmin().as_bool() }
}

/// Relance l'exécutable courant via UAC (`runas`).
pub fn elevate_self() -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("chemin de l'exécutable : {e}"))?;
    let dir = exe
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    let operation = HSTRING::from("runas");
    let file = HSTRING::from(exe.to_string_lossy().as_ref());
    let directory = HSTRING::from(dir.to_string_lossy().as_ref());

    let result = unsafe {
        ShellExecuteW(
            None,
            &operation,
            &file,
            PCWSTR::null(),
            &directory,
            SW_SHOWNORMAL,
        )
    };

    let code = result.0 as isize;
    if code > 32 {
        Ok(())
    } else {
        Err(format!("élévation annulée ou impossible (code {code})"))
    }
}

/// Thème sombre actif dans les paramètres Windows (Applications).
pub fn system_is_dark() -> bool {
    use winreg::RegKey;
    use winreg::enums::HKEY_CURRENT_USER;

    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .and_then(|key| key.get_value::<u32, _>("AppsUseLightTheme"))
        .map(|value| value == 0)
        .unwrap_or(true)
}

pub fn error_box(message: &str) {
    boxed(message, "FastIpSwitch", MB_OK | MB_ICONERROR);
}

pub fn info_box(message: &str) {
    boxed(message, "FastIpSwitch", MB_OK | MB_ICONINFORMATION);
}

fn boxed(
    message: &str,
    title: &str,
    flags: windows::Win32::UI::WindowsAndMessaging::MESSAGEBOX_STYLE,
) {
    unsafe {
        MessageBoxW(None, &HSTRING::from(message), &HSTRING::from(title), flags);
    }
}
