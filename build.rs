// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Alatus Contributors

fn main() {
    #[cfg(feature = "gui")]
    slint_build::compile("ui/dashboard.slint").expect("Slint compilation failed");
}
