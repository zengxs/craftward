// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use anyhow::{Context, Result, bail, ensure};
use csscolorparser::Color;
use jsonc_parser::{ParseOptions, parse_to_serde_value};
use serde::{Deserialize, Serialize};

const LIGHT_THEME_FILE: &str = "One Light.tmTheme";
const DARK_THEME_FILE: &str = "One Dark.tmTheme";

const GROUPED_SUPPORT_TYPE_SELECTOR: &str =
    "support.type - (support.type.package, support.type.vendor-prefix.css)";
const CSS_PROPERTY_SELECTOR: &str = "(source.css, source.less, source.sass, source.scss) & (meta.property-name, meta.property-value)";
const CSS_PROPERTY_NAME_SELECTOR: &str =
    "(source.css, source.less, source.sass, source.scss) & support.type.property-name";
const CSS_CUSTOM_PROPERTY_SELECTOR: &str = "(source.css, source.less, source.sass, source.scss) & (punctuation.definition.custom-property, support.type.custom-property.name)";
const CSS_SOURCES: [&str; 4] = ["source.css", "source.less", "source.sass", "source.scss"];
const DIFF_CHARACTER_SCOPES: [&str; 2] = ["diff.deleted.char", "diff.inserted.char"];

pub struct ProjectPaths {
    light_source: PathBuf,
    dark_source: PathBuf,
    output_directory: PathBuf,
}

impl ProjectPaths {
    pub fn new(light_source: PathBuf, dark_source: PathBuf) -> Self {
        let xtask_directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repository_directory = xtask_directory
            .parent()
            .expect("xtask must be located directly under the repository root");

        Self {
            light_source,
            dark_source,
            output_directory: repository_directory.join("core/ward-highlighting/assets/Themes"),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct ImportSummary {
    pub changed_files: usize,
    pub omitted_foreground_adjustments: usize,
}

/// Converts two Sublime color schemes into the maintained TextMate theme assets.
///
/// The source files are read in place and are never copied into the repository.
pub fn import(paths: &ProjectPaths) -> Result<ImportSummary> {
    let light_source = read_source(&paths.light_source, ThemeVariant::Light)?;
    let dark_source = read_source(&paths.dark_source, ThemeVariant::Dark)?;

    // Render both inputs before changing either maintained asset.
    let light = convert_scheme(&light_source, ThemeVariant::Light).with_context(|| {
        format!(
            "failed to convert the light Sublime color scheme {}",
            paths.light_source.display()
        )
    })?;
    let dark = convert_scheme(&dark_source, ThemeVariant::Dark).with_context(|| {
        format!(
            "failed to convert the dark Sublime color scheme {}",
            paths.dark_source.display()
        )
    })?;

    fs::create_dir_all(&paths.output_directory).with_context(|| {
        format!(
            "failed to create the maintained theme directory {}",
            paths.output_directory.display()
        )
    })?;

    let mut changed_files = 0;
    changed_files += usize::from(write_if_changed(
        &paths.output_directory.join(LIGHT_THEME_FILE),
        &light.xml,
    )?);
    changed_files += usize::from(write_if_changed(
        &paths.output_directory.join(DARK_THEME_FILE),
        &dark.xml,
    )?);

    Ok(ImportSummary {
        changed_files,
        omitted_foreground_adjustments: light.omitted_foreground_adjustments
            + dark.omitted_foreground_adjustments,
    })
}

fn read_source(path: &Path, variant: ThemeVariant) -> Result<String> {
    fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read the {} Sublime color scheme {}",
            variant.label(),
            path.display()
        )
    })
}

#[derive(Clone, Copy)]
enum ThemeVariant {
    Light,
    Dark,
}

impl ThemeVariant {
    const fn label(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Light => "One Light",
            Self::Dark => "One Dark",
        }
    }

    const fn diff_foreground_adjustment(self) -> &'static str {
        match self {
            Self::Light => "l(-10%)",
            Self::Dark => "l(+10%)",
        }
    }
}

#[derive(Debug)]
struct ConvertedTheme {
    xml: Vec<u8>,
    omitted_foreground_adjustments: usize,
}

fn convert_scheme(source: &str, variant: ThemeVariant) -> Result<ConvertedTheme> {
    let parse_options = ParseOptions {
        allow_comments: true,
        allow_loose_object_property_names: false,
        allow_trailing_commas: true,
        allow_missing_commas: false,
        allow_single_quoted_strings: false,
        allow_hexadecimal_numbers: false,
        allow_unary_plus_numbers: false,
    };
    let scheme: SublimeColorScheme = parse_to_serde_value(source, &parse_options)
        .context("the source is not a supported Sublime JSON color scheme")?;

    if let Some(source_name) = scheme.name.as_deref() {
        ensure!(
            source_name == variant.name(),
            "the source theme name {source_name:?} does not match {:?}",
            variant.name()
        );
    }
    ensure!(!scheme.rules.is_empty(), "the source theme has no rules");

    let mut resolver = ColorResolver::new(&scheme.variables);
    let foreground = resolve_global(&scheme.globals, "foreground", &mut resolver)?;
    let background = resolve_global(&scheme.globals, "background", &mut resolver)?;

    let mut settings = Vec::with_capacity(scheme.rules.len() + 1);
    settings.push(TextMateThemeItem {
        name: None,
        scope: None,
        settings: TextMateStyle {
            foreground: Some(foreground),
            background: Some(background),
            font_style: None,
        },
    });

    let mut omitted_foreground_adjustments = 0;
    let mut adjusted_diff_character_scopes = BTreeSet::new();
    for (index, rule) in scheme.rules.into_iter().enumerate() {
        let rule_number = index + 1;
        let scope = lower_selector(&rule.scope)
            .with_context(|| format!("rule {rule_number} has an unsupported scope selector"))?;
        let foreground = rule
            .foreground
            .as_deref()
            .map(|expression| resolver.resolve_hex(expression))
            .transpose()
            .with_context(|| format!("rule {rule_number} has an invalid foreground"))?;
        let background = rule
            .background
            .as_deref()
            .map(|expression| resolver.resolve_hex(expression))
            .transpose()
            .with_context(|| format!("rule {rule_number} has an invalid background"))?;
        let font_style = rule
            .font_style
            .as_deref()
            .map(normalize_font_style)
            .transpose()
            .with_context(|| format!("rule {rule_number} has an invalid font_style"))?;

        if let Some(adjustment) = rule.foreground_adjust.as_deref() {
            validate_omitted_foreground_adjustment(
                variant,
                &rule.scope,
                adjustment,
                background.as_deref(),
            )
            .with_context(|| format!("rule {rule_number} has an unsupported foreground_adjust"))?;
            ensure!(
                adjusted_diff_character_scopes.insert(rule.scope.trim().to_owned()),
                "rule {rule_number} duplicates a reviewed foreground_adjust scope"
            );
            omitted_foreground_adjustments += 1;
        }

        ensure!(
            foreground.is_some() || background.is_some() || font_style.is_some(),
            "rule {rule_number} does not define a supported style"
        );

        settings.push(TextMateThemeItem {
            name: rule.name,
            scope: Some(scope),
            settings: TextMateStyle {
                foreground,
                background,
                font_style,
            },
        });
    }
    ensure!(
        adjusted_diff_character_scopes.len() == DIFF_CHARACTER_SCOPES.len()
            && DIFF_CHARACTER_SCOPES
                .iter()
                .all(|scope| adjusted_diff_character_scopes.contains(*scope)),
        "the {} theme must adjust each reviewed diff character scope exactly once",
        variant.label()
    );

    let theme = TextMateTheme {
        name: variant.name().to_owned(),
        author: scheme.author,
        settings,
    };
    let mut xml = Vec::new();
    plist::to_writer_xml(&mut xml, &theme).context("failed to serialize the TextMate theme")?;

    Ok(ConvertedTheme {
        xml,
        omitted_foreground_adjustments,
    })
}

fn resolve_global(
    globals: &BTreeMap<String, String>,
    name: &str,
    resolver: &mut ColorResolver<'_>,
) -> Result<String> {
    let expression = globals
        .get(name)
        .with_context(|| format!("the source theme has no {name:?} global"))?;
    resolver
        .resolve_hex(expression)
        .with_context(|| format!("the {name:?} global is invalid"))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SublimeColorScheme {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    author: Option<String>,
    #[serde(default)]
    variables: BTreeMap<String, String>,
    globals: BTreeMap<String, String>,
    rules: Vec<SublimeRule>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SublimeRule {
    #[serde(default)]
    name: Option<String>,
    scope: String,
    #[serde(default)]
    foreground: Option<String>,
    #[serde(default)]
    background: Option<String>,
    #[serde(default)]
    font_style: Option<String>,
    #[serde(default)]
    foreground_adjust: Option<String>,
}

#[derive(Serialize)]
struct TextMateTheme {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    settings: Vec<TextMateThemeItem>,
}

#[derive(Serialize)]
struct TextMateThemeItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
    settings: TextMateStyle,
}

#[derive(Serialize)]
struct TextMateStyle {
    #[serde(skip_serializing_if = "Option::is_none")]
    foreground: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    background: Option<String>,
    #[serde(rename = "fontStyle", skip_serializing_if = "Option::is_none")]
    font_style: Option<String>,
}

struct ColorResolver<'a> {
    variables: &'a BTreeMap<String, String>,
    resolving_variables: Vec<String>,
}

impl<'a> ColorResolver<'a> {
    fn new(variables: &'a BTreeMap<String, String>) -> Self {
        Self {
            variables,
            resolving_variables: Vec::new(),
        }
    }

    fn resolve_hex(&mut self, expression: &str) -> Result<String> {
        Ok(format_color(&self.resolve(expression)?))
    }

    fn resolve(&mut self, expression: &str) -> Result<Color> {
        let expression = expression.trim();
        ensure!(!expression.is_empty(), "the color expression is empty");

        if let Some(name) = function_body(expression, "var")? {
            return self.resolve_variable(name.trim());
        }
        if let Some(body) = function_body(expression, "color")? {
            return self.resolve_adjusted_color(body);
        }

        // Atomic CSS colors are delegated deliberately. Only Sublime-specific
        // variable and color-adjustment expressions require the adapter above.
        Color::from_str(expression)
            .with_context(|| format!("unsupported color expression {expression:?}"))
    }

    fn resolve_variable(&mut self, name: &str) -> Result<Color> {
        ensure!(!name.is_empty(), "the variable name is empty");
        if let Some(position) = self
            .resolving_variables
            .iter()
            .position(|active| active == name)
        {
            let mut cycle = self.resolving_variables[position..].to_vec();
            cycle.push(name.to_owned());
            bail!("color variable cycle: {}", cycle.join(" -> "));
        }

        let expression = self
            .variables
            .get(name)
            .cloned()
            .with_context(|| format!("unknown color variable {name:?}"))?;
        self.resolving_variables.push(name.to_owned());
        let result = self
            .resolve(&expression)
            .with_context(|| format!("failed to resolve color variable {name:?}"));
        self.resolving_variables.pop();
        result
    }

    fn resolve_adjusted_color(&mut self, body: &str) -> Result<Color> {
        let terms = split_top_level_terms(body)?;
        let (base, adjustments) = terms.split_first().context("color() has no base color")?;
        let mut color = self.resolve(base)?;

        for adjustment in adjustments {
            if let Some(value) = function_body(adjustment, "a")? {
                color.a = parse_alpha(value)?;
                continue;
            }
            if let Some(value) = function_body(adjustment, "l")? {
                let delta = parse_relative_percentage(value, "lightness")?;
                let [hue, saturation, lightness, alpha] = color.to_hsla();
                color =
                    Color::from_hsla(hue, saturation, (lightness + delta).clamp(0.0, 1.0), alpha);
                continue;
            }
            bail!("unsupported color adjuster {adjustment:?}");
        }

        Ok(color)
    }
}

fn function_body<'a>(expression: &'a str, name: &str) -> Result<Option<&'a str>> {
    let expression = expression.trim();
    let Some(rest) = expression.strip_prefix(name) else {
        return Ok(None);
    };
    let Some(rest) = rest.strip_prefix('(') else {
        return Ok(None);
    };

    let mut depth = 1usize;
    for (index, character) in rest.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    ensure!(
                        rest[index + 1..].trim().is_empty(),
                        "unexpected text after {name}()"
                    );
                    return Ok(Some(&rest[..index]));
                }
            }
            _ => {}
        }
    }

    bail!("unterminated {name}() expression")
}

fn split_top_level_terms(expression: &str) -> Result<Vec<&str>> {
    let mut terms = Vec::new();
    let mut start = None;
    let mut depth = 0usize;

    for (index, character) in expression.char_indices() {
        if character.is_whitespace() && depth == 0 {
            if let Some(term_start) = start.take() {
                terms.push(&expression[term_start..index]);
            }
            continue;
        }

        start.get_or_insert(index);
        match character {
            '(' => depth += 1,
            ')' => {
                ensure!(depth > 0, "unexpected ')' in color expression");
                depth -= 1;
            }
            _ => {}
        }
    }

    ensure!(depth == 0, "unterminated function in color expression");
    if let Some(term_start) = start {
        terms.push(&expression[term_start..]);
    }
    Ok(terms)
}

fn parse_alpha(value: &str) -> Result<f32> {
    let compact = compact_expression(value);
    let alpha = if let Some(percentage) = compact.strip_suffix('%') {
        percentage
            .parse::<f32>()
            .context("alpha percentage is not a number")?
            / 100.0
    } else {
        compact
            .parse::<f32>()
            .context("alpha value is not a number")?
    };
    ensure!(alpha.is_finite(), "alpha value is not finite");
    ensure!((0.0..=1.0).contains(&alpha), "alpha value is out of range");
    Ok(alpha)
}

fn parse_relative_percentage(value: &str, label: &str) -> Result<f32> {
    let compact = compact_expression(value);
    let signed = compact
        .strip_suffix('%')
        .with_context(|| format!("{label} adjustment is not a percentage"))?;
    ensure!(
        signed.starts_with('+') || signed.starts_with('-'),
        "{label} adjustment is not relative"
    );
    let percentage = signed
        .parse::<f32>()
        .with_context(|| format!("{label} adjustment is not a number"))?;
    ensure!(percentage.is_finite(), "{label} adjustment is not finite");
    ensure!(
        (-100.0..=100.0).contains(&percentage),
        "{label} adjustment is out of range"
    );
    Ok(percentage / 100.0)
}

fn compact_expression(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect()
}

fn format_color(color: &Color) -> String {
    let [red, green, blue, alpha] = color.to_rgba8();
    if alpha == u8::MAX {
        format!("#{red:02X}{green:02X}{blue:02X}")
    } else {
        format!("#{red:02X}{green:02X}{blue:02X}{alpha:02X}")
    }
}

fn normalize_font_style(source: &str) -> Result<String> {
    let mut bold = false;
    let mut italic = false;
    let mut underline = false;

    for part in source.split_whitespace() {
        match part {
            "bold" => bold = true,
            "italic" => italic = true,
            "underline" => underline = true,
            _ => bail!("unsupported font style {part:?}"),
        }
    }

    let mut parts = Vec::new();
    if bold {
        parts.push("bold");
    }
    if italic {
        parts.push("italic");
    }
    if underline {
        parts.push("underline");
    }
    Ok(parts.join(" "))
}

fn validate_omitted_foreground_adjustment(
    variant: ThemeVariant,
    scope: &str,
    adjustment: &str,
    background: Option<&str>,
) -> Result<()> {
    ensure!(
        DIFF_CHARACTER_SCOPES.contains(&scope.trim()),
        "{scope:?} is not an allowlisted diff character scope"
    );
    ensure!(
        background.is_some(),
        "foreground_adjust requires a background"
    );
    let compact = compact_expression(adjustment);
    ensure!(
        compact == variant.diff_foreground_adjustment(),
        "the adjustment does not match the maintained {} variant",
        variant.label()
    );
    Ok(())
}

fn lower_selector(source: &str) -> Result<String> {
    let source = source.trim();
    ensure!(!source.is_empty(), "the scope selector is empty");

    let lowered = match source {
        GROUPED_SUPPORT_TYPE_SELECTOR => {
            "support.type - support.type.package - support.type.vendor-prefix.css".to_owned()
        }
        CSS_PROPERTY_SELECTOR => {
            cartesian_selectors(&CSS_SOURCES, &["meta.property-name", "meta.property-value"])
        }
        CSS_PROPERTY_NAME_SELECTOR => {
            cartesian_selectors(&CSS_SOURCES, &["support.type.property-name"])
        }
        CSS_CUSTOM_PROPERTY_SELECTOR => cartesian_selectors(
            &CSS_SOURCES,
            &[
                "punctuation.definition.custom-property",
                "support.type.custom-property.name",
            ],
        ),
        _ => source.to_owned(),
    };

    ensure!(
        !lowered.contains(['&', '(', ')']),
        "unsupported grouping or conjunction in {source:?}"
    );
    Ok(lowered)
}

fn cartesian_selectors(left: &[&str], right: &[&str]) -> String {
    left.iter()
        .flat_map(|left| right.iter().map(move |right| format!("{left} {right}")))
        .collect::<Vec<_>>()
        .join(", ")
}

fn write_if_changed(path: &Path, contents: &[u8]) -> Result<bool> {
    if fs::read(path).is_ok_and(|existing| existing == contents) {
        return Ok(false);
    }

    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use serde_json::Value;

    use super::*;

    const LOWERED_CSS_PROPERTY_SELECTOR: &str = "source.css meta.property-name, source.css meta.property-value, source.less meta.property-name, source.less meta.property-value, source.sass meta.property-name, source.sass meta.property-value, source.scss meta.property-name, source.scss meta.property-value";
    const LOWERED_CSS_PROPERTY_NAME_SELECTOR: &str = "source.css support.type.property-name, source.less support.type.property-name, source.sass support.type.property-name, source.scss support.type.property-name";
    const LOWERED_CSS_CUSTOM_PROPERTY_SELECTOR: &str = "source.css punctuation.definition.custom-property, source.css support.type.custom-property.name, source.less punctuation.definition.custom-property, source.less support.type.custom-property.name, source.sass punctuation.definition.custom-property, source.sass support.type.custom-property.name, source.scss punctuation.definition.custom-property, source.scss support.type.custom-property.name";

    const TEST_SCHEME: &str = r#"
{
  "variables": {
    "foreground": "hsl(230, 8%, 24%)",
    "accent": "var(foreground)",
  },
  "globals": {
    "foreground": "var(foreground)",
    "background": "hsl(230, 1%, 98%)",
    "ignored_ui_color": "color(var(foreground) a(0.5)",
  },
  "rules": [
    {
      "name": "Grouped exclusion",
      "scope": "support.type - (support.type.package, support.type.vendor-prefix.css)",
      "foreground": "color(var(accent) l(+ 10%))",
      "font_style": "underline bold",
    },
    {
      "scope": "(source.css, source.less, source.sass, source.scss) & (meta.property-name, meta.property-value)",
      "foreground": "var(accent)",
    },
    {
      "scope": "(source.css, source.less, source.sass, source.scss) & support.type.property-name",
      "foreground": "var(accent)",
    },
    {
      "scope": "(source.css, source.less, source.sass, source.scss) & (punctuation.definition.custom-property, support.type.custom-property.name)",
      "foreground": "var(accent)",
    },
    {
      "scope": "diff.deleted.char",
      "background": "rgba(255, 0, 0, 0.5)",
      "foreground_adjust": "l(- 10%)",
    },
    {
      "scope": "diff.inserted.char",
      "background": "rgba(0, 255, 0, 0.25)",
      "foreground_adjust": "l(- 10%)",
    },
  ],
}
"#;

    static NEXT_TEST_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let repository_directory = Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .expect("xtask must be located directly under the repository root");
            let path = repository_directory
                .join(".tmp/xtask-theme-tests")
                .join(format!("{}-{sequence}", std::process::id()));
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
    fn reports_color_variable_cycles() {
        let source = TEST_SCHEME.replace(
            "\"foreground\": \"hsl(230, 8%, 24%)\"",
            "\"foreground\": \"var(accent)\"",
        );
        let error = convert_scheme(&source, ThemeVariant::Light).unwrap_err();
        assert!(format!("{error:#}").contains("color variable cycle"));
    }

    #[test]
    fn rejects_unrecognized_selector_expressions() {
        let error = lower_selector("source.rust & meta.function").unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unsupported grouping or conjunction")
        );
    }

    #[test]
    fn rejects_unreviewed_foreground_adjustment_occurrences() {
        let source = TEST_SCHEME.replace("diff.deleted.char", "diff.modified.char");
        let error = convert_scheme(&source, ThemeVariant::Light).unwrap_err();

        assert!(format!("{error:#}").contains("not an allowlisted diff character scope"));
    }

    #[test]
    fn rejects_missing_reviewed_foreground_adjustment_occurrences() {
        let source = TEST_SCHEME.replacen("      \"foreground_adjust\": \"l(- 10%)\",\n", "", 1);
        let error = convert_scheme(&source, ThemeVariant::Light).unwrap_err();

        assert!(format!("{error:#}").contains("must adjust each reviewed diff character scope"));
    }

    #[test]
    fn rejects_foreground_adjustments_for_the_wrong_theme_variant() {
        let error = convert_scheme(TEST_SCHEME, ThemeVariant::Dark).unwrap_err();

        assert!(format!("{error:#}").contains("does not match the maintained dark variant"));
    }

    #[test]
    fn imports_both_variants_without_copying_the_sources() {
        let directory = TestDirectory::new();
        let light_source = directory.0.join("light.sublime-color-scheme");
        let dark_source = directory.0.join("dark.sublime-color-scheme");
        let dark_scheme = TEST_SCHEME.replace("l(- 10%)", "l(+ 10%)");
        fs::write(&light_source, TEST_SCHEME).unwrap();
        fs::write(&dark_source, &dark_scheme).unwrap();
        let paths = ProjectPaths {
            light_source: light_source.clone(),
            dark_source: dark_source.clone(),
            output_directory: directory.0.join("output"),
        };

        let first = import(&paths).unwrap();
        assert_eq!(first.changed_files, 2);
        assert_eq!(first.omitted_foreground_adjustments, 4);
        let light_theme_path = paths.output_directory.join(LIGHT_THEME_FILE);
        assert!(light_theme_path.is_file());
        assert!(paths.output_directory.join(DARK_THEME_FILE).is_file());
        assert_eq!(fs::read_to_string(light_source).unwrap(), TEST_SCHEME);
        assert_eq!(fs::read_to_string(dark_source).unwrap(), dark_scheme);

        let theme: Value = plist::from_file(light_theme_path).unwrap();
        assert_eq!(theme["name"], "One Light");
        let settings = theme["settings"].as_array().unwrap();
        assert_eq!(settings.len(), 7);
        assert_eq!(settings[0]["settings"]["foreground"], "#383A42");
        assert_eq!(settings[0]["settings"]["background"], "#FAFAFA");
        assert_eq!(
            settings[1]["scope"],
            "support.type - support.type.package - support.type.vendor-prefix.css"
        );
        assert_eq!(settings[1]["settings"]["fontStyle"], "bold underline");
        assert_eq!(settings[2]["scope"], LOWERED_CSS_PROPERTY_SELECTOR);
        assert_eq!(settings[3]["scope"], LOWERED_CSS_PROPERTY_NAME_SELECTOR);
        assert_eq!(settings[4]["scope"], LOWERED_CSS_CUSTOM_PROPERTY_SELECTOR);
        assert_eq!(settings[5]["settings"]["background"], "#FF000080");
        assert_eq!(settings[6]["settings"]["background"], "#00FF0040");

        let second = import(&paths).unwrap();
        assert_eq!(second.changed_files, 0);
    }
}
