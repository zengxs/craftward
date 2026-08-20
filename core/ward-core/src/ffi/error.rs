// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{CString, c_char};

/// An owned immediate-call error passed through Ward Core's private C
/// interface.
///
/// A caller that receives a non-null error owns it and must destroy it with
/// [`ward_core_error_destroy`]. Final failures for accepted asynchronous
/// operations are delivered through their event callback instead of this type.
pub struct WardError {
    message: CString,
}

pub(super) fn c_string(value: impl AsRef<str>) -> CString {
    let value = value.as_ref().replace('\0', "�");
    CString::new(value).expect("NUL bytes were replaced before constructing the C string")
}

pub(super) unsafe fn write_error(output: *mut *mut WardError, message: impl AsRef<str>) {
    if output.is_null() {
        return;
    }

    let error = Box::new(WardError {
        message: c_string(message),
    });
    // SAFETY: The caller provided a writable output pointer and takes
    // ownership of the resulting WardError.
    unsafe { *output = Box::into_raw(error) };
}

pub(super) unsafe fn clear_error(output: *mut *mut WardError) {
    if !output.is_null() {
        // SAFETY: The C caller supplied a writable error output pointer.
        unsafe { *output = std::ptr::null_mut() };
    }
}

/// Returns a borrowed error message owned by `error`.
///
/// # Safety
///
/// `error` must be null or a live error returned by this interface.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_error_message(error: *const WardError) -> *const c_char {
    // SAFETY: A non-null pointer names a live error owned by the caller.
    unsafe { error.as_ref() }.map_or(std::ptr::null(), |error| error.message.as_ptr())
}

/// Destroys an error returned through Ward Core's private C interface.
///
/// # Safety
///
/// `error` must be null or a live error returned by this interface, and
/// ownership may be transferred only once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_error_destroy(error: *mut WardError) {
    if !error.is_null() {
        // SAFETY: The caller transfers an error handle exactly once.
        drop(unsafe { Box::from_raw(error) });
    }
}
