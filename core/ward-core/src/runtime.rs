// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use ward_runtime::WardRuntime as RuntimeOwner;

use crate::{WardError, clear_error, write_error};

/// An opaque owner of Ward Core's process-wide asynchronous runtime.
pub struct WardRuntime {
    owner: RuntimeOwner,
}

impl WardRuntime {
    pub(crate) fn handle(&self) -> tokio::runtime::Handle {
        self.owner.handle()
    }
}

/// Creates the asynchronous runtime used by Ward Core handles.
///
/// # Safety
///
/// `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_runtime_create(
    output_error: *mut *mut WardError,
) -> *mut WardRuntime {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };

    match RuntimeOwner::new() {
        Ok(owner) => Box::into_raw(Box::new(WardRuntime { owner })),
        Err(error) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe {
                write_error(
                    output_error,
                    format!("failed to start the Ward async runtime: {error}"),
                )
            };
            std::ptr::null_mut()
        }
    }
}

/// Shuts down and destroys a Ward asynchronous runtime.
///
/// Every handle created with this runtime must be destroyed first.
/// This function must be called outside the runtime's worker threads.
///
/// # Safety
///
/// `runtime` must be null or a live handle returned by
/// [`ward_core_runtime_create`], and ownership may be transferred only once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_runtime_destroy(runtime: *mut WardRuntime) {
    if !runtime.is_null() {
        // SAFETY: The caller transfers the live handle exactly once.
        drop(unsafe { Box::from_raw(runtime) });
    }
}
