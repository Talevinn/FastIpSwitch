// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Valentin PICARD

//! Logique commune à l'interface et au helper privilégié.
//!
//! `platform` isole les différences Windows/Linux : lecture des interfaces,
//! commandes réseau, chemins, thème, instanciation unique et installation.

pub mod cidr;
pub mod helper;
pub mod i18n;
pub mod log;
pub mod platform;
pub mod store;
pub mod worker;
