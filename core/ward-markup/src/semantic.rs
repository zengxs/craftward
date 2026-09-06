// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{collections::HashMap, ops::Range};

use pulldown_cmark::{Alignment as MarkdownAlignment, BlockQuoteKind, Event, Parser, Tag};

use crate::{SourceFormat, code_block_language, markdown_options};

/// A complete source snapshot with document-wide reference resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticDocument {
    pub source_format: SourceFormat,
    pub blocks: Vec<SemanticBlock>,
}

/// One top-level Markdown structure, independent of later rendering segments.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticBlock {
    pub id: String,
    pub source_range: Range<usize>,
    /// Nodes in preorder; parents precede children. Indices are snapshot-local.
    pub nodes: Vec<SemanticNode>,
}

/// A semantic identity and its source provenance, without any layout objects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticNode {
    /// Scoped to the owning block. See docs/markup-semantics.md for stability rules.
    pub id: String,
    pub parent: Option<usize>,
    /// Half-open UTF-8 byte positions in the complete input source.
    pub source_range: Range<usize>,
    pub content: NodeContent,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContainerKind {
    Paragraph,
    BlockQuote,
    ListItem,
    TableCell,
    Emphasis,
    Strong,
    Strikethrough,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TextKind {
    Plain,
    InlineCode,
    SoftBreak,
    HardBreak,
    /// Raw HTML is preserved as literal text, never executable markup.
    Literal,
    /// A syntax outside this contract; consumers must fall back for its block.
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Alignment {
    None,
    Left,
    Center,
    Right,
}

/// Only the active variant's metadata applies to this node.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NodeContent {
    Container(ContainerKind),
    Heading(u32),
    List { start: Option<u64> },
    Table { columns: Vec<Alignment> },
    TableRow { header: bool },
    CodeBlock { language: Option<String> },
    Link { target: String, title: String },
    Image { target: String, title: String },
    Text { kind: TextKind, value: MappedText },
    Annotation { index: u32, label: MappedText },
    TaskMarker { checked: bool },
    Rule,
    FootnoteDefinition { label: String },
    FootnoteReference { label: String },
    Admonition { kind: String },
}

impl NodeContent {
    fn identity_kind(&self) -> &'static str {
        match self {
            Self::Container(kind) => match kind {
                ContainerKind::Paragraph => "paragraph",
                ContainerKind::BlockQuote => "quote",
                ContainerKind::ListItem => "item",
                ContainerKind::TableCell => "cell",
                ContainerKind::Emphasis => "emphasis",
                ContainerKind::Strong => "strong",
                ContainerKind::Strikethrough => "strike",
            },
            Self::Heading(_) => "heading",
            Self::List { .. } => "list",
            Self::Table { .. } => "table",
            Self::TableRow { .. } => "row",
            Self::CodeBlock { .. } => "code-block",
            Self::Link { .. } => "link",
            Self::Image { .. } => "image",
            Self::Text { kind, .. } => match kind {
                TextKind::Plain => "text",
                TextKind::InlineCode => "code",
                TextKind::SoftBreak => "soft-break",
                TextKind::HardBreak => "hard-break",
                TextKind::Literal => "literal",
                TextKind::Unsupported => "unsupported",
            },
            Self::Annotation { .. } => "annotation",
            Self::TaskMarker { .. } => "task",
            Self::Rule => "rule",
            Self::FootnoteDefinition { .. } => "footnote-definition",
            Self::FootnoteReference { .. } => "footnote-reference",
            Self::Admonition { .. } => "admonition",
        }
    }
}

/// Decoded text with explicit UTF-8 source / UTF-16 text mapping pieces.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MappedText {
    pub text: String,
    pub mappings: Vec<TextMapping>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TextMapping {
    pub source_range: Range<usize>,
    /// Half-open UTF-16 code-unit positions in MappedText::text, not the document.
    pub utf16_range: Range<usize>,
    /// The source slice equals the decoded text slice. Otherwise this is an
    /// indivisible replacement: interior source cursor positions are not implied.
    pub verbatim: bool,
}

impl MappedText {
    fn new(source: &str, source_range: Range<usize>, text: String) -> Self {
        let verbatim = source[source_range.clone()] == text;
        let utf16_length = text.encode_utf16().count();
        Self {
            text,
            mappings: vec![TextMapping {
                source_range,
                utf16_range: 0..utf16_length,
                verbatim,
            }],
        }
    }
}

/// Parses the complete message snapshot, including inline structure and Codex
/// annotations. This does not replace the legacy bounded-block parse interface.
/// Unchanged node kinds and starts retain IDs on append; syntax reinterpretation
/// can replace nodes. Arbitrary edits require caller-owned reconciliation.
pub fn parse_semantic(source: &str, format: SourceFormat) -> SemanticDocument {
    let mut builder = Builder::new(source);
    if format == SourceFormat::PlainText {
        if !source.is_empty() {
            builder.push(
                NodeContent::Container(ContainerKind::Paragraph),
                0..source.len(),
            );
            builder.parents.push(0);
            builder.text(TextKind::Plain, source, 0..source.len());
            builder.parents.pop();
            builder.finish_block();
        }
    } else {
        let mut opaque_depth = 0;
        for (event, range) in Parser::new_ext(source, markdown_options()).into_offset_iter() {
            if opaque_depth > 0 {
                match event {
                    Event::Start(_) => opaque_depth += 1,
                    Event::End(_) => opaque_depth -= 1,
                    _ => {}
                }
                if opaque_depth == 0 && builder.parents.is_empty() {
                    builder.finish_block();
                }
                continue;
            }
            match event {
                Event::Start(tag) => {
                    let content = container(tag);
                    if let Some(content) = content {
                        let index = builder.push(content, range);
                        builder.parents.push(index);
                    } else {
                        builder.text(TextKind::Unsupported, &source[range.clone()], range);
                        opaque_depth = 1;
                    }
                }
                Event::End(_) => {
                    builder.parents.pop();
                    if builder.parents.is_empty() {
                        builder.finish_block();
                    }
                }
                Event::Text(text) => {
                    let inert = builder.parents.iter().any(|&index| {
                        matches!(
                            builder.nodes[index].content,
                            NodeContent::CodeBlock { .. }
                                | NodeContent::Link { .. }
                                | NodeContent::Image { .. }
                        )
                    });
                    if inert || source[range.clone()] != *text {
                        builder.text(TextKind::Plain, &text, range);
                    } else {
                        builder.inline_text(&text, range);
                    }
                }
                Event::Code(text) => builder.text(TextKind::InlineCode, &text, range),
                Event::SoftBreak => builder.text(TextKind::SoftBreak, "\n", range),
                Event::HardBreak => builder.text(TextKind::HardBreak, "\n", range),
                Event::Html(text) | Event::InlineHtml(text) => {
                    builder.text(TextKind::Literal, &text, range)
                }
                Event::FootnoteReference(label) => {
                    builder.push(
                        NodeContent::FootnoteReference {
                            label: label.into_string(),
                        },
                        range,
                    );
                }
                Event::TaskListMarker(checked) => {
                    builder.push(NodeContent::TaskMarker { checked }, range);
                }
                Event::Rule => {
                    builder.push(NodeContent::Rule, range);
                }
                Event::InlineMath(_) | Event::DisplayMath(_) => {
                    builder.text(TextKind::Unsupported, &source[range.clone()], range)
                }
            }
            if builder.parents.is_empty() && opaque_depth == 0 {
                builder.finish_block();
            }
        }
    }
    SemanticDocument {
        source_format: format,
        blocks: builder.blocks,
    }
}

fn container(tag: Tag<'_>) -> Option<NodeContent> {
    Some(match tag {
        Tag::Paragraph => NodeContent::Container(ContainerKind::Paragraph),
        Tag::Heading { level, .. } => NodeContent::Heading(level as u32),
        Tag::BlockQuote(None) => NodeContent::Container(ContainerKind::BlockQuote),
        Tag::BlockQuote(Some(kind)) => NodeContent::Admonition {
            kind: match kind {
                BlockQuoteKind::Note => "note",
                BlockQuoteKind::Tip => "tip",
                BlockQuoteKind::Important => "important",
                BlockQuoteKind::Warning => "warning",
                BlockQuoteKind::Caution => "caution",
            }
            .to_owned(),
        },
        Tag::List(start) => NodeContent::List { start },
        Tag::Item => NodeContent::Container(ContainerKind::ListItem),
        Tag::Table(columns) => NodeContent::Table {
            columns: columns
                .into_iter()
                .map(|column| match column {
                    MarkdownAlignment::None => Alignment::None,
                    MarkdownAlignment::Left => Alignment::Left,
                    MarkdownAlignment::Center => Alignment::Center,
                    MarkdownAlignment::Right => Alignment::Right,
                })
                .collect(),
        },
        Tag::TableHead => NodeContent::TableRow { header: true },
        Tag::TableRow => NodeContent::TableRow { header: false },
        Tag::TableCell => NodeContent::Container(ContainerKind::TableCell),
        Tag::Emphasis => NodeContent::Container(ContainerKind::Emphasis),
        Tag::Strong => NodeContent::Container(ContainerKind::Strong),
        Tag::Strikethrough => NodeContent::Container(ContainerKind::Strikethrough),
        Tag::CodeBlock(kind) => NodeContent::CodeBlock {
            language: code_block_language(kind),
        },
        Tag::Link {
            dest_url, title, ..
        } => NodeContent::Link {
            target: dest_url.into_string(),
            title: title.into_string(),
        },
        Tag::Image {
            dest_url, title, ..
        } => NodeContent::Image {
            target: dest_url.into_string(),
            title: title.into_string(),
        },
        Tag::FootnoteDefinition(label) => NodeContent::FootnoteDefinition {
            label: label.into_string(),
        },
        // Preserve unsupported block syntax as one opaque source slice.
        _ => return None,
    })
}

struct Builder<'a> {
    source: &'a str,
    blocks: Vec<SemanticBlock>,
    nodes: Vec<SemanticNode>,
    parents: Vec<usize>,
    identities: HashMap<String, usize>,
}

impl<'a> Builder<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            blocks: Vec::new(),
            nodes: Vec::new(),
            parents: Vec::new(),
            identities: HashMap::new(),
        }
    }

    fn push(&mut self, content: NodeContent, source_range: Range<usize>) -> usize {
        let index = self.nodes.len();
        self.nodes.push(SemanticNode {
            id: String::new(),
            parent: self.parents.last().copied(),
            source_range,
            content,
        });
        index
    }

    fn text(&mut self, kind: TextKind, text: &str, range: Range<usize>) {
        if text.is_empty() {
            return;
        }
        let value = MappedText::new(self.source, range.clone(), text.to_owned());
        self.push(NodeContent::Text { kind, value }, range);
    }

    fn inline_text(&mut self, text: &str, range: Range<usize>) {
        let mut copied = 0;
        for (start, _) in text.match_indices(":codex-annotation{") {
            let absolute = range.start + start;
            let escapes = self.source[..absolute]
                .bytes()
                .rev()
                .take_while(|&byte| byte == b'\\')
                .count();
            if escapes % 2 == 1 {
                continue;
            }
            let Some((length, index)) = annotation(&text[start..]) else {
                continue;
            };
            if start < copied {
                continue;
            }
            self.text(
                TextKind::Plain,
                &text[copied..start],
                range.start + copied..absolute,
            );
            let source_range = absolute..absolute + length;
            let label = MappedText::new(self.source, source_range.clone(), format!("[{index}]"));
            self.push(NodeContent::Annotation { index, label }, source_range);
            copied = start + length;
        }
        self.text(
            TextKind::Plain,
            &text[copied..],
            range.start + copied..range.end,
        );
    }

    fn finish_block(&mut self) {
        if self.nodes.is_empty() {
            return;
        }
        // Some Markdown events (notably task markers in loose lists) precede
        // their parent's reported range. Normalize provenance before assigning IDs.
        for index in (0..self.nodes.len()).rev() {
            if let Some(parent) = self.nodes[index].parent {
                let child = self.nodes[index].source_range.clone();
                let range = &mut self.nodes[parent].source_range;
                range.start = range.start.min(child.start);
                range.end = range.end.max(child.end);
            }
        }
        let block_start = self.nodes[0].source_range.start;
        for node in &mut self.nodes {
            let base = format!(
                "{}:{}",
                node.content.identity_kind(),
                node.source_range.start - block_start
            );
            let occurrence = self.identities.entry(base.clone()).or_default();
            node.id = if *occurrence == 0 {
                base
            } else {
                format!("{base}:{}", *occurrence)
            };
            *occurrence += 1;
        }
        let root = &self.nodes[0];
        let id = format!(
            "{}:{}",
            root.content.identity_kind(),
            root.source_range.start
        );
        self.blocks.push(SemanticBlock {
            id,
            source_range: root.source_range.clone(),
            nodes: std::mem::take(&mut self.nodes),
        });
        self.identities.clear();
    }
}

// Only this known directive is semantic. Unknown attributes or partial syntax
// remain ordinary text; code, links, images, and raw HTML never call this parser.
fn annotation(source: &str) -> Option<(usize, u32)> {
    let rest = source
        .strip_prefix(":codex-annotation{")?
        .trim_start_matches([' ', '\t']);
    let rest = rest.strip_prefix("index")?.trim_start_matches([' ', '\t']);
    let rest = rest.strip_prefix('=')?.trim_start_matches([' ', '\t']);
    let rest = rest.strip_prefix('"')?;
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    let index: u32 = rest[..digits].parse().ok()?;
    if index == 0 {
        return None;
    }
    let rest = rest[digits..]
        .strip_prefix('"')?
        .trim_start_matches([' ', '\t']);
    let rest = rest.strip_prefix('}')?;
    Some((source.len() - rest.len(), index))
}
