// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::error::Error;
use std::path::PathBuf;

#[path = "build_support/pack.rs"]
mod pack;
#[path = "src/theme.rs"]
mod theme;

fn main() -> Result<(), Box<dyn Error>> {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must set CARGO_MANIFEST_DIR"),
    );
    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("Cargo must set OUT_DIR"));

    for path in ["build_support/pack.rs", "src/theme.rs"] {
        println!("cargo:rerun-if-changed={path}");
    }
    let asset_root = manifest_dir.join("assets");
    for directory in pack::asset_directories() {
        println!(
            "cargo:rerun-if-changed={}",
            asset_root.join(directory).display()
        );
    }

    pack::compile_assets(&asset_root, &output_dir)?;
    Ok(())
}
