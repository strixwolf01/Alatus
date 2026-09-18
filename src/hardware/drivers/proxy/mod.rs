// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

//! D-Bus fallback proxies for external hardware daemons (e.g. asusd).

pub mod asusctl;

pub use asusctl::{AsusctlProxyDriver, MockAsusdState};
