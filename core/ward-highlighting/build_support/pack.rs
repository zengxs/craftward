// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

use syntect::dumps::{dump_to_file, dump_to_uncompressed_file};
use syntect::highlighting::ThemeSet;
use syntect::parsing::syntax_definition::Context;
use syntect::parsing::{Scope, SyntaxDefinition, SyntaxSet, SyntaxSetBuilder};
use thiserror::Error;

use crate::theme::Theme;

pub const SYNTAX_PACK_FILE: &str = "syntaxes.packdump";
pub const THEME_PACK_FILE: &str = "themes.packdump";

const REQUIRED_SYNTAXES: &[&str] = &[
    "Plain Text",
    "Rust",
    "JSON",
    "Diff",
    "QML",
    "INI",
    "Swift",
    "Zig",
    "Protocol Buffer",
    "CMake",
];

const PACKAGES_DIRECTORY: &str = "Packages";
const PLAIN_TEXT_SOURCE: &str = "Text/Plain text.tmLanguage";
const THEME_DIRECTORY: &str = "Themes";

const SYNTAX_SOURCES: &[SyntaxSource] = &[
    SyntaxSource::standard("Sublime Packages", PACKAGES_DIRECTORY),
    SyntaxSource::qml("QML", "QML"),
    SyntaxSource::standard("INI", "INI"),
    SyntaxSource::standard("Swift", "Swift-Next"),
    SyntaxSource::standard("Zig", "Zig"),
    SyntaxSource::standard("Protocol Buffer", "Protobuf"),
    SyntaxSource::standard("CMake", "CMake"),
];

// Syntect at the pinned revision treats a false meta_append directive as a
// match rule. Removing it preserves Sublime's default replace merge mode. The
// exact-count check below makes an upstream change fail visibly instead of
// leaving a stale compatibility transformation in place.
const QML_META_APPEND_FALSE: &str = "    - meta_append: false\n";

#[derive(Clone, Copy)]
struct SyntaxSource {
    label: &'static str,
    directory: &'static str,
    loader: SyntaxLoader,
}

impl SyntaxSource {
    const fn standard(label: &'static str, directory: &'static str) -> Self {
        Self {
            label,
            directory,
            loader: SyntaxLoader::Standard,
        }
    }

    const fn qml(label: &'static str, directory: &'static str) -> Self {
        Self {
            label,
            directory,
            loader: SyntaxLoader::QmlCompatibility,
        }
    }
}

#[derive(Clone, Copy)]
enum SyntaxLoader {
    Standard,
    QmlCompatibility,
}

pub fn asset_directories() -> impl Iterator<Item = &'static str> {
    SYNTAX_SOURCES
        .iter()
        .map(|source| source.directory)
        .chain(std::iter::once(THEME_DIRECTORY))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PackSummary {
    pub syntax_count: usize,
    pub theme_count: usize,
}

#[derive(Debug, Error)]
pub enum PackError {
    #[error("the {label} asset directory is missing or uninitialized: {path}")]
    MissingAssetDirectory { label: &'static str, path: PathBuf },
    #[error("the {label} asset directory contains no loadable Sublime syntax: {path}")]
    EmptySyntaxSource { label: &'static str, path: PathBuf },
    #[error("failed to load {label} syntaxes from {path}: {source}")]
    LoadSyntaxes {
        label: &'static str,
        path: PathBuf,
        #[source]
        source: syntect::LoadingError,
    },
    #[error("failed to inspect syntax assets under {path}: {source}")]
    InspectSyntaxDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to read the syntax asset {path}: {source}")]
    ReadSyntax {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("the maintained syntax asset {path} is invalid: {message}")]
    InvalidSyntax { path: PathBuf, message: String },
    #[error("failed to load the legacy syntax asset {path}: {source}")]
    LoadLegacySyntax {
        path: PathBuf,
        #[source]
        source: plist::Error,
    },
    #[error("the expected QML compatibility patch does not apply exactly once to {path}")]
    QmlCompatibilityPatchMismatch { path: PathBuf },
    #[error("syntax pack validation failed:\n{0}")]
    InvalidSyntaxPack(String),
    #[error("failed to load themes from {path}: {source}")]
    LoadThemes {
        path: PathBuf,
        #[source]
        source: syntect::LoadingError,
    },
    #[error("the maintained theme {0:?} is missing")]
    MissingTheme(&'static str),
    #[error("failed to create the pack output directory {path}: {source}")]
    CreateOutputDirectory {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write the syntax pack {path}: {source}")]
    WriteSyntaxPack {
        path: PathBuf,
        #[source]
        source: syntect::dumps::DumpError,
    },
    #[error("failed to write the theme pack {path}: {source}")]
    WriteThemePack {
        path: PathBuf,
        #[source]
        source: syntect::dumps::DumpError,
    },
}

/// Compiles all maintained syntax and theme assets into runtime pack files.
pub fn compile_assets(asset_root: &Path, output_dir: &Path) -> Result<PackSummary, PackError> {
    let syntaxes = load_syntaxes(asset_root)?;
    validate_syntaxes(&syntaxes)?;

    let theme_directory = asset_root.join(THEME_DIRECTORY);
    ensure_directory("theme", &theme_directory)?;
    let themes =
        ThemeSet::load_from_folder(&theme_directory).map_err(|source| PackError::LoadThemes {
            path: theme_directory.clone(),
            source,
        })?;
    for required in Theme::ALL {
        if !themes.themes.contains_key(required.name()) {
            return Err(PackError::MissingTheme(required.name()));
        }
    }

    fs::create_dir_all(output_dir).map_err(|source| PackError::CreateOutputDirectory {
        path: output_dir.to_owned(),
        source,
    })?;
    let syntax_pack = output_dir.join(SYNTAX_PACK_FILE);
    dump_to_uncompressed_file(&syntaxes, &syntax_pack).map_err(|source| {
        PackError::WriteSyntaxPack {
            path: syntax_pack,
            source,
        }
    })?;
    let theme_pack = output_dir.join(THEME_PACK_FILE);
    dump_to_file(&themes, &theme_pack).map_err(|source| PackError::WriteThemePack {
        path: theme_pack,
        source,
    })?;

    Ok(PackSummary {
        syntax_count: syntaxes.syntaxes().len(),
        theme_count: themes.themes.len(),
    })
}

fn load_syntaxes(asset_root: &Path) -> Result<SyntaxSet, PackError> {
    let mut builder = SyntaxSetBuilder::new();
    add_project_plain_text_syntax(
        &mut builder,
        &asset_root.join(PACKAGES_DIRECTORY).join(PLAIN_TEXT_SOURCE),
    )?;

    for source in SYNTAX_SOURCES {
        let directory = asset_root.join(source.directory);
        ensure_directory(source.label, &directory)?;
        let previous_count = builder.syntaxes().len();
        match source.loader {
            SyntaxLoader::Standard => {
                builder.add_from_folder(&directory, true).map_err(|error| {
                    PackError::LoadSyntaxes {
                        label: source.label,
                        path: directory.clone(),
                        source: error,
                    }
                })?
            }
            SyntaxLoader::QmlCompatibility => {
                add_qml_syntaxes(&mut builder, &directory)?;
            }
        }
        if builder.syntaxes().len() == previous_count {
            return Err(PackError::EmptySyntaxSource {
                label: source.label,
                path: directory,
            });
        }
    }

    let mut warnings = builder.warnings().to_vec();
    let syntaxes = builder.build();
    warnings.extend(syntaxes.warnings().iter().cloned());
    warnings.sort();
    warnings.dedup();
    if !warnings.is_empty() {
        return Err(PackError::InvalidSyntaxPack(warnings.join("\n")));
    }
    Ok(syntaxes)
}

fn add_project_plain_text_syntax(
    builder: &mut SyntaxSetBuilder,
    path: &Path,
) -> Result<(), PackError> {
    let value = plist::Value::from_file(path).map_err(|source| PackError::LoadLegacySyntax {
        path: path.to_owned(),
        source,
    })?;
    let dictionary = value
        .as_dictionary()
        .ok_or_else(|| invalid_syntax(path, "the plist root is not a dictionary"))?;
    let name = legacy_string(dictionary, "name", path)?;
    let scope_name = legacy_string(dictionary, "scopeName", path)?;
    let patterns = dictionary
        .get("patterns")
        .and_then(plist::Value::as_array)
        .ok_or_else(|| invalid_syntax(path, "patterns is not an array"))?;
    if !patterns.is_empty() {
        return Err(invalid_syntax(
            path,
            "Plain Text unexpectedly contains highlighting patterns",
        ));
    }
    let file_extensions = dictionary
        .get("fileTypes")
        .and_then(plist::Value::as_array)
        .ok_or_else(|| invalid_syntax(path, "fileTypes is not an array"))?
        .iter()
        .map(|value| {
            value
                .as_string()
                .map(str::to_owned)
                .ok_or_else(|| invalid_syntax(path, "fileTypes contains a non-string value"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let scope = Scope::new(scope_name)
        .map_err(|error| invalid_syntax(path, format!("scopeName is invalid: {error}")))?;
    let contexts = HashMap::from([("main".to_owned(), Context::new(None))]);

    builder.add(SyntaxDefinition {
        name: name.to_owned(),
        file_extensions,
        scope,
        first_line_match: None,
        hidden: false,
        variables: HashMap::new(),
        contexts,
        extends: Vec::new(),
        version: 1,
    });
    Ok(())
}

fn legacy_string<'a>(
    dictionary: &'a plist::Dictionary,
    key: &str,
    path: &Path,
) -> Result<&'a str, PackError> {
    dictionary
        .get(key)
        .and_then(plist::Value::as_string)
        .ok_or_else(|| invalid_syntax(path, format!("{key} is not a string")))
}

fn invalid_syntax(path: &Path, message: impl Into<String>) -> PackError {
    PackError::InvalidSyntax {
        path: path.to_owned(),
        message: message.into(),
    }
}

fn add_qml_syntaxes(builder: &mut SyntaxSetBuilder, directory: &Path) -> Result<(), PackError> {
    let mut syntax_paths = Vec::new();
    discover_syntaxes(directory, &mut syntax_paths)?;
    syntax_paths.sort();

    for path in syntax_paths {
        let mut source = fs::read_to_string(&path).map_err(|error| PackError::ReadSyntax {
            path: path.clone(),
            source: error,
        })?;
        if path.ends_with("Support/QML.sublime-syntax") {
            if source.matches(QML_META_APPEND_FALSE).count() != 1 {
                return Err(PackError::QmlCompatibilityPatchMismatch { path });
            }
            source = source.replacen(QML_META_APPEND_FALSE, "", 1);
        }

        let fallback_name = path.file_stem().and_then(|name| name.to_str());
        let syntax =
            SyntaxDefinition::load_from_str(&source, true, fallback_name).map_err(|error| {
                PackError::InvalidSyntax {
                    path: path.clone(),
                    message: error.to_string(),
                }
            })?;
        builder.add(syntax);
    }
    Ok(())
}

fn discover_syntaxes(directory: &Path, output: &mut Vec<PathBuf>) -> Result<(), PackError> {
    let entries = fs::read_dir(directory).map_err(|error| PackError::InspectSyntaxDirectory {
        path: directory.to_owned(),
        source: error,
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| PackError::InspectSyntaxDirectory {
            path: directory.to_owned(),
            source: error,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|error| PackError::InspectSyntaxDirectory {
                path: path.clone(),
                source: error,
            })?;
        if file_type.is_dir() {
            discover_syntaxes(&path, output)?;
        } else if path
            .extension()
            .is_some_and(|extension| extension == "sublime-syntax")
        {
            output.push(path);
        }
    }
    Ok(())
}

fn validate_syntaxes(syntaxes: &SyntaxSet) -> Result<(), PackError> {
    let mut problems = duplicate_syntaxes(syntaxes);

    for required in REQUIRED_SYNTAXES {
        if syntaxes.find_syntax_by_name(required).is_none() {
            problems.push(format!("the maintained syntax {required:?} is missing"));
        }
    }

    problems.extend(syntaxes.find_unlinked_contexts());
    if problems.is_empty() {
        Ok(())
    } else {
        problems.sort();
        Err(PackError::InvalidSyntaxPack(problems.join("\n")))
    }
}

fn duplicate_syntaxes(syntaxes: &SyntaxSet) -> Vec<String> {
    let mut names = BTreeMap::<&str, Vec<(&str, bool)>>::new();
    let mut scopes = BTreeMap::<String, Vec<(&str, bool)>>::new();
    for syntax in syntaxes.syntaxes() {
        names
            .entry(&syntax.name)
            .or_default()
            .push((&syntax.name, syntax.hidden));
        scopes
            .entry(syntax.scope.to_string())
            .or_default()
            .push((&syntax.name, syntax.hidden));
    }

    let mut duplicates = Vec::new();
    for (name, definitions) in names {
        if is_ambiguous_duplicate(&definitions) {
            duplicates.push(format!(
                "syntax name {name:?} is defined {} times",
                definitions.len()
            ));
        }
    }
    for (scope, definitions) in scopes {
        if is_ambiguous_duplicate(&definitions) {
            duplicates.push(format!(
                "syntax scope {scope:?} is shared by {}",
                definitions
                    .iter()
                    .map(|(name, _)| *name)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }
    duplicates
}

fn is_ambiguous_duplicate(definitions: &[(&str, bool)]) -> bool {
    if definitions.len() < 2 {
        return false;
    }
    let visible_count = definitions.iter().filter(|(_, hidden)| !hidden).count();
    let lookup_winner_is_visible = definitions.last().is_some_and(|(_, hidden)| !hidden);
    visible_count != 1 || !lookup_winner_is_visible
}

fn ensure_directory(label: &'static str, path: &Path) -> Result<(), PackError> {
    if path.is_dir() {
        Ok(())
    } else {
        Err(PackError::MissingAssetDirectory {
            label,
            path: path.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use syntect::parsing::{SyntaxDefinition, SyntaxSetBuilder};

    use super::duplicate_syntaxes;

    #[test]
    fn reports_duplicate_names_and_scopes() {
        let mut builder = SyntaxSetBuilder::new();
        for source in [
            "name: Duplicate\nscope: source.one\ncontexts: {main: []}",
            "name: Duplicate\nscope: source.two\ncontexts: {main: []}",
            "name: Third\nscope: source.one\ncontexts: {main: []}",
        ] {
            builder.add(
                SyntaxDefinition::load_from_str(source, true, None)
                    .expect("the fixture syntax should load"),
            );
        }

        let problems = duplicate_syntaxes(&builder.build()).join("\n");
        assert!(problems.contains("syntax name \"Duplicate\" is defined 2 times"));
        assert!(problems.contains("syntax scope \"source.one\" is shared by Duplicate, Third"));
    }

    #[test]
    fn permits_a_hidden_helper_before_its_visible_syntax() {
        let mut builder = SyntaxSetBuilder::new();
        for source in [
            "name: Shared\nscope: source.shared\nhidden: true\ncontexts: {main: []}",
            "name: Shared\nscope: source.shared\ncontexts: {main: []}",
        ] {
            builder.add(
                SyntaxDefinition::load_from_str(source, true, None)
                    .expect("the fixture syntax should load"),
            );
        }

        assert!(duplicate_syntaxes(&builder.build()).is_empty());
    }
}
