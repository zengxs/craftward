// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;

fn main() -> anyhow::Result<()> {
    println!("cargo:rerun-if-changed=bridge/vz_bridge.h");
    println!("cargo:rerun-if-changed=bridge/vz_bridge.mm");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return Ok(());
    }

    cc::Build::new()
        .file("bridge/vz_bridge.mm")
        .flag("-fobjc-arc")
        .flag("-std=c++20")
        .compile("realm_vz_bridge");

    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=Virtualization");

    Ok(())
}
