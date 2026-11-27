// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use super::exec;

/// Thème sombre du bureau. `gsettings` est disponible sur GNOME, Cinnamon et
/// MATE ; en dernier recours on retombe sur le thème clair.
pub fn system_is_dark() -> bool {
    if let Ok(scheme) = exec::run_quiet(
        "gsettings",
        &["get", "org.gnome.desktop.interface", "color-scheme"],
    ) {
        let scheme = scheme.to_lowercase();
        if scheme.contains("prefer-dark") {
            return true;
        }
        if scheme.contains("prefer-light") {
            return false;
        }
    }

    for schema in [
        "org.cinnamon.desktop.interface",
        "org.gnome.desktop.interface",
    ] {
        if let Ok(theme) = exec::run_quiet("gsettings", &["get", schema, "gtk-theme"]) {
            let theme = theme.to_lowercase();
            if !theme.trim().is_empty() {
                return theme.contains("dark");
            }
        }
    }

    false
}
