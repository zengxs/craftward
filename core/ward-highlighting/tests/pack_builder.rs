// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "../build_support/pack.rs"]
mod pack;
#[path = "../src/theme.rs"]
mod theme;

use std::fs;
use std::path::{Path, PathBuf};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(repository_root: &Path) -> Self {
        let path = repository_root
            .join(".tmp/ward-highlighting-pack-tests")
            .join(std::process::id().to_string());
        if path.exists() {
            fs::remove_dir_all(&path).expect("the stale test directory should be removable");
        }
        fs::create_dir_all(&path).expect("the test directory should be creatable");
        Self(path)
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("the test directory should be removable");
    }
}

#[test]
fn compiles_the_maintained_assets_through_the_pack_interface() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repository_root = manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("the crate should be located under <repository>/core");
    let output = TestDirectory::new(repository_root);

    let summary = pack::compile_assets(&manifest_dir.join("assets"), &output.0)
        .expect("the maintained assets should compile");

    assert!(summary.syntax_count > 10);
    assert_eq!(summary.theme_count, 2);
    assert!(output.0.join(pack::SYNTAX_PACK_FILE).is_file());
    assert!(output.0.join(pack::THEME_PACK_FILE).is_file());
    assert!(pack::asset_directories().any(|directory| directory == "Packages"));
}
