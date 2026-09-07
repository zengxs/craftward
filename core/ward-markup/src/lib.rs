// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use pulldown_cmark::{CodeBlockKind, Options};

mod semantic;
pub use semantic::{
    Alignment, ContainerKind, MappedText, NodeContent, SemanticBlock, SemanticDocument,
    SemanticNode, TextKind, TextMapping, parse_semantic,
};

/// The source syntax interpreted by the markup parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SourceFormat {
    PlainText,
    Markdown,
}

fn code_block_language(kind: CodeBlockKind<'_>) -> Option<String> {
    let CodeBlockKind::Fenced(info) = kind else {
        return None;
    };
    info.split_whitespace()
        .next()
        .filter(|language| !language.is_empty())
        .map(str::to_owned)
}

fn markdown_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM
}
