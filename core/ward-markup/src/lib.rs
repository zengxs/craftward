// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};

mod semantic;
pub use semantic::{
    Alignment, ContainerKind, MappedText, NodeContent, SemanticBlock, SemanticDocument,
    SemanticNode, TextKind, TextMapping, parse_semantic,
};

const MAX_PROSE_TOP_LEVEL_BLOCKS: usize = 4;
const TARGET_PROSE_SOURCE_BYTES: usize = 8 * 1024;

/// The source syntax interpreted by the markup parser.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum SourceFormat {
    PlainText,
    Markdown,
}

/// A parsed document represented as independently renderable blocks.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Document {
    pub source_format: SourceFormat,
    pub blocks: Vec<Block>,
}

/// One independently renderable block in a parsed document.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Block {
    Prose(ProseBlock),
    Code(CodeBlock),
}

impl Block {
    /// Returns the identifier that remains stable while content after this block grows.
    pub fn id(&self) -> &str {
        match self {
            Self::Prose(block) => &block.id,
            Self::Code(block) => &block.id,
        }
    }

    /// Returns the UTF-8 byte range occupied by this block in the source.
    pub fn source_range(&self) -> &Range<usize> {
        match self {
            Self::Prose(block) => &block.source_range,
            Self::Code(block) => &block.source_range,
        }
    }

    /// Returns the block's plain-text representation.
    pub fn plain_text(&self) -> &str {
        match self {
            Self::Prose(block) => &block.plain_text,
            Self::Code(block) => &block.code,
        }
    }
}

/// A bounded textual source fragment rendered through one native text document.
///
/// Version 1 preserves source and plain text rather than resolved inline semantics.
/// References spanning independently rendered blocks are therefore not guaranteed
/// to resolve.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProseBlock {
    pub id: String,
    pub source_range: Range<usize>,
    pub source: String,
    pub plain_text: String,
}

/// A top-level code block rendered independently from surrounding prose.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CodeBlock {
    pub id: String,
    pub source_range: Range<usize>,
    pub code: String,
    pub language: Option<String>,
}

/// Parses source text into independently renderable semantic blocks.
///
/// Block identifiers depend only on their kind and starting UTF-8 byte offset.
/// They therefore remain stable when a streaming caller only appends content.
pub fn parse(source: &str, format: SourceFormat) -> Document {
    match format {
        SourceFormat::PlainText => plain_text_document(source),
        SourceFormat::Markdown => markdown_document(source),
    }
}

fn plain_text_document(source: &str) -> Document {
    let blocks = if source.is_empty() {
        Vec::new()
    } else {
        vec![Block::Prose(ProseBlock {
            id: "prose:0".to_owned(),
            source_range: 0..source.len(),
            source: source.to_owned(),
            plain_text: source.to_owned(),
        })]
    };
    Document {
        source_format: SourceFormat::PlainText,
        blocks,
    }
}

fn markdown_document(source: &str) -> Document {
    let code_blocks = top_level_code_blocks(source);
    let mut blocks = Vec::with_capacity(code_blocks.len().saturating_mul(2).saturating_add(1));
    let mut cursor = 0;
    for code_block in code_blocks {
        push_markdown_prose(&mut blocks, source, cursor..code_block.source_range.start);
        cursor = code_block.source_range.end;
        blocks.push(Block::Code(code_block));
    }
    push_markdown_prose(&mut blocks, source, cursor..source.len());
    Document {
        source_format: SourceFormat::Markdown,
        blocks,
    }
}

fn push_markdown_prose(blocks: &mut Vec<Block>, source: &str, candidate: Range<usize>) {
    let Some(candidate) = trimmed_range(source, candidate) else {
        return;
    };

    let mut group_start = candidate.start;
    let mut group_block_count = 0usize;
    for relative_end in top_level_block_ends(&source[candidate.clone()]) {
        group_block_count += 1;
        let group_end = candidate.start + relative_end;
        let group_is_full = group_block_count >= MAX_PROSE_TOP_LEVEL_BLOCKS
            || group_end.saturating_sub(group_start) >= TARGET_PROSE_SOURCE_BYTES;
        if !group_is_full {
            continue;
        }
        push_prose_group(blocks, source, group_start..group_end);
        group_start = group_end;
        group_block_count = 0;
    }
    push_prose_group(blocks, source, group_start..candidate.end);
}

fn push_prose_group(blocks: &mut Vec<Block>, source: &str, candidate: Range<usize>) {
    let Some(source_range) = trimmed_range(source, candidate) else {
        return;
    };
    let prose_source = &source[source_range.clone()];
    blocks.push(Block::Prose(ProseBlock {
        id: format!("prose:{}", source_range.start),
        source_range,
        source: prose_source.to_owned(),
        plain_text: markdown_plain_text(prose_source),
    }));
}

fn top_level_block_ends(source: &str) -> Vec<usize> {
    let mut ends = Vec::new();
    let mut nesting_depth = 0usize;
    for (event, source_range) in Parser::new_ext(source, markdown_options()).into_offset_iter() {
        match event {
            Event::Start(_) => nesting_depth += 1,
            Event::End(_) if nesting_depth > 0 => {
                nesting_depth -= 1;
                if nesting_depth == 0 {
                    ends.push(source_range.end);
                }
            }
            Event::Rule | Event::Html(_) if nesting_depth == 0 => ends.push(source_range.end),
            _ => {}
        }
    }
    ends.dedup();
    ends
}

fn trimmed_range(source: &str, candidate: Range<usize>) -> Option<Range<usize>> {
    let candidate_source = &source[candidate.clone()];
    let leading = candidate_source.len() - candidate_source.trim_start().len();
    let trailing = candidate_source.len() - candidate_source.trim_end().len();
    let start = candidate.start + leading;
    let end = candidate.end.saturating_sub(trailing);
    (start < end).then_some(start..end)
}

struct OpenCodeBlock {
    source_start: usize,
    code: String,
    language: Option<String>,
}

fn top_level_code_blocks(source: &str) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();
    let mut nesting_depth = 0usize;
    let mut open_code_block: Option<OpenCodeBlock> = None;

    for (event, source_range) in Parser::new_ext(source, markdown_options()).into_offset_iter() {
        match event {
            Event::Start(Tag::CodeBlock(kind)) if nesting_depth == 0 => {
                open_code_block = Some(OpenCodeBlock {
                    source_start: source_range.start,
                    code: String::new(),
                    language: code_block_language(kind),
                });
            }
            Event::End(TagEnd::CodeBlock) if open_code_block.is_some() => {
                let open = open_code_block
                    .take()
                    .expect("the checked code block must still be open");
                blocks.push(CodeBlock {
                    id: format!("code:{}", open.source_start),
                    source_range: open.source_start..source_range.end,
                    code: open.code,
                    language: open.language,
                });
            }
            Event::Text(text) if open_code_block.is_some() => {
                open_code_block
                    .as_mut()
                    .expect("the checked code block must still be open")
                    .code
                    .push_str(&text);
            }
            Event::Start(_) if open_code_block.is_none() => nesting_depth += 1,
            Event::End(_) if open_code_block.is_none() => {
                nesting_depth = nesting_depth.saturating_sub(1)
            }
            _ => {}
        }
    }

    blocks
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

fn markdown_plain_text(source: &str) -> String {
    let mut output = String::new();
    for event in Parser::new_ext(source, markdown_options()) {
        match event {
            Event::Text(text)
            | Event::Code(text)
            | Event::InlineMath(text)
            | Event::DisplayMath(text)
            | Event::Html(text) => output.push_str(&text),
            Event::FootnoteReference(label) => {
                output.push_str("[^");
                output.push_str(&label);
                output.push(']');
            }
            Event::SoftBreak | Event::HardBreak => output.push('\n'),
            Event::Rule => push_line_break(&mut output),
            Event::TaskListMarker(checked) => {
                output.push_str(if checked { "[x] " } else { "[ ] " })
            }
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::Item
                | TagEnd::TableRow
                | TagEnd::DefinitionListTitle
                | TagEnd::DefinitionListDefinition,
            ) => push_line_break(&mut output),
            Event::Start(_) | Event::End(_) | Event::InlineHtml(_) => {}
        }
    }
    output.trim_end_matches('\n').to_owned()
}

fn push_line_break(output: &mut String) {
    if !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
}

fn markdown_options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM
}

#[cfg(test)]
mod tests {
    use super::{Block, SourceFormat, parse};

    #[test]
    fn preserves_plain_text_without_interpreting_markup() {
        let document = parse("# Not a heading\n`not code`", SourceFormat::PlainText);

        assert_eq!(document.source_format, SourceFormat::PlainText);
        assert_eq!(document.blocks.len(), 1);
        let Block::Prose(prose) = &document.blocks[0] else {
            panic!("plain text must produce one prose block");
        };
        assert_eq!(prose.source, "# Not a heading\n`not code`");
        assert_eq!(prose.plain_text, prose.source);
        assert_eq!(prose.source_range, 0..prose.source.len());
    }

    #[test]
    fn separates_top_level_code_from_surrounding_markdown() {
        let source = "# Result\n\n```rust\nfn main() {}\n```\n\nDone.";
        let document = parse(source, SourceFormat::Markdown);

        assert_eq!(document.blocks.len(), 3);
        let Block::Prose(heading) = &document.blocks[0] else {
            panic!("the heading must remain prose");
        };
        assert_eq!(heading.source, "# Result");
        assert_eq!(heading.plain_text, "Result");

        let Block::Code(code) = &document.blocks[1] else {
            panic!("the fenced block must become code");
        };
        assert_eq!(code.language.as_deref(), Some("rust"));
        assert_eq!(code.code, "fn main() {}\n");
        assert_eq!(
            &source[code.source_range.clone()],
            "```rust\nfn main() {}\n```"
        );

        let Block::Prose(trailing) = &document.blocks[2] else {
            panic!("the trailing paragraph must remain prose");
        };
        assert_eq!(trailing.source, "Done.");
    }

    #[test]
    fn leaves_nested_code_inside_its_markup_container() {
        let source = "> quoted\n>\n> ```text\n> code\n> ```";
        let document = parse(source, SourceFormat::Markdown);

        assert_eq!(document.blocks.len(), 1);
        let Block::Prose(prose) = &document.blocks[0] else {
            panic!("nested code must remain in the surrounding prose group");
        };
        assert_eq!(prose.source, source);
    }

    #[test]
    fn keeps_completed_block_identifiers_stable_when_the_tail_grows() {
        let prefix = "Before\n\n```sh\necho ready\n```\n";
        let initial = parse(&format!("{prefix}\nAfter"), SourceFormat::Markdown);
        let extended = parse(&format!("{prefix}\nAfter more"), SourceFormat::Markdown);

        assert_eq!(initial.blocks.len(), extended.blocks.len());
        assert_eq!(initial.blocks[0].id(), extended.blocks[0].id());
        assert_eq!(initial.blocks[1], extended.blocks[1]);
        assert_eq!(initial.blocks[2].id(), extended.blocks[2].id());
    }

    #[test]
    fn bounds_prose_groups_at_top_level_block_boundaries() {
        let source = "One.\n\nTwo.\n\nThree.\n\nFour.\n\nFive.";
        let document = parse(source, SourceFormat::Markdown);

        assert_eq!(document.blocks.len(), 2);
        let Block::Prose(committed) = &document.blocks[0] else {
            panic!("the first render unit must remain prose");
        };
        assert_eq!(committed.source, "One.\n\nTwo.\n\nThree.\n\nFour.");
        let Block::Prose(tail) = &document.blocks[1] else {
            panic!("the mutable render unit must remain prose");
        };
        assert_eq!(tail.source, "Five.");
    }

    #[test]
    fn keeps_an_unclosed_code_tail_stable_when_the_fence_closes() {
        let initial = parse("Before\n\n```rust\nfn main() {", SourceFormat::Markdown);
        let completed = parse(
            "Before\n\n```rust\nfn main() {\n}\n```",
            SourceFormat::Markdown,
        );

        assert_eq!(initial.blocks.len(), 2);
        assert_eq!(completed.blocks.len(), 2);
        assert_eq!(initial.blocks[0], completed.blocks[0]);
        assert_eq!(initial.blocks[1].id(), completed.blocks[1].id());
        let Block::Code(initial_code) = &initial.blocks[1] else {
            panic!("the unclosed fence must already produce a code block");
        };
        assert_eq!(initial_code.language.as_deref(), Some("rust"));
        assert_eq!(initial_code.code, "fn main() {");
    }

    #[test]
    fn reports_utf8_source_ranges_as_byte_offsets() {
        let source = "你好\n\n```\nworld\n```";
        let document = parse(source, SourceFormat::Markdown);
        let Block::Code(code) = &document.blocks[1] else {
            panic!("the second block must be code");
        };

        assert_eq!(code.source_range.start, "你好\n\n".len());
        assert_eq!(&source[code.source_range.clone()], "```\nworld\n```");
    }
}
