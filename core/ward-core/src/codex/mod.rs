// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{CStr, c_char};

use crate::{WardError, write_error};

mod live;
mod observer;
mod wire;

/// An opaque serialized payload passed through Ward Core's private C interface.
pub struct WardBuffer {
    bytes: Box<[u8]>,
}

unsafe fn clear_error(output_error: *mut *mut WardError) {
    if !output_error.is_null() {
        // SAFETY: The C caller supplied a writable error output pointer.
        unsafe { *output_error = std::ptr::null_mut() };
    }
}

unsafe fn required_string(
    value: *const c_char,
    name: &'static str,
    output_error: *mut *mut WardError,
) -> Option<String> {
    if value.is_null() {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, format!("{name} is missing")) };
        return None;
    }
    // SAFETY: The private C interface requires a NUL-terminated UTF-8 string.
    Some(
        unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned(),
    )
}

/// Returns the borrowed bytes in a serialized Ward buffer.
///
/// The returned pointer remains valid for the lifetime of the borrowed buffer.
///
/// # Safety
///
/// `buffer` must be null or a valid borrowed handle supplied by Ward Core.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_buffer_data(buffer: *const WardBuffer) -> *const u8 {
    // SAFETY: A non-null pointer names a valid borrowed handle.
    unsafe { buffer.as_ref() }.map_or(std::ptr::null(), |buffer| buffer.bytes.as_ptr())
}

/// Returns the number of bytes in a serialized Ward buffer.
///
/// # Safety
///
/// `buffer` must be null or a valid borrowed handle supplied by Ward Core.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_buffer_size(buffer: *const WardBuffer) -> usize {
    // SAFETY: A non-null pointer names a valid borrowed handle.
    unsafe { buffer.as_ref() }.map_or(0, |buffer| buffer.bytes.len())
}
