// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Détection du thème sombre du bureau.

use super::exec;

/// Fragments de nom de thème (GTK, XFCE, Plasma) indiquant un thème sombre.
/// `nokto` couvre les thèmes Adapta, `black` les variantes BlackMATE.
const DARK_TOKENS: [&str; 4] = ["dark", "sombre", "nokto", "black"];

/// Interprète `org.gnome.desktop.interface color-scheme`.
/// `None` signifie « pas d'information » (valeur vide ou `default`).
fn dark_from_color_scheme(value: &str) -> Option<bool> {
    let value = value.to_lowercase();
    if value.contains("prefer-dark") {
        Some(true)
    } else if value.contains("prefer-light") {
        Some(false)
    } else {
        None
    }
}

/// Interprète un nom de thème GTK / XFCE / couleur Plasma.
fn dark_from_theme_name(value: &str) -> Option<bool> {
    let value = value
        .trim()
        .trim_matches(|c| c == '\'' || c == '"')
        .to_lowercase();

    if value.is_empty() || value == "default" {
        return None;
    }

    Some(DARK_TOKENS.iter().any(|token| value.contains(token)))
}

/// Thème sombre du bureau.
///
/// Ordre de détection : GNOME (`color-scheme`), Cinnamon (`gtk-theme`),
/// XFCE (`xfconf-query`), puis KDE Plasma (`kreadconfig6`/`kreadconfig5`).
/// Un outil absent ou en échec est simplement ignoré ; en dernier recours on
/// considère le thème clair. Toutes les commandes passent par `run_quiet` et
/// sont des lectures ponctuelles, sans interaction ni blocage durable.
pub fn system_is_dark() -> bool {
    if let Ok(scheme) = exec::run_quiet(
        "gsettings",
        &["get", "org.gnome.desktop.interface", "color-scheme"],
    ) && let Some(dark) = dark_from_color_scheme(&scheme)
    {
        return dark;
    }

    if let Ok(theme) = exec::run_quiet(
        "gsettings",
        &["get", "org.cinnamon.desktop.interface", "gtk-theme"],
    ) && let Some(dark) = dark_from_theme_name(&theme)
    {
        return dark;
    }

    if let Ok(theme) = exec::run_quiet("xfconf-query", &["-c", "xsettings", "-p", "/Net/ThemeName"])
        && let Some(dark) = dark_from_theme_name(&theme)
    {
        return dark;
    }

    for program in ["kreadconfig6", "kreadconfig5"] {
        if let Ok(scheme) =
            exec::run_quiet(program, &["--group", "General", "--key", "ColorScheme"])
            && let Some(dark) = dark_from_theme_name(&scheme)
        {
            return dark;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_scheme_gnome() {
        assert_eq!(dark_from_color_scheme("'prefer-dark'"), Some(true));
        assert_eq!(dark_from_color_scheme("'prefer-light'"), Some(false));
        assert_eq!(dark_from_color_scheme("'default'"), None);
        assert_eq!(dark_from_color_scheme(""), None);
    }

    #[test]
    fn noms_de_theme_sombres() {
        assert_eq!(dark_from_theme_name("'Adwaita-dark'"), Some(true));
        assert_eq!(dark_from_theme_name("BreezeDark"), Some(true));
        assert_eq!(dark_from_theme_name("Adapta-Nokto"), Some(true));
        assert_eq!(dark_from_theme_name("BlackMATE"), Some(true));
    }

    #[test]
    fn noms_de_theme_clairs() {
        assert_eq!(dark_from_theme_name("'Adwaita'"), Some(false));
        assert_eq!(dark_from_theme_name("Breeze"), Some(false));
        assert_eq!(dark_from_theme_name("Greybird"), Some(false));
        assert_eq!(dark_from_theme_name("default"), None);
        assert_eq!(dark_from_theme_name("  "), None);
    }
}
