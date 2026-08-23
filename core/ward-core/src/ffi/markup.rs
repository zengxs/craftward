// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::str;

use prost::Message as _;
use ward_markup::{Block as MarkupBlock, SourceFormat};

use super::buffer::WardOwnedBuffer;
use super::error::{WardError, clear_error, write_error};

mod wire {
    include!(concat!(env!("OUT_DIR"), "/ward.markup.v1.rs"));
}

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

/// Parses source text into an owned serialized render document on the calling thread.
///
/// The returned buffer is a `ward.markup.v1.Document` payload. The caller owns
/// it and must destroy it with [`ward_core_owned_buffer_destroy`].
///
/// # Safety
///
/// `source` must point to `source_size` readable bytes when `source_size` is
/// positive. The bytes must be UTF-8. `output_error`, when non-null, must be
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_markup_parse(
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

    let document = document_to_wire(ward_markup::parse(source, format.into()));
    Box::into_raw(Box::new(WardOwnedBuffer::new(document.encode_to_vec())))
}

fn document_to_wire(document: ward_markup::Document) -> wire::Document {
    wire::Document {
        source_format: match document.source_format {
            SourceFormat::PlainText => wire::SourceFormat::PlainText,
            SourceFormat::Markdown => wire::SourceFormat::Markdown,
            _ => wire::SourceFormat::Unspecified,
        } as i32,
        blocks: document.blocks.into_iter().map(block_to_wire).collect(),
    }
}

fn block_to_wire(block: MarkupBlock) -> wire::Block {
    let source_range = block.source_range().clone();
    let block_id = block.id().to_owned();
    let body = match block {
        MarkupBlock::Prose(prose) => wire::block::Body::Prose(wire::ProseBlock {
            source: prose.source,
            plain_text: prose.plain_text,
        }),
        MarkupBlock::Code(code) => wire::block::Body::CodeBlock(wire::CodeBlock {
            code: code.code,
            language: code.language,
        }),
        _ => unreachable!("ward-markup returned an unsupported block"),
    };
    wire::Block {
        block_id,
        source_start: source_range.start as u64,
        source_end: source_range.end as u64,
        body: Some(body),
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    use prost::Message as _;

    use super::super::buffer::{
        ward_core_owned_buffer_data, ward_core_owned_buffer_destroy, ward_core_owned_buffer_size,
    };
    use super::super::error::{ward_core_error_destroy, ward_core_error_message};
    use super::{WardMarkupSourceFormat, ward_core_markup_parse, wire};

    #[test]
    fn parses_markdown_through_the_private_c_interface() {
        let source = b"Before\n\n```rust\nfn main() {}\n```";
        let mut error = std::ptr::null_mut();

        // SAFETY: The source and error output satisfy the private C interface.
        let buffer = unsafe {
            ward_core_markup_parse(
                WardMarkupSourceFormat::Markdown,
                source.as_ptr(),
                source.len(),
                &raw mut error,
            )
        };

        assert!(!buffer.is_null());
        assert!(error.is_null());
        // SAFETY: The returned buffer remains live until it is destroyed below.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                ward_core_owned_buffer_data(buffer),
                ward_core_owned_buffer_size(buffer),
            )
        };
        let document = wire::Document::decode(bytes).expect("the document should decode");
        assert_eq!(document.blocks.len(), 2);
        assert!(matches!(
            document.blocks[1].body,
            Some(wire::block::Body::CodeBlock(_))
        ));

        // SAFETY: Ownership of the returned buffer is transferred exactly once.
        unsafe { ward_core_owned_buffer_destroy(buffer) };
    }

    #[test]
    fn rejects_non_utf8_source() {
        let source = [0xff];
        let mut error = std::ptr::null_mut();

        // SAFETY: The source and error output satisfy the private C interface.
        let buffer = unsafe {
            ward_core_markup_parse(
                WardMarkupSourceFormat::PlainText,
                source.as_ptr(),
                source.len(),
                &raw mut error,
            )
        };

        assert!(buffer.is_null());
        assert!(!error.is_null());
        // SAFETY: The error remains live until it is destroyed below.
        let message = unsafe { CStr::from_ptr(ward_core_error_message(error)) };
        assert!(message.to_string_lossy().contains("not UTF-8"));
        // SAFETY: Ownership of the returned error is transferred exactly once.
        unsafe { ward_core_error_destroy(error) };
    }
}
