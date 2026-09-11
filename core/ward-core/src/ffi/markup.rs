// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::str;

use prost::Message as _;
use ward_markup::SourceFormat;

use super::buffer::WardOwnedBuffer;
use super::error::{WardError, clear_error, write_error};

mod wire {
    include!(concat!(env!("OUT_DIR"), "/ward.markup.v1.rs"));
}

mod semantic;

/// A source syntax accepted by Ward Core's app-only markup interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub enum WardMarkupSourceFormat {
    PlainText = 0,
    Markdown = 1,
}

impl From<WardMarkupSourceFormat> for SourceFormat {
    fn from(format: WardMarkupSourceFormat) -> Self {
        match format {
            WardMarkupSourceFormat::PlainText => Self::PlainText,
            WardMarkupSourceFormat::Markdown => Self::Markdown,
        }
    }
}

/// Parses a complete message snapshot into explicit inline and block semantics.
///
/// The returned buffer is a `ward.markup.v1.SemanticDocument` payload.
/// Reference resolution requires the complete source, not an independently parsed
/// tail. This synchronous entry point does
/// not retain state or perform layout. The caller owns the buffer and must destroy
/// it with `ward_core_owned_buffer_destroy`.
///
/// # Safety
///
/// `source` must point to `source_size` readable UTF-8 bytes when the size is
/// positive. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_markup_parse_semantic(
    format: WardMarkupSourceFormat,
    source: *const u8,
    source_size: usize,
    output_error: *mut *mut WardError,
) -> *mut WardOwnedBuffer {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    let source_bytes = if source_size == 0 {
        &[]
    } else {
        if source.is_null() {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, "the markup source is missing") };
            return std::ptr::null_mut();
        }
        // SAFETY: The caller guarantees that the source range is readable.
        unsafe { std::slice::from_raw_parts(source, source_size) }
    };
    let source = match str::from_utf8(source_bytes) {
        Ok(source) => source,
        Err(error) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe {
                write_error(
                    output_error,
                    format!("the markup source is not UTF-8: {error}"),
                )
            };
            return std::ptr::null_mut();
        }
    };

    let document = semantic::document_to_wire(ward_markup::parse_semantic(source, format.into()));
    Box::into_raw(Box::new(WardOwnedBuffer::new(document.encode_to_vec())))
}

#[cfg(test)]
mod tests {
    use prost::Message as _;

    use super::super::buffer::{
        ward_core_owned_buffer_data, ward_core_owned_buffer_destroy, ward_core_owned_buffer_size,
    };
    use super::super::error::ward_core_error_destroy;
    use super::{WardMarkupSourceFormat, ward_core_markup_parse_semantic, wire};

    #[test]
    fn serializes_semantic_structure_and_source_mapping_through_the_c_interface() {
        let source = "你好 👋 **bold** :codex-annotation{index=\"4\"}\n\n0. [ ] task\n\n| A | B |\n|---|---:|\n| `a()` | [Ready][r] |\n\n[r]: /ready";
        let mut error = std::ptr::null_mut();
        // SAFETY: Source and error output are valid until the call returns.
        let buffer = unsafe {
            ward_core_markup_parse_semantic(
                WardMarkupSourceFormat::Markdown,
                source.as_ptr(),
                source.len(),
                &raw mut error,
            )
        };
        assert!(!buffer.is_null());
        assert!(error.is_null());
        // SAFETY: The owned buffer remains live while it is decoded.
        let document = unsafe {
            wire::SemanticDocument::decode(std::slice::from_raw_parts(
                ward_core_owned_buffer_data(buffer),
                ward_core_owned_buffer_size(buffer),
            ))
        }
        .unwrap();
        // SAFETY: Decoding owns its data; release the buffer exactly once.
        unsafe { ward_core_owned_buffer_destroy(buffer) };
        assert_eq!(document.blocks.len(), 3);
        assert!(document.blocks[0].nodes[0].parent_index.is_none());
        assert_eq!(document.blocks[0].nodes[1].parent_index, Some(0));
        use wire::semantic_node::Body;
        let nodes: Vec<_> = document
            .blocks
            .iter()
            .flat_map(|block| &block.nodes)
            .collect();
        assert!(nodes.iter().any(|node| matches!(&node.body, Some(Body::Annotation(annotation)) if annotation.index == 4 && annotation.label.as_ref().unwrap().text == "[4]")));
        assert!(
            nodes.iter().any(
                |node| matches!(&node.body, Some(Body::Link(link)) if link.target == "/ready")
            )
        );
        assert!(
            nodes
                .iter()
                .any(|node| matches!(node.body, Some(Body::TaskChecked(false))))
        );
        assert!(
            nodes
                .iter()
                .any(|node| matches!(node.body, Some(Body::TableRowHeader(false))))
        );
        assert!(
            nodes
                .iter()
                .any(|node| matches!(&node.body, Some(Body::List(list)) if list.start == Some(0)))
        );
        let Some(Body::Text(text)) = &document.blocks[0].nodes[1].body else {
            panic!("expected decoded text");
        };
        let value = text.value.as_ref().unwrap();
        assert_eq!(value.text, "你好 👋 ");
        assert_eq!(value.mappings[0].utf16_end, 6);
        assert_eq!(value.mappings[0].source.as_ref().unwrap().end, 12);
        assert!(value.mappings[0].verbatim);
    }

    #[test]
    fn serializes_code_comment_metadata_and_markdown_children() {
        let source = r#"::code-comment{title="[P0] 修复" body="Use **bold** and `code`.\n\nSecond paragraph." file="/project/file.cpp" start=2 end=4 priority=0}"#;
        let mut error = std::ptr::null_mut();
        // SAFETY: Source and error output are valid until the call returns.
        let buffer = unsafe {
            ward_core_markup_parse_semantic(
                WardMarkupSourceFormat::Markdown,
                source.as_ptr(),
                source.len(),
                &raw mut error,
            )
        };
        assert!(!buffer.is_null());
        assert!(error.is_null());
        // SAFETY: The owned buffer remains live while it is decoded.
        let document = unsafe {
            wire::SemanticDocument::decode(std::slice::from_raw_parts(
                ward_core_owned_buffer_data(buffer),
                ward_core_owned_buffer_size(buffer),
            ))
        }
        .unwrap();
        // SAFETY: Decoding owns its data; release the buffer exactly once.
        unsafe { ward_core_owned_buffer_destroy(buffer) };
        assert_eq!(document.blocks.len(), 1);
        let nodes = &document.blocks[0].nodes;
        use wire::semantic_node::Body;
        let Some(Body::CodeComment(comment)) = &nodes[0].body else {
            panic!("expected a code comment");
        };
        assert_eq!(comment.title.as_ref().unwrap().text, "[P0] 修复");
        assert_eq!(comment.file.as_ref().unwrap().text, "/project/file.cpp");
        assert_eq!(
            (comment.start, comment.end, comment.priority),
            (Some(2), Some(4), Some(0))
        );
        assert_eq!(
            nodes
                .iter()
                .filter(|node| node.parent_index == Some(0))
                .count(),
            2
        );
        assert!(nodes.iter().any(|node| matches!(node.body, Some(Body::Container(kind)) if kind == wire::ContainerKind::Strong as i32)));
        let code = nodes
            .iter()
            .find_map(|node| match &node.body {
                Some(Body::Text(text)) if text.kind == wire::TextKind::InlineCode as i32 => {
                    Some(text.value.as_ref().unwrap())
                }
                _ => None,
            })
            .unwrap();
        assert_eq!(code.text, "code");
        let mapping = &code.mappings[0];
        let range = mapping.source.as_ref().unwrap();
        assert_eq!(&source[range.start as usize..range.end as usize], "`code`");
        assert!(!mapping.verbatim);
    }

    #[test]
    fn semantic_interface_accepts_empty_input_and_rejects_invalid_input() {
        let mut error = std::ptr::null_mut();
        // SAFETY: A zero-length input may have a null source pointer.
        let buffer = unsafe {
            ward_core_markup_parse_semantic(
                WardMarkupSourceFormat::Markdown,
                std::ptr::null(),
                0,
                &raw mut error,
            )
        };
        assert!(!buffer.is_null());
        assert!(error.is_null());
        // SAFETY: The returned buffer is owned by this test.
        unsafe { ward_core_owned_buffer_destroy(buffer) };
        for (pointer, size) in [(std::ptr::null(), 1), (b"\xff".as_ptr(), 1)] {
            // SAFETY: The non-null source contains the stated number of bytes.
            let buffer = unsafe {
                ward_core_markup_parse_semantic(
                    WardMarkupSourceFormat::Markdown,
                    pointer,
                    size,
                    &raw mut error,
                )
            };
            assert!(buffer.is_null());
            assert!(!error.is_null());
            // SAFETY: Each failed call returns a separately owned error.
            unsafe { ward_core_error_destroy(error) };
            error = std::ptr::null_mut();
        }
    }
}
