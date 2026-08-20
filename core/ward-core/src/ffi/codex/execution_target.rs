// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::c_char;
use std::path::PathBuf;

use ward_codex::CodexAppServerSource;

use super::required_string;
use crate::ffi::error::{WardError, clear_error, write_error};

/// An opaque Codex execution target passed through Ward Core's private C
/// interface.
///
/// An execution target hides how independent Codex app-server connections are
/// opened. A host target starts a local executable; other target adapters can
/// provide the same connection source without changing observers.
pub struct WardCodexExecutionTarget {
    source: CodexAppServerSource,
}

impl WardCodexExecutionTarget {
    pub(super) fn source(&self) -> CodexAppServerSource {
        self.source.clone()
    }
}

/// Creates a host-backed Codex execution target.
///
/// The target starts `executable` in `app-server --stdio` mode whenever an
/// observer needs a new Codex connection. Creating the target does not start a
/// process.
///
/// # Safety
///
/// `executable` must point to a non-empty NUL-terminated string.
/// `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_execution_target_create_host(
    executable: *const c_char,
    output_error: *mut *mut WardError,
) -> *mut WardCodexExecutionTarget {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    // SAFETY: The private C interface requires the documented string pointer.
    let Some(executable) =
        (unsafe { required_string(executable, "the Codex executable", output_error) })
    else {
        return std::ptr::null_mut();
    };
    if executable.is_empty() {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex executable is empty") };
        return std::ptr::null_mut();
    }

    Box::into_raw(Box::new(WardCodexExecutionTarget {
        source: CodexAppServerSource::executable(PathBuf::from(executable)),
    }))
}

/// Destroys a Codex execution target.
///
/// Observers clone the target's connection source when they are opened, so an
/// execution target may be destroyed before observers created from it.
///
/// # Safety
///
/// `target` must be null or a live handle returned by a Codex execution-target
/// factory, and ownership may be transferred only once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_execution_target_destroy(
    target: *mut WardCodexExecutionTarget,
) {
    if !target.is_null() {
        // SAFETY: The caller transfers the live handle exactly once.
        drop(unsafe { Box::from_raw(target) });
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::{CStr, CString};

    use super::{
        ward_core_codex_execution_target_create_host, ward_core_codex_execution_target_destroy,
    };
    use crate::ffi::error::{ward_core_error_destroy, ward_core_error_message};

    #[test]
    fn creates_and_destroys_a_host_execution_target() {
        let executable = CString::new("codex").expect("the executable is valid");
        let mut error = std::ptr::null_mut();

        // SAFETY: The executable and error output remain valid for the call.
        let target = unsafe {
            ward_core_codex_execution_target_create_host(executable.as_ptr(), &mut error)
        };

        assert!(!target.is_null());
        assert!(error.is_null());
        // SAFETY: The target was returned above and ownership is transferred
        // exactly once.
        unsafe { ward_core_codex_execution_target_destroy(target) };
    }

    #[test]
    fn rejects_an_empty_host_executable() {
        let executable = CString::new("").expect("the empty executable is valid C text");
        let mut error = std::ptr::null_mut();

        // SAFETY: The executable and error output remain valid for the call.
        let target = unsafe {
            ward_core_codex_execution_target_create_host(executable.as_ptr(), &mut error)
        };

        assert!(target.is_null());
        assert!(!error.is_null());
        // SAFETY: The error and its message remain live until destruction.
        let message = unsafe { CStr::from_ptr(ward_core_error_message(error)) };
        assert_eq!(message.to_bytes(), b"the Codex executable is empty");
        // SAFETY: Ownership of the returned error is transferred exactly once.
        unsafe { ward_core_error_destroy(error) };
    }
}
