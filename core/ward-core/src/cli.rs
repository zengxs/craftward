// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{CStr, OsString, c_char, c_int};

use ward_cli::CliDisposition;

/// The result of dispatching process arguments through Ward's embedded CLI.
#[repr(C)]
pub struct WardCliResult {
    handled: bool,
    exit_code: c_int,
}

/// Dispatches process arguments through Ward's embedded CLI.
///
/// An invocation without arguments is left unhandled so the caller can start
/// its graphical interface. Every other invocation is handled as a CLI request
/// and returns the corresponding process exit code.
///
/// # Safety
///
/// `argv` must contain `argc` pointers to NUL-terminated strings when `argc`
/// is positive. The strings must remain valid until this function returns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_cli_try_run(
    argc: c_int,
    argv: *mut *mut c_char,
) -> WardCliResult {
    if argc <= 0 || argv.is_null() {
        return WardCliResult {
            handled: false,
            exit_code: 0,
        };
    }

    // SAFETY: The caller guarantees that `argv` contains `argc` pointers.
    let raw_arguments = unsafe { std::slice::from_raw_parts(argv, argc as usize) };
    let mut arguments = Vec::with_capacity(raw_arguments.len());
    for &argument in raw_arguments {
        if argument.is_null() {
            return WardCliResult {
                handled: true,
                exit_code: 2,
            };
        }
        // SAFETY: Each non-null argument is NUL-terminated for the duration of
        // this function call.
        arguments.push(os_string(unsafe { CStr::from_ptr(argument) }));
    }

    match ward_cli::try_run(arguments) {
        CliDisposition::NotRequested => WardCliResult {
            handled: false,
            exit_code: 0,
        },
        CliDisposition::Exit(exit_code) => WardCliResult {
            handled: true,
            exit_code,
        },
    }
}

#[cfg(unix)]
fn os_string(value: &CStr) -> OsString {
    use std::os::unix::ffi::OsStringExt;

    OsString::from_vec(value.to_bytes().to_vec())
}

#[cfg(not(unix))]
fn os_string(value: &CStr) -> OsString {
    OsString::from(value.to_string_lossy().into_owned())
}
