// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, anyhow, bail};

const GENERATE_COMMAND: &str = "task xtask:bindings:generate";

pub struct ProjectPaths {
    pub crate_dir: PathBuf,
    pub config: PathBuf,
    pub header: PathBuf,
}

impl Default for ProjectPaths {
    fn default() -> Self {
        let xtask_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repository_dir = xtask_dir
            .parent()
            .expect("xtask must be located directly under the repository root");
        let crate_dir = repository_dir.join("core/ward-core");

        Self {
            config: crate_dir.join("cbindgen.toml"),
            header: crate_dir.join("include/ward_core.h"),
            crate_dir,
        }
    }
}

pub fn generate(paths: &ProjectPaths) -> Result<bool> {
    let generated = render(paths)?;
    if fs::read(&paths.header).ok().as_deref() == Some(generated.as_slice()) {
        return Ok(false);
    }

    let parent = paths
        .header
        .parent()
        .expect("the generated header path must have a parent");
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create the generated header directory {}",
            parent.display()
        )
    })?;
    fs::write(&paths.header, generated).with_context(|| {
        format!(
            "failed to write the generated Ward Core C interface {}",
            paths.header.display()
        )
    })?;
    Ok(true)
}

pub fn check(paths: &ProjectPaths) -> Result<()> {
    let generated = render(paths)?;
    let committed = fs::read(&paths.header).with_context(|| {
        format!(
            "failed to read the generated Ward Core C interface {}; run `{GENERATE_COMMAND}`",
            paths.header.display()
        )
    })?;
    if committed != generated {
        bail!(
            "generated Ward Core C interface {} is stale; run `{GENERATE_COMMAND}`",
            paths.header.display()
        );
    }
    Ok(())
}

fn render(paths: &ProjectPaths) -> Result<Vec<u8>> {
    let config = cbindgen::Config::from_file(&paths.config).map_err(|error| {
        anyhow!(
            "failed to load the cbindgen configuration {}: {error}",
            paths.config.display()
        )
    })?;
    let bindings = cbindgen::Builder::new()
        .with_crate(&paths.crate_dir)
        .with_config(config)
        .generate()
        .with_context(|| {
            format!(
                "failed to generate the Ward Core C interface from {}",
                paths.crate_dir.display()
            )
        })?;

    let mut generated = Vec::new();
    bindings.write(&mut generated);
    Ok(generated)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{ProjectPaths, check, generate};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let repository_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("xtask must be located directly under the repository root");
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("the system clock should follow the Unix epoch")
                .as_nanos();
            let path = repository_dir
                .join(".tmp/xtask-bindings-tests")
                .join(format!("{}-{nonce}", std::process::id()));
            fs::create_dir_all(&path).expect("the test directory should be created");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("the test directory should be removed");
            if let Some(parent) = self.0.parent() {
                let _ = fs::remove_dir(parent);
            }
        }
    }

    #[test]
    fn generates_and_checks_a_header() {
        let sandbox = TestDirectory::new();
        let crate_dir = sandbox.0.join("ffi-crate");
        fs::create_dir_all(crate_dir.join("src")).expect("the test crate source should be created");
        fs::write(
            crate_dir.join("Cargo.toml"),
            "[package]\nname = \"ffi-test\"\nversion = \"0.0.0\"\nedition = \"2024\"\n",
        )
        .expect("the test manifest should be written");
        fs::write(
            crate_dir.join("src/lib.rs"),
            "#[unsafe(no_mangle)]\npub extern \"C\" fn ffi_answer() -> u32 { 42 }\n",
        )
        .expect("the test source should be written");
        let config = crate_dir.join("cbindgen.toml");
        fs::write(&config, "language = \"C\"\n").expect("the test configuration should be written");
        let header = crate_dir.join("include/ffi.h");
        let paths = ProjectPaths {
            crate_dir,
            config,
            header: header.clone(),
        };

        assert!(generate(&paths).expect("the first generation should succeed"));
        assert!(!generate(&paths).expect("unchanged generation should succeed"));
        check(&paths).expect("the generated header should pass its check");

        fs::write(&header, "stale\n").expect("the generated header should be made stale");
        let error = check(&paths).expect_err("a stale header should fail its check");
        assert!(error.to_string().contains("stale"));
    }
}
