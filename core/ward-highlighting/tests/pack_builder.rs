// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#[path = "../build_support/pack.rs"]
mod pack;
#[path = "../src/theme.rs"]
mod theme;

use std::fs;
use std::path::{Path, PathBuf};

use syntect::dumps::from_dump_file;
use syntect::highlighting::{Color, FontStyle, ScopeSelectors, Theme, ThemeSet};

const fn opaque(red: u8, green: u8, blue: u8) -> Color {
    Color {
        r: red,
        g: green,
        b: blue,
        a: u8::MAX,
    }
}

fn assert_scope_foregrounds(theme: &Theme, expectations: &[(&str, Color)]) {
    for &(selector, expected) in expectations {
        let selectors: ScopeSelectors = selector.parse().unwrap_or_else(|error| {
            panic!("the maintained selector {selector:?} should parse: {error}")
        });
        let actual = theme
            .scopes
            .iter()
            .find(|item| item.scope == selectors)
            .unwrap_or_else(|| panic!("the maintained theme should contain {selector:?}"));

        assert_eq!(actual.style.foreground, Some(expected), "{selector:?}");
    }
}

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

    assert_eq!(summary.theme_count, theme::Theme::ALL.len());
    assert!(output.0.join(pack::SYNTAX_PACK_FILE).is_file());
    assert!(output.0.join(pack::THEME_PACK_FILE).is_file());
    assert!(pack::asset_directories().any(|directory| directory == "Packages"));

    let themes: ThemeSet = from_dump_file(output.0.join(pack::THEME_PACK_FILE))
        .expect("the generated theme pack should load");
    for application_theme in theme::Theme::ALL {
        let loaded = themes
            .themes
            .get(application_theme.name())
            .expect("each application theme should be packed under its runtime name");
        assert!(loaded.settings.foreground.is_some());
        assert!(loaded.settings.background.is_some());
    }

    let light = themes
        .themes
        .get(theme::Theme::Light.name())
        .expect("One Light should be available");
    assert_eq!(
        light.settings.foreground,
        Some(Color {
            r: 0x38,
            g: 0x3a,
            b: 0x42,
            a: 0xff,
        })
    );
    assert_eq!(
        light.settings.background,
        Some(Color {
            r: 0xfa,
            g: 0xfa,
            b: 0xfa,
            a: 0xff,
        })
    );
    assert!(light.scopes.iter().any(|item| {
        item.style
            .font_style
            .is_some_and(|style| style.contains(FontStyle::UNDERLINE))
    }));
    assert_scope_foregrounds(
        light,
        &[
            (
                "comment, punctuation.definition.comment",
                opaque(0x5c, 0x63, 0x70),
            ),
            (
                "string, string entity.name.function",
                opaque(0x50, 0xa1, 0x4f),
            ),
            ("keyword", opaque(0xa6, 0x26, 0xa4)),
            ("entity.name.tag", opaque(0xe4, 0x56, 0x49)),
            ("meta.mapping.key string", opaque(0xe4, 0x56, 0x49)),
        ],
    );

    let dark = themes
        .themes
        .get(theme::Theme::Dark.name())
        .expect("One Dark should be available");
    assert_eq!(dark.name.as_deref(), Some("One Dark"));
    assert_eq!(
        dark.settings.foreground,
        Some(Color {
            r: 0xab,
            g: 0xb2,
            b: 0xbf,
            a: 0xff,
        })
    );
    assert_eq!(
        dark.settings.background,
        Some(Color {
            r: 0x28,
            g: 0x2c,
            b: 0x34,
            a: 0xff,
        })
    );
    assert_scope_foregrounds(
        dark,
        &[
            (
                "comment, punctuation.definition.comment",
                opaque(0x5c, 0x63, 0x70),
            ),
            (
                "string, string entity.name.function",
                opaque(0x98, 0xc3, 0x79),
            ),
            ("keyword", opaque(0xc6, 0x78, 0xdd)),
            ("entity.name.tag", opaque(0xe0, 0x6c, 0x75)),
            ("meta.mapping.key string", opaque(0xe0, 0x6c, 0x75)),
        ],
    );
}
