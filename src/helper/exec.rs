// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

use super::protocol::Operation;

/// Exécute une opération déjà validée. Le helper tourne en root.
pub fn execute(_op: &Operation) -> Result<(), String> {
    Err("opération non implémentée".into())
}
