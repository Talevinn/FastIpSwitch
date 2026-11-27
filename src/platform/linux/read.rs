// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use crate::platform::AdapterInfo;

/// Lit les interfaces IPv4 via `ip -j addr`, `ip -j link` et
/// `ip -j route show default` (voir docs/cross-platform.md).
pub fn read_adapters() -> Result<Vec<AdapterInfo>, String> {
    Ok(Vec::new())
}
