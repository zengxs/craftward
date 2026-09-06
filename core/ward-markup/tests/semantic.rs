// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashSet;

use ward_markup::{
    Alignment, ContainerKind, NodeContent, SemanticDocument, SourceFormat, TextKind, parse_semantic,
};

fn markdown(source: &str) -> SemanticDocument {
    let document = parse_semantic(source, SourceFormat::Markdown);
    for block in &document.blocks {
        let mut ids = HashSet::new();
        for (index, node) in block.nodes.iter().enumerate() {
            assert!(ids.insert(&node.id), "duplicate identity: {}", node.id);
            assert!(source.get(node.source_range.clone()).is_some());
            assert!(node.source_range.start >= block.source_range.start);
            assert!(node.source_range.end <= block.source_range.end);
            if let Some(parent) = node.parent {
                assert!(parent < index);
                let parent = &block.nodes[parent];
                assert!(parent.source_range.start <= node.source_range.start);
                assert!(parent.source_range.end >= node.source_range.end);
            } else {
                assert_eq!(index, 0);
            }
        }
    }
    document
}

fn texts(document: &SemanticDocument) -> String {
    document
        .blocks
        .iter()
        .flat_map(|block| &block.nodes)
        .filter_map(|node| match &node.content {
            NodeContent::Text { value, .. } => Some(value.text.as_str()),
            NodeContent::Annotation { label, .. } => Some(label.text.as_str()),
            _ => None,
        })
        .collect()
}

#[test]
fn plain_text_does_not_interpret_markdown_or_directives() {
    let source = "**你好 👩‍💻** :codex-annotation{index=\"4\"}";
    let document = parse_semantic(source, SourceFormat::PlainText);
    assert_eq!(texts(&document), source);
    assert_eq!(document.blocks[0].nodes.len(), 2);
    assert!(
        parse_semantic("", SourceFormat::PlainText)
            .blocks
            .is_empty()
    );
}

#[test]
fn preserves_nested_inline_structure_and_resolved_link_metadata() {
    let document = markdown("# Heading\n\n**bold *and `a()`* [guide](/go \"Title\")**");
    assert_eq!(document.blocks.len(), 2);
    assert_eq!(document.blocks[0].nodes[0].content, NodeContent::Heading(1));
    let nodes = &document.blocks[1].nodes;
    let code = nodes
        .iter()
        .find(|node| {
            matches!(
                node.content,
                NodeContent::Text {
                    kind: TextKind::InlineCode,
                    ..
                }
            )
        })
        .unwrap();
    let emphasis = &nodes[code.parent.unwrap()];
    assert_eq!(
        emphasis.content,
        NodeContent::Container(ContainerKind::Emphasis)
    );
    assert_eq!(
        nodes[emphasis.parent.unwrap()].content,
        NodeContent::Container(ContainerKind::Strong)
    );
    assert!(nodes.iter().any(|node| node.content
        == NodeContent::Link {
            target: "/go".into(),
            title: "Title".into()
        }));
    assert_eq!(texts(&document), "Headingbold and a() guide");
}

#[test]
fn recognizes_only_complete_unescaped_known_annotations_in_ordinary_text() {
    let directive = ":codex-annotation{index=\"4\"}";
    let document = markdown(&format!(
        "**{directive}** &amp; :codex-annotation{{ index = \"12\" }}"
    ));
    let annotations: Vec<_> = document.blocks[0]
        .nodes
        .iter()
        .filter_map(|node| match &node.content {
            NodeContent::Annotation { index, label } => Some((*index, label.text.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(annotations, [(4, "[4]"), (12, "[12]")]);
    for source in [
        format!("\\{directive}"),
        format!("`{directive}`"),
        format!("```text\n{directive}\n```"),
        format!("[{directive}](/go)"),
        format!("![{directive}](/image)"),
        ":codex-annotation{index=\"4\"".into(),
        ":codex-annotation{index=\"0\"}".into(),
        ":codex-annotation{index=\"4294967296\"}".into(),
        ":codex-annotation{index=\"4\" extra=\"x\"}".into(),
        ":codex-unknown{index=\"4\"}".into(),
        "&#58;codex-annotation{index=\"4\"}".into(),
    ] {
        let document = markdown(&source);
        assert!(
            !document
                .blocks
                .iter()
                .flat_map(|block| &block.nodes)
                .any(|node| matches!(node.content, NodeContent::Annotation { .. })),
            "unexpected annotation: {source}"
        );
        assert!(texts(&document).contains(":codex-"));
    }
}

#[test]
fn tables_retain_rows_cells_alignment_and_inline_content() {
    let source = "| Name | State |\n|:---|---:|\n| `a()` | [Ready][r] :codex-annotation{index=\"4\"} |\n\n[r]: /ready \"Status\"";
    let document = markdown(source);
    assert_eq!(document.blocks.len(), 1);
    let nodes = &document.blocks[0].nodes;
    assert_eq!(
        nodes[0].content,
        NodeContent::Table {
            columns: vec![Alignment::Left, Alignment::Right]
        }
    );
    assert_eq!(
        nodes
            .iter()
            .filter(|node| matches!(node.content, NodeContent::TableRow { .. }))
            .count(),
        2
    );
    assert_eq!(
        nodes
            .iter()
            .filter(|node| node.content == NodeContent::Container(ContainerKind::TableCell))
            .count(),
        4
    );
    let annotation = nodes
        .iter()
        .find(|node| matches!(node.content, NodeContent::Annotation { .. }))
        .unwrap();
    assert_eq!(
        nodes[annotation.parent.unwrap()].content,
        NodeContent::Container(ContainerKind::TableCell)
    );
    assert!(nodes.iter().any(|node| node.content
        == NodeContent::Link {
            target: "/ready".into(),
            title: "Status".into()
        }));
    assert_eq!(texts(&document), "NameStatea()Ready [4]");
}

#[test]
fn maps_utf8_source_and_utf16_text_without_fabricating_replacement_offsets() {
    let source = "你好 👩‍💻 &amp; ` a\nb ` :codex-annotation{index=\"4\"}";
    let document = markdown(source);
    let nodes = &document.blocks[0].nodes;
    let values: Vec<_> = nodes
        .iter()
        .filter_map(|node| match &node.content {
            NodeContent::Text { value, .. } => Some(value),
            _ => None,
        })
        .collect();
    assert_eq!(values[0].text, "你好 👩‍💻 ");
    assert_eq!(values[0].mappings[0].utf16_range, 0..9);
    assert_eq!(values[0].mappings[0].source_range, 0..19);
    assert!(values[0].mappings[0].verbatim);
    let entity = values.iter().find(|value| value.text == "&").unwrap();
    assert_eq!(&source[entity.mappings[0].source_range.clone()], "&amp;");
    assert_eq!(entity.mappings[0].utf16_range, 0..1);
    assert!(!entity.mappings[0].verbatim);
    let code = values.iter().find(|value| value.text == "a b").unwrap();
    assert_eq!(&source[code.mappings[0].source_range.clone()], "` a\nb `");
    assert!(!code.mappings[0].verbatim);
}

#[test]
fn append_retains_unchanged_identities_and_updates_the_growing_text_node() {
    let prefix = "Stable **prefix**.\n\n```rust\nlet x = 1;\n```\n\n";
    let initial = markdown(&format!("{prefix}Tail"));
    let extended = markdown(&format!("{prefix}Tail more"));
    assert_eq!(initial.blocks[..2], extended.blocks[..2]);
    assert_eq!(initial.blocks[2].id, extended.blocks[2].id);
    assert_eq!(
        initial.blocks[2].nodes[1].id,
        extended.blocks[2].nodes[1].id
    );
    assert_eq!(texts(&extended), "Stable prefix.let x = 1;\nTail more");
}

#[test]
fn completing_syntax_replaces_semantics_without_changing_earlier_blocks() {
    let initial = markdown("Stable.\n\n:codex-annotation{index=\"4\"");
    let complete = markdown("Stable.\n\n:codex-annotation{index=\"4\"}");
    assert_eq!(initial.blocks[0], complete.blocks[0]);
    assert_eq!(initial.blocks[1].id, complete.blocks[1].id);
    assert!(matches!(
        initial.blocks[1].nodes[1].content,
        NodeContent::Text { .. }
    ));
    assert!(matches!(
        complete.blocks[1].nodes[1].content,
        NodeContent::Annotation { index: 4, .. }
    ));
    let initial = markdown("[later][r]");
    let complete = markdown("[later][r]\n\n[r]: /resolved");
    assert_eq!(texts(&initial), "[later][r]");
    assert_eq!(texts(&complete), "later");
    assert!(complete.blocks[0].nodes.iter().any(
        |node| matches!(&node.content, NodeContent::Link { target, .. } if target == "/resolved")
    ));
}

#[test]
fn retains_code_whitespace_nested_lists_and_opaque_syntax() {
    let source = "0. first\n   - [ ] nested\n\n> quote\n\n```rust\n    a()\n\n    b()\n```\n\n<div>:codex-annotation{index=\"4\"}</div>";
    let document = markdown(source);
    assert!(
        document
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| node.content == NodeContent::List { start: Some(0) })
    );
    assert!(
        document
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .any(|node| node.content == NodeContent::TaskMarker { checked: false })
    );
    assert!(texts(&document).contains("    a()\n\n    b()\n"));
    assert!(matches!(
        document.blocks.last().unwrap().nodes[0].content,
        NodeContent::Text {
            kind: TextKind::Unsupported,
            ..
        }
    ));
    assert!(texts(&document).contains("<div>:codex-annotation"));
}

#[test]
fn structure_and_ranges_cover_empty_cells_footnotes_breaks_and_admonitions() {
    for source in [
        "| A | B |\n|---|---|\n| | value |\n| tail | |",
        "text[^a]\n\n[^a]: footnote **body**\n\n    second paragraph",
        "> [!NOTE]\n> **hello**",
        "first  \r\nsecond\r\nthird\n\n---",
        "![**alt**](/image \"Title\") and <span>literal tag</span>",
        "- item\n\n  ```rust\n  \tcode\n  ```",
        "```\n```",
        "* [ ] foo\n\n* [ ] bar\n\nbaz\n",
    ] {
        markdown(source);
    }
    for (name, kind) in [
        ("NOTE", "note"),
        ("TIP", "tip"),
        ("IMPORTANT", "important"),
        ("WARNING", "warning"),
        ("CAUTION", "caution"),
    ] {
        let document = markdown(&format!("> [!{name}]\n> body"));
        assert_eq!(
            document.blocks[0].nodes[0].content,
            NodeContent::Admonition { kind: kind.into() }
        );
        assert_eq!(texts(&document), "body");
    }
}
