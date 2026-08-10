// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;

fn main() -> anyhow::Result<()> {
    const BRIDGE_FILES: &[&str] = &[
        "bridge/errors.h",
        "bridge/macos_bundle.h",
        "bridge/macos_bundle.mm",
        "bridge/macos_installer.h",
        "bridge/macos_installer.mm",
        "bridge/macos_vm.h",
        "bridge/macos_vm.mm",
        "bridge/vz.h",
        "bridge/vz.mm",
    ];
    for file in BRIDGE_FILES {
        println!("cargo:rerun-if-changed={file}");
    }

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return Ok(());
    }

    cc::Build::new()
        .cpp(true)
        .files([
            "bridge/macos_bundle.mm",
            "bridge/macos_installer.mm",
            "bridge/macos_vm.mm",
            "bridge/vz.mm",
        ])
        .flag("-fobjc-arc")
        .flag("-std=c++20")
        .compile("realm_vz_bridge");

    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=Virtualization");

    Ok(())
}
