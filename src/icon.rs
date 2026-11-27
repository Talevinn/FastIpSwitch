// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use slint::{Image, Rgba8Pixel, SharedPixelBuffer};

/// Icône de la zone de notification (32×32, RGBA brut embarqué).
pub fn tray_image() -> Image {
    from_rgba(include_bytes!("../assets/tray32.rgba"), 32)
}

/// Logo affiché dans l'en-tête (24×24, RGBA brut embarqué).
pub fn logo_image() -> Image {
    from_rgba(include_bytes!("../assets/logo24.rgba"), 24)
}

fn from_rgba(bytes: &[u8], size: u32) -> Image {
    let buffer = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(bytes, size, size);
    Image::from_rgba8(buffer)
}
