// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ops::Range;

use syntect::dumps::{from_reader, from_uncompressed_data};
use syntect::easy::HighlightLines;
use syntect::highlighting::{FontStyle, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;
use thiserror::Error;

mod theme;

pub use theme::Theme;

const PLAIN_TEXT_NAME: &str = "Plain Text";
const SYNTAX_PACK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/syntaxes.packdump"));
const THEME_PACK: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/themes.packdump"));

/// One eight-bit RGBA color resolved from a theme.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

/// The visual style shared by all rendering adapters.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Style {
    pub foreground: Color,
    pub background: Color,
    pub bold: bool,
    pub italic: bool,
    pub underline: bool,
}

/// One styled UTF-8 byte range in the original source.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Span {
    pub range: Range<usize>,
    pub style: Style,
}

/// A source snapshot highlighted independently of any rendering toolkit.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HighlightedCode {
    pub syntax_name: String,
    pub spans: Vec<Span>,
    pub language_recognized: bool,
}

/// Failures while loading embedded packs or highlighting source text.
#[derive(Debug, Error)]
pub enum Error {
    #[error("the embedded {name} pack is invalid: {message}")]
    InvalidPack { name: &'static str, message: String },
    #[error("the maintained theme {0:?} is unavailable")]
    MissingTheme(String),
    #[error("syntax highlighting failed: {0}")]
    Highlight(String),
}

/// An immutable highlighter built from application-maintained embedded packs.
///
/// This module never calls Syntect's default syntax or theme loaders. Rendering
/// adapters only consume renderer-independent spans and do not know how the
/// maintained resources are collected or compiled.
pub struct Highlighter {
    syntaxes: SyntaxSet,
    themes: ThemeSet,
}

impl Highlighter {
    /// Loads the packs generated from the maintained assets at build time.
    pub fn new() -> Result<Self, Error> {
        let syntaxes: SyntaxSet =
            from_uncompressed_data(SYNTAX_PACK).map_err(|error| Error::InvalidPack {
                name: "syntax",
                message: error.to_string(),
            })?;
        let themes: ThemeSet = from_reader(THEME_PACK).map_err(|error| Error::InvalidPack {
            name: "theme",
            message: error.to_string(),
        })?;
        for required in Theme::ALL {
            if !themes.themes.contains_key(required.name()) {
                return Err(Error::MissingTheme(required.name().to_owned()));
            }
        }

        Ok(Self { syntaxes, themes })
    }

    /// Highlights one complete UTF-8 source snapshot.
    ///
    /// Returned ranges always index `source` in UTF-8 bytes. A language token
    /// selects a maintained syntax by extension or name. Unknown and absent
    /// tokens use plain text without running the Syntect parser.
    pub fn highlight(
        &self,
        source: &str,
        language: Option<&str>,
        theme: Theme,
    ) -> Result<HighlightedCode, Error> {
        let (syntax, language_recognized) = self.syntax_for(language);
        let Some(syntax) = syntax else {
            return Ok(HighlightedCode {
                syntax_name: PLAIN_TEXT_NAME.to_owned(),
                spans: Vec::new(),
                language_recognized,
            });
        };
        let theme_name = theme.name();
        let theme = self
            .themes
            .themes
            .get(theme_name)
            .ok_or_else(|| Error::MissingTheme(theme_name.to_owned()))?;
        let mut line_highlighter = HighlightLines::new(syntax, theme);
        let mut spans = Vec::new();
        let mut source_offset = 0usize;

        for line in source_lines(source) {
            let regions = line_highlighter
                .highlight_line(line, &self.syntaxes)
                .map_err(|error| Error::Highlight(error.to_string()))?;
            for (style, region) in regions {
                if region.is_empty() {
                    continue;
                }
                let end = source_offset + region.len();
                push_span(
                    &mut spans,
                    source_offset..end,
                    Style {
                        foreground: color(style.foreground),
                        background: color(style.background),
                        bold: style.font_style.contains(FontStyle::BOLD),
                        italic: style.font_style.contains(FontStyle::ITALIC),
                        underline: style.font_style.contains(FontStyle::UNDERLINE),
                    },
                );
                source_offset = end;
            }
        }

        debug_assert_eq!(source_offset, source.len());
        Ok(HighlightedCode {
            syntax_name: syntax.name.clone(),
            spans,
            language_recognized,
        })
    }

    fn syntax_for(&self, language: Option<&str>) -> (Option<&SyntaxReference>, bool) {
        let Some(language) = language
            .map(str::trim)
            .filter(|language| !language.is_empty())
        else {
            return (None, false);
        };
        if is_plain_text_language(language) {
            return (None, true);
        }
        let normalized = normalize_language(language);
        let syntax = self
            .syntaxes
            .find_syntax_by_token(normalized)
            .or_else(|| self.syntaxes.find_syntax_by_token(language));
        match syntax {
            Some(syntax) => (Some(syntax), true),
            None => (None, false),
        }
    }
}

fn source_lines(source: &str) -> LinesWithEndings<'_> {
    LinesWithEndings::from(source)
}

fn is_plain_text_language(language: &str) -> bool {
    [
        "plain",
        "plain text",
        "plaintext",
        "plain_text",
        "text",
        "txt",
    ]
    .iter()
    .any(|token| language.eq_ignore_ascii_case(token))
}

fn normalize_language(language: &str) -> &str {
    match language.to_ascii_lowercase().as_str() {
        "c++" => "cpp",
        "c#" => "cs",
        "javascript" => "js",
        "python" => "py",
        "regex" | "regexp" => "re",
        "shell" | "shellscript" => "sh",
        "typescript" => "ts",
        _ => language,
    }
}

fn color(color: syntect::highlighting::Color) -> Color {
    Color {
        red: color.r,
        green: color.g,
        blue: color.b,
        alpha: color.a,
    }
}

fn push_span(spans: &mut Vec<Span>, range: Range<usize>, style: Style) {
    if let Some(previous) = spans.last_mut()
        && previous.range.end == range.start
        && previous.style == style
    {
        previous.range.end = range.end;
        return;
    }
    spans.push(Span { range, style });
}

#[cfg(test)]
mod tests {
    use super::{Highlighter, Theme};

    fn highlighter() -> Highlighter {
        Highlighter::new().expect("the embedded packs should load")
    }

    #[test]
    fn highlights_with_application_supplied_resources() {
        let source = "let value = 42;\n";
        let highlighted = highlighter()
            .highlight(source, Some("rust"), Theme::Light)
            .expect("the source should highlight");

        assert_eq!(highlighted.syntax_name, "Rust");
        assert!(highlighted.language_recognized);
        assert_eq!(
            highlighted.spans.first().expect("a keyword span").range,
            0..3
        );
        assert_eq!(
            highlighted.spans.last().expect("a final span").range.end,
            source.len()
        );
    }

    #[test]
    fn applies_the_sublime_one_yaml_mapping_key_rule() {
        let highlighted = highlighter()
            .highlight("name: Check dist/\n", Some("yaml"), Theme::Light)
            .expect("the YAML source should highlight");
        let key = highlighted
            .spans
            .iter()
            .find(|span| span.range.contains(&1))
            .expect("the mapping key should have a style");
        let value = highlighted
            .spans
            .iter()
            .find(|span| span.range.contains(&7))
            .expect("the mapping value should have a style");

        assert_ne!(key.style.foreground, value.style.foreground);
    }

    #[test]
    fn uses_plain_text_without_a_language_token() {
        let highlighted = highlighter()
            .highlight("let value", None, Theme::Light)
            .expect("plain text should highlight");

        assert_eq!(highlighted.syntax_name, "Plain Text");
        assert!(highlighted.spans.is_empty());
        assert!(!highlighted.language_recognized);
    }

    #[test]
    fn uses_plain_text_for_an_unknown_language_token() {
        let highlighted = highlighter()
            .highlight("fn answer() {}", Some("unknown-language"), Theme::Light)
            .expect("plain text should highlight");

        assert_eq!(highlighted.syntax_name, "Plain Text");
        assert!(highlighted.spans.is_empty());
        assert!(!highlighted.language_recognized);
    }

    #[test]
    fn recognizes_explicit_plain_text_tokens_without_parsing() {
        for token in [
            "plain",
            "plain text",
            "text",
            "txt",
            "plaintext",
            "plain_text",
        ] {
            let highlighted = highlighter()
                .highlight("let value", Some(token), Theme::Light)
                .expect("plain text should highlight");

            assert_eq!(highlighted.syntax_name, "Plain Text");
            assert!(highlighted.spans.is_empty());
            assert!(highlighted.language_recognized);
        }
    }

    #[test]
    fn keeps_project_plain_text_available_for_embedded_syntax_fallbacks() {
        assert!(
            highlighter()
                .syntaxes
                .find_syntax_by_name("Plain Text")
                .is_some()
        );
    }

    #[test]
    fn resolves_regular_expression_aliases() {
        for token in ["regex", "regexp", "re"] {
            let highlighted = highlighter()
                .highlight(r"^(?<word>\w+)$", Some(token), Theme::Light)
                .expect("the regular expression should highlight");

            assert_eq!(highlighted.syntax_name, "Regular Expression");
            assert!(!highlighted.spans.is_empty());
            assert!(highlighted.language_recognized);
        }
    }

    #[test]
    fn loads_custom_package_syntaxes() {
        let highlighter = highlighter();
        for (token, expected) in [
            ("qml", "QML"),
            ("ini", "INI"),
            ("swift", "Swift"),
            ("zig", "Zig"),
            ("proto", "Protocol Buffer"),
            ("cmake", "CMake"),
        ] {
            let highlighted = highlighter
                .highlight("value", Some(token), Theme::Light)
                .expect("the maintained syntax should highlight");
            assert_eq!(highlighted.syntax_name, expected);
            assert!(highlighted.language_recognized);
        }
    }

    #[test]
    fn highlighter_can_be_shared_across_rendering_adapters() {
        fn assert_send_and_sync<T: Send + Sync>() {}

        assert_send_and_sync::<Highlighter>();
    }
}
