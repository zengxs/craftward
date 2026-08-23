// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

/// An opaque borrowed serialized payload passed through Ward Core's private C interface.
pub struct WardBuffer {
    pub(super) bytes: Box<[u8]>,
}

/// An opaque owned serialized payload returned through Ward Core's app-only interface.
#[cfg(feature = "app")]
pub struct WardOwnedBuffer {
    bytes: Box<[u8]>,
}

#[cfg(feature = "app")]
impl WardOwnedBuffer {
    pub(super) fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes: bytes.into_boxed_slice(),
        }
    }
}

/// Returns the borrowed bytes in a serialized Ward buffer.
///
/// The returned pointer remains valid for the lifetime of the borrowed buffer.
///
/// # Safety
///
/// `buffer` must be null or a valid borrowed buffer supplied by Ward Core.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_buffer_data(buffer: *const WardBuffer) -> *const u8 {
    // SAFETY: A non-null pointer names a valid borrowed buffer.
    unsafe { buffer.as_ref() }.map_or(std::ptr::null(), |buffer| buffer.bytes.as_ptr())
}

/// Returns the number of bytes in a serialized Ward buffer.
///
/// # Safety
///
/// `buffer` must be null or a valid borrowed buffer supplied by Ward Core.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_buffer_size(buffer: *const WardBuffer) -> usize {
    // SAFETY: A non-null pointer names a valid borrowed buffer.
    unsafe { buffer.as_ref() }.map_or(0, |buffer| buffer.bytes.len())
}

/// Returns the borrowed bytes in an owned Ward buffer.
///
/// The returned pointer remains valid until the owned buffer is destroyed.
///
/// # Safety
///
/// `buffer` must be null or a valid live owned buffer returned by Ward Core.
#[cfg(feature = "app")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_owned_buffer_data(buffer: *const WardOwnedBuffer) -> *const u8 {
    // SAFETY: A non-null pointer names a valid live owned buffer.
    unsafe { buffer.as_ref() }.map_or(std::ptr::null(), |buffer| buffer.bytes.as_ptr())
}

/// Returns the number of bytes in an owned Ward buffer.
///
/// # Safety
///
/// `buffer` must be null or a valid live owned buffer returned by Ward Core.
#[cfg(feature = "app")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_owned_buffer_size(buffer: *const WardOwnedBuffer) -> usize {
    // SAFETY: A non-null pointer names a valid live owned buffer.
    unsafe { buffer.as_ref() }.map_or(0, |buffer| buffer.bytes.len())
}

/// Destroys an owned Ward buffer returned directly from Ward Core.
///
/// # Safety
///
/// `buffer` must be null or a valid live owned buffer returned by Ward Core,
/// and ownership may be transferred only once.
#[cfg(feature = "app")]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_owned_buffer_destroy(buffer: *mut WardOwnedBuffer) {
    if !buffer.is_null() {
        // SAFETY: The caller transfers ownership of the buffer exactly once.
        drop(unsafe { Box::from_raw(buffer) });
    }
}
