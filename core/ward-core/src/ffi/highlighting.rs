// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{str, sync::Arc};

use prost::Message as _;
use ward_highlighting::{Highlighter, Theme as HighlightingTheme};

use super::buffer::WardOwnedBuffer;
use super::error::{WardError, clear_error, write_error};

mod wire {
    include!(concat!(env!("OUT_DIR"), "/ward.highlighting.v1.rs"));
}

/// An immutable syntax-highlighting engine built from embedded application packs.
pub struct WardSyntaxHighlightingEngine {
    highlighter: Arc<Highlighter>,
}

/// A syntax-highlighting theme selected by the application color scheme.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub enum WardSyntaxHighlightingTheme {
    Light = 0,
    Dark = 1,
}

impl From<WardSyntaxHighlightingTheme> for HighlightingTheme {
    fn from(theme: WardSyntaxHighlightingTheme) -> Self {
        match theme {
            WardSyntaxHighlightingTheme::Light => Self::Light,
            WardSyntaxHighlightingTheme::Dark => Self::Dark,
        }
    }
}

/// Creates a syntax-highlighting engine from the application-maintained packs.
///
/// The returned engine is immutable and may be used concurrently. The caller
/// owns it and must destroy it with
/// [`ward_core_syntax_highlighting_engine_destroy`].
///
/// # Safety
///
/// `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_syntax_highlighting_engine_create(
    output_error: *mut *mut WardError,
) -> *mut WardSyntaxHighlightingEngine {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    match Highlighter::new() {
        Ok(highlighter) => Box::into_raw(Box::new(WardSyntaxHighlightingEngine {
            highlighter: Arc::new(highlighter),
        })),
        Err(error) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, error.to_string()) };
            std::ptr::null_mut()
        }
    }
}

/// Destroys a syntax-highlighting engine.
///
/// # Safety
///
/// `engine` must be null or a live handle returned by
/// [`ward_core_syntax_highlighting_engine_create`], and ownership may be
/// transferred only once. No highlighting call may use it concurrently.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_syntax_highlighting_engine_destroy(
    engine: *mut WardSyntaxHighlightingEngine,
) {
    if !engine.is_null() {
        // SAFETY: The caller transfers the live handle exactly once.
        drop(unsafe { Box::from_raw(engine) });
    }
}

/// Highlights one complete source snapshot on the calling thread.
///
/// The returned buffer is a `ward.highlighting.v1.HighlightedCode` payload
/// whose spans use UTF-8 byte ranges in `source`. The caller owns the buffer
/// and must destroy it with [`ward_core_owned_buffer_destroy`].
///
/// # Safety
///
/// `engine` must point to a live engine and remain valid for the call. The
/// source and language ranges must be readable UTF-8 for their declared sizes;
/// either pointer may be null only when its corresponding size is zero. `theme`
/// must be a valid [`WardSyntaxHighlightingTheme`] value. `output_error`, when
/// non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_syntax_highlight(
    engine: *const WardSyntaxHighlightingEngine,
    source: *const u8,
    source_size: usize,
    language: *const u8,
    language_size: usize,
    theme: WardSyntaxHighlightingTheme,
    output_error: *mut *mut WardError,
) -> *mut WardOwnedBuffer {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    // SAFETY: A non-null pointer names a live immutable handle.
    let Some(engine) = (unsafe { engine.as_ref() }) else {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the syntax-highlighting engine is missing") };
        return std::ptr::null_mut();
    };
    // SAFETY: The caller promises readable UTF-8 argument ranges.
    let source = match unsafe { utf8_argument(source, source_size, "source") } {
        Ok(source) => source,
        Err(message) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, message) };
            return std::ptr::null_mut();
        }
    };
    // SAFETY: The caller promises readable UTF-8 argument ranges.
    let language = match unsafe { utf8_argument(language, language_size, "language") } {
        Ok(language) => (!language.is_empty()).then_some(language),
        Err(message) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, message) };
            return std::ptr::null_mut();
        }
    };
    match engine.highlighter.highlight(source, language, theme.into()) {
        Ok(highlighted) => {
            let highlighted = highlighted_to_wire(highlighted);
            Box::into_raw(Box::new(WardOwnedBuffer::new(highlighted.encode_to_vec())))
        }
        Err(error) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, error.to_string()) };
            std::ptr::null_mut()
        }
    }
}

/// A mutable editor highlighter, created, used and destroyed on one worker thread.
pub struct WardSyntaxDocument {
    document: ward_highlighting::Document,
}

/// Creates a document mirror using a serialized `DocumentConfiguration`.
/// The document retains its immutable engine independently of the engine handle.
///
/// # Safety
/// `engine` must be live. Nonempty input ranges must be readable. `source` must
/// be UTF-8. The optional error output must be writable. All document operations
/// including destruction must subsequently run on this same thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_syntax_document_create(
    engine: *const WardSyntaxHighlightingEngine,
    source: *const u8,
    source_size: usize,
    configuration: *const u8,
    configuration_size: usize,
    output_error: *mut *mut WardError,
) -> *mut WardSyntaxDocument {
    // SAFETY: The caller guarantees all argument ranges and the optional output.
    unsafe { clear_error(output_error) };
    let result = (|| {
        // SAFETY: Non-null engine handles remain live for this call.
        let engine =
            unsafe { engine.as_ref() }.ok_or("the syntax-highlighting engine is missing")?;
        // SAFETY: Input ranges remain readable for this call.
        let source = unsafe { utf8_argument(source, source_size, "source") }?;
        let config: wire::DocumentConfiguration =
            unsafe { decode_argument(configuration, configuration_size) }?;
        let theme = if config.dark_theme {
            HighlightingTheme::Dark
        } else {
            HighlightingTheme::Light
        };
        Ok::<_, String>(ward_highlighting::Document::new(
            engine.highlighter.clone(),
            source,
            &config.language,
            &config.file_name,
            theme,
        ))
    })();
    match result {
        Ok(document) => Box::into_raw(Box::new(WardSyntaxDocument { document })),
        Err(message) => {
            // SAFETY: The caller provided the optional error output.
            unsafe { write_error(output_error, message) };
            std::ptr::null_mut()
        }
    }
}

/// Applies an ordered serialized `DocumentEdit` to a document mirror.
///
/// # Safety
/// `document` must be a live handle on its owning thread. The input range must
/// be readable and the optional error output writable. Calls must not overlap.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_syntax_document_edit(
    document: *mut WardSyntaxDocument,
    edit: *const u8,
    edit_size: usize,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller provided the optional error output.
    unsafe { clear_error(output_error) };
    let result = (|| {
        // SAFETY: The handle is exclusively borrowed on its owning thread.
        let document =
            unsafe { document.as_mut() }.ok_or("the highlighting document is missing")?;
        // SAFETY: The caller guarantees a readable input range.
        let edit: wire::DocumentEdit = unsafe { decode_argument(edit, edit_size) }?;
        let inserted = str::from_utf8(&edit.inserted).map_err(|error| error.to_string())?;
        let start = usize::try_from(edit.start).map_err(|error| error.to_string())?;
        let deleted = usize::try_from(edit.deleted).map_err(|error| error.to_string())?;
        document
            .document
            .edit(edit.from_revision, start, deleted, inserted)
            .map_err(|error| error.to_string())
    })();
    if let Err(message) = result {
        // SAFETY: The caller provided the optional error output.
        unsafe { write_error(output_error, message) };
        return false;
    }
    true
}

/// Reconfigures a document using serialized `DocumentConfiguration` and advances
/// its configuration counter, initially zero, by one.
///
/// # Safety
/// The handle must be exclusively accessed on its owning thread. The input
/// range must be readable and the optional error output writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_syntax_document_configure(
    document: *mut WardSyntaxDocument,
    configuration: *const u8,
    configuration_size: usize,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller provided the optional error output.
    unsafe { clear_error(output_error) };
    let result = (|| {
        // SAFETY: The handle is exclusively borrowed on its owning thread.
        let document =
            unsafe { document.as_mut() }.ok_or("the highlighting document is missing")?;
        // SAFETY: The caller guarantees a readable input range.
        let config: wire::DocumentConfiguration =
            unsafe { decode_argument(configuration, configuration_size) }?;
        let theme = if config.dark_theme {
            HighlightingTheme::Dark
        } else {
            HighlightingTheme::Light
        };
        document
            .document
            .configure(&config.language, &config.file_name, theme);
        Ok::<_, String>(())
    })();
    if let Err(message) = result {
        // SAFETY: The caller provided the optional error output.
        unsafe { write_error(output_error, message) };
        return false;
    }
    true
}

/// Parses a bounded batch and returns serialized `DocumentStyles`. Null without
/// an error means no further work. A returned batch must be acknowledged before
/// stepping again. The caller owns the returned buffer.
///
/// # Safety
/// The handle must be exclusively accessed on its owning thread and the optional
/// error output must be writable. Budgets are positive counts of lines and bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_syntax_document_step(
    document: *mut WardSyntaxDocument,
    line_budget: usize,
    byte_budget: usize,
    output_error: *mut *mut WardError,
) -> *mut WardOwnedBuffer {
    // SAFETY: The caller provided the optional error output.
    unsafe { clear_error(output_error) };
    // SAFETY: The handle is exclusively borrowed on its owning thread.
    let Some(document) = (unsafe { document.as_mut() }) else {
        return std::ptr::null_mut();
    };
    match document.document.step(line_budget, byte_budget) {
        Ok(Some(batch)) => {
            let styles = wire::DocumentStyles {
                revision: batch.revision,
                configuration: batch.configuration,
                sequence: batch.sequence,
                start: batch.range.start as u64,
                end: batch.range.end as u64,
                spans: batch.spans.into_iter().map(span_to_wire).collect(),
                syntax_name: batch.syntax_name,
                language_recognized: batch.language_recognized,
                complete: batch.complete,
            };
            Box::into_raw(Box::new(WardOwnedBuffer::new(styles.encode_to_vec())))
        }
        Ok(None) => std::ptr::null_mut(),
        Err(error) => {
            // SAFETY: The caller provided the optional error output.
            unsafe { write_error(output_error, error.to_string()) };
            std::ptr::null_mut()
        }
    }
}

/// Acknowledges a published batch. Rejected or obsolete styles remain pending.
///
/// # Safety
/// The document must be live and exclusively accessed on its owning thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_syntax_document_acknowledge(
    document: *mut WardSyntaxDocument,
    sequence: u64,
    applied: bool,
) {
    // SAFETY: The caller provides exclusive access on the owning thread.
    if let Some(document) = unsafe { document.as_mut() } {
        document.document.acknowledge(sequence, applied);
    }
}

/// Destroys a document on its owning thread.
///
/// # Safety
/// The handle must be null or live, with ownership transferred exactly once.
/// No other call may use the document concurrently.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_syntax_document_destroy(document: *mut WardSyntaxDocument) {
    if !document.is_null() {
        // SAFETY: The caller transfers ownership exactly once on the owning thread.
        drop(unsafe { Box::from_raw(document) });
    }
}

unsafe fn decode_argument<T: prost::Message + Default>(
    pointer: *const u8,
    size: usize,
) -> Result<T, String> {
    let bytes = if size == 0 {
        &[]
    } else {
        if pointer.is_null() {
            return Err("the highlighting request is missing".into());
        }
        // SAFETY: The caller guarantees that the input range is readable.
        unsafe { std::slice::from_raw_parts(pointer, size) }
    };
    T::decode(bytes).map_err(|error| error.to_string())
}

unsafe fn utf8_argument<'a>(
    pointer: *const u8,
    size: usize,
    label: &str,
) -> Result<&'a str, String> {
    let bytes = if size == 0 {
        &[]
    } else {
        if pointer.is_null() {
            return Err(format!("the syntax-highlighting {label} is missing"));
        }
        // SAFETY: The caller promises this byte range is readable.
        unsafe { std::slice::from_raw_parts(pointer, size) }
    };
    str::from_utf8(bytes)
        .map_err(|error| format!("the syntax-highlighting {label} is not UTF-8: {error}"))
}

fn highlighted_to_wire(highlighted: ward_highlighting::HighlightedCode) -> wire::HighlightedCode {
    wire::HighlightedCode {
        syntax_name: highlighted.syntax_name,
        spans: highlighted.spans.into_iter().map(span_to_wire).collect(),
        language_recognized: highlighted.language_recognized,
    }
}

fn span_to_wire(span: ward_highlighting::Span) -> wire::Span {
    wire::Span {
        utf8_start: span.range.start as u64,
        utf8_end: span.range.end as u64,
        style: Some(wire::Style {
            foreground: Some(color_to_wire(span.style.foreground)),
            background: Some(color_to_wire(span.style.background)),
            bold: span.style.bold,
            italic: span.style.italic,
            underline: span.style.underline,
        }),
    }
}

fn color_to_wire(color: ward_highlighting::Color) -> wire::Color {
    wire::Color {
        red: color.red.into(),
        green: color.green.into(),
        blue: color.blue.into(),
        alpha: color.alpha.into(),
    }
}

#[cfg(test)]
mod tests {
    use prost::Message as _;

    use super::super::buffer::{
        ward_core_owned_buffer_data, ward_core_owned_buffer_destroy, ward_core_owned_buffer_size,
    };
    use super::{
        WardSyntaxHighlightingTheme, ward_core_syntax_highlight,
        ward_core_syntax_highlighting_engine_create, ward_core_syntax_highlighting_engine_destroy,
        wire,
    };

    #[test]
    fn document_interface_preserves_pending_styles_and_owns_its_engine() {
        use super::*;
        let source = b"let value = 1;\n";
        let configuration = wire::DocumentConfiguration {
            language: "rust".into(),
            file_name: String::new(),
            dark_theme: false,
        }
        .encode_to_vec();
        let mut error = std::ptr::null_mut();
        // SAFETY: All inputs are owned for these calls; the document is used and
        // destroyed on this test thread. Every returned buffer is released once.
        unsafe {
            let engine = ward_core_syntax_highlighting_engine_create(&raw mut error);
            assert!(!engine.is_null());
            let document = ward_core_syntax_document_create(
                engine,
                source.as_ptr(),
                source.len(),
                configuration.as_ptr(),
                configuration.len(),
                &raw mut error,
            );
            assert!(!document.is_null());
            ward_core_syntax_highlighting_engine_destroy(engine);
            let buffer = ward_core_syntax_document_step(document, 32, 4096, &raw mut error);
            assert!(!buffer.is_null());
            let batch = wire::DocumentStyles::decode(std::slice::from_raw_parts(
                ward_core_owned_buffer_data(buffer),
                ward_core_owned_buffer_size(buffer),
            ))
            .unwrap();
            ward_core_owned_buffer_destroy(buffer);
            assert_eq!(batch.syntax_name, "Rust");
            let edit = wire::DocumentEdit {
                from_revision: 0,
                start: 0,
                deleted: 0,
                inserted: b"// ".to_vec(),
            }
            .encode_to_vec();
            assert!(ward_core_syntax_document_edit(
                document,
                edit.as_ptr(),
                edit.len(),
                &raw mut error
            ));
            ward_core_syntax_document_acknowledge(document, batch.sequence, false);
            let buffer = ward_core_syntax_document_step(document, 32, 4096, &raw mut error);
            assert!(!buffer.is_null());
            let batch = wire::DocumentStyles::decode(std::slice::from_raw_parts(
                ward_core_owned_buffer_data(buffer),
                ward_core_owned_buffer_size(buffer),
            ))
            .unwrap();
            ward_core_owned_buffer_destroy(buffer);
            assert_eq!(batch.revision, 1);
            assert_eq!(batch.start, 0);
            assert_eq!(batch.end, (source.len() + 3) as u64);
            ward_core_syntax_document_acknowledge(document, batch.sequence, true);
            assert!(ward_core_syntax_document_step(document, 32, 4096, &raw mut error).is_null());
            assert!(error.is_null());
            ward_core_syntax_document_destroy(document);
        }
    }

    #[test]
    fn highlights_through_the_app_only_c_interface() {
        let mut error = std::ptr::null_mut();

        // SAFETY: The optional error output remains writable for the call.
        let engine = unsafe { ward_core_syntax_highlighting_engine_create(&raw mut error) };
        assert!(!engine.is_null());
        assert!(error.is_null());

        let source = b"let answer = 42;\n";
        let language = b"rs";
        // SAFETY: The engine and all borrowed argument ranges remain live.
        let buffer = unsafe {
            ward_core_syntax_highlight(
                engine,
                source.as_ptr(),
                source.len(),
                language.as_ptr(),
                language.len(),
                WardSyntaxHighlightingTheme::Light,
                &raw mut error,
            )
        };
        assert!(!buffer.is_null());
        assert!(error.is_null());

        // SAFETY: The returned buffer remains live until destruction below.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                ward_core_owned_buffer_data(buffer),
                ward_core_owned_buffer_size(buffer),
            )
        };
        let highlighted = wire::HighlightedCode::decode(bytes).expect("the result should decode");
        assert_eq!(highlighted.syntax_name, "Rust");
        assert!(highlighted.language_recognized);
        assert_eq!(highlighted.spans.first().expect("a span").utf8_start, 0);
        assert_eq!(
            highlighted.spans.last().expect("a span").utf8_end,
            source.len() as u64
        );

        // SAFETY: Both owned values are transferred exactly once.
        unsafe {
            ward_core_owned_buffer_destroy(buffer);
            ward_core_syntax_highlighting_engine_destroy(engine);
        }
    }
}
