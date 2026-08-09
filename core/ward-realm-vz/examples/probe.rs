// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

fn main() {
    println!(
        "Virtualization.framework supported: {}",
        ward_realm_vz::is_supported()
    );
}
