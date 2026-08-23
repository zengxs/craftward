// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{CStr, c_char};

pub(super) use super::buffer::WardBuffer;
use super::error::{WardError, write_error};

mod execution_target;
mod live;
mod observer;
mod wire;

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
