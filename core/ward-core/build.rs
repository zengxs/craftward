// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{env, io, path::PathBuf};

const PROTO_FILES: &[&str] = &["ward/codex/v1/history.proto"];

fn main() -> io::Result<()> {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("Cargo must set CARGO_MANIFEST_DIR"),
    );
    let repository_root = manifest_dir
        .parent()
        .and_then(|core_dir| core_dir.parent())
        .expect("ward-core must be located under <repository>/core");
    let proto_root = repository_root.join("proto");
    let proto_files = PROTO_FILES
        .iter()
        .map(|path| proto_root.join(path))
        .collect::<Vec<_>>();

    for proto_file in &proto_files {
        println!("cargo:rerun-if-changed={}", proto_file.display());
    }

    prost_build::compile_protos(&proto_files, &[proto_root])
}
