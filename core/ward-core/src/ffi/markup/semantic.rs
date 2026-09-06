// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use ward_markup::{Alignment, ContainerKind, NodeContent, SourceFormat, TextKind};

use super::wire;

pub(super) fn document_to_wire(document: ward_markup::SemanticDocument) -> wire::SemanticDocument {
    wire::SemanticDocument {
        source_format: match document.source_format {
            SourceFormat::PlainText => wire::SourceFormat::PlainText,
            SourceFormat::Markdown => wire::SourceFormat::Markdown,
            _ => wire::SourceFormat::Unspecified,
        } as i32,
        blocks: document
            .blocks
            .into_iter()
            .map(|block| wire::SemanticBlock {
                block_id: block.id,
                source: Some(source_range(block.source_range)),
                nodes: block.nodes.into_iter().map(node_to_wire).collect(),
            })
            .collect(),
    }
}

fn source_range(range: std::ops::Range<usize>) -> wire::SourceRange {
    wire::SourceRange {
        start: range.start as u64,
        end: range.end as u64,
    }
}

fn mapped_text(value: ward_markup::MappedText) -> wire::MappedText {
    wire::MappedText {
        text: value.text,
        mappings: value
            .mappings
            .into_iter()
            .map(|mapping| wire::TextMapping {
                source: Some(source_range(mapping.source_range)),
                utf16_start: mapping.utf16_range.start as u64,
                utf16_end: mapping.utf16_range.end as u64,
                verbatim: mapping.verbatim,
            })
            .collect(),
    }
}

fn node_to_wire(node: ward_markup::SemanticNode) -> wire::SemanticNode {
    use wire::semantic_node::Body;

    let body = match node.content {
        NodeContent::Container(kind) => Body::Container(match kind {
            ContainerKind::Paragraph => wire::ContainerKind::Paragraph,
            ContainerKind::BlockQuote => wire::ContainerKind::BlockQuote,
            ContainerKind::ListItem => wire::ContainerKind::ListItem,
            ContainerKind::TableCell => wire::ContainerKind::TableCell,
            ContainerKind::Emphasis => wire::ContainerKind::Emphasis,
            ContainerKind::Strong => wire::ContainerKind::Strong,
            ContainerKind::Strikethrough => wire::ContainerKind::Strikethrough,
        } as i32),
        NodeContent::Heading(level) => Body::HeadingLevel(level),
        NodeContent::List { start } => Body::List(wire::SemanticList { start }),
        NodeContent::Table { columns } => Body::Table(wire::SemanticTable {
            columns: columns
                .into_iter()
                .map(|column| match column {
                    Alignment::None => wire::ColumnAlignment::Unspecified,
                    Alignment::Left => wire::ColumnAlignment::Left,
                    Alignment::Center => wire::ColumnAlignment::Center,
                    Alignment::Right => wire::ColumnAlignment::Right,
                } as i32)
                .collect(),
        }),
        NodeContent::TableRow { header } => Body::TableRowHeader(header),
        NodeContent::CodeBlock { language } => {
            Body::CodeBlock(wire::SemanticCodeBlock { language })
        }
        NodeContent::Link { target, title } => Body::Link(wire::SemanticLink { target, title }),
        NodeContent::Image { target, title } => Body::Image(wire::SemanticLink { target, title }),
        NodeContent::Text { kind, value } => Body::Text(wire::SemanticText {
            kind: match kind {
                TextKind::Plain => wire::TextKind::Plain,
                TextKind::InlineCode => wire::TextKind::InlineCode,
                TextKind::SoftBreak => wire::TextKind::SoftBreak,
                TextKind::HardBreak => wire::TextKind::HardBreak,
                TextKind::Literal => wire::TextKind::Literal,
                TextKind::Unsupported => wire::TextKind::Unsupported,
            } as i32,
            value: Some(mapped_text(value)),
        }),
        NodeContent::Annotation { index, label } => Body::Annotation(wire::SemanticAnnotation {
            index,
            label: Some(mapped_text(label)),
        }),
        NodeContent::TaskMarker { checked } => Body::TaskChecked(checked),
        NodeContent::Rule => Body::Rule(true),
        NodeContent::FootnoteDefinition { label } => Body::FootnoteDefinition(label),
        NodeContent::FootnoteReference { label } => Body::FootnoteReference(label),
        NodeContent::Admonition { kind } => Body::AdmonitionKind(kind),
    };
    wire::SemanticNode {
        node_id: node.id,
        parent_index: node.parent.map(|index| {
            u32::try_from(index).expect("a semantic block must fit the wire node index")
        }),
        source: Some(source_range(node.source_range)),
        body: Some(body),
    }
}
