// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::str;

use prost::Message as _;
use ward_highlighting::{Highlighter, Theme as HighlightingTheme};

use super::buffer::WardOwnedBuffer;
use super::error::{WardError, clear_error, write_error};

mod wire {
    include!(concat!(env!("OUT_DIR"), "/ward.highlighting.v1.rs"));
}

/// An immutable syntax-highlighting engine built from embedded application packs.
pub struct WardSyntaxHighlightingEngine {
    highlighter: Highlighter,
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
        Ok(highlighter) => Box::into_raw(Box::new(WardSyntaxHighlightingEngine { highlighter })),
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
/// language may be null only when its size is zero. `theme` must be a valid
/// [`WardSyntaxHighlightingTheme`] value. `output_error`, when non-null, must be
/// writable.
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
    let source = match unsafe { utf8_argument(source, source_size, "source", true) } {
        Ok(source) => source,
        Err(message) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, message) };
            return std::ptr::null_mut();
        }
    };
    // SAFETY: The caller promises readable UTF-8 argument ranges.
    let language = match unsafe { utf8_argument(language, language_size, "language", true) } {
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

unsafe fn utf8_argument<'a>(
    pointer: *const u8,
    size: usize,
    label: &str,
    allow_null_when_empty: bool,
) -> Result<&'a str, String> {
    let bytes = if size == 0 && (allow_null_when_empty || !pointer.is_null()) {
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
        spans: highlighted
            .spans
            .into_iter()
            .map(|span| wire::Span {
                utf8_start: span.range.start as u64,
                utf8_end: span.range.end as u64,
                style: Some(wire::Style {
                    foreground: Some(color_to_wire(span.style.foreground)),
                    background: Some(color_to_wire(span.style.background)),
                    bold: span.style.bold,
                    italic: span.style.italic,
                    underline: span.style.underline,
                }),
            })
            .collect(),
        language_recognized: highlighted.language_recognized,
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
    fn highlights_through_the_app_only_c_interface() {
        let mut error = std::ptr::null_mut();

        // SAFETY: The optional error output remains writable for the call.
        let engine = unsafe { ward_core_syntax_highlighting_engine_create(&raw mut error) };
        assert!(!engine.is_null());
        assert!(error.is_null());

        let source = b"let answer = 42;\n";
        // SAFETY: The engine and all borrowed argument ranges remain live.
        let buffer = unsafe {
            ward_core_syntax_highlight(
                engine,
                source.as_ptr(),
                source.len(),
                b"rs".as_ptr(),
                2,
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
