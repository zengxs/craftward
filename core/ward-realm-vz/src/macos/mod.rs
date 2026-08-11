// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

mod configuration;
mod installer;
mod preparation;
mod vm;

pub use configuration::{
    MacOsDiskConfiguration, MacOsDisplayConfiguration, MacOsSavedStateFiles,
    MacOsVirtualMachineConfiguration,
};
#[cfg(target_os = "macos")]
use configuration::{NativeMacOsSavedStateFiles, NativeMacOsVirtualMachineConfiguration};
pub use installer::{MacOsInstallationError, MacOsInstallationRequest, install_macos};
pub use preparation::{
    MacOsPreparationError, MacOsPreparationInfo, MacOsPreparationRequest, MacOsVersion,
    prepare_macos,
};
pub use vm::{
    MacOsVirtualMachine, MacOsVirtualMachineDisplay, MacOsVirtualMachineError,
    MacOsVirtualMachineEvent, MacOsVirtualMachineState, MacOsVirtualMachineStatus,
};

#[cfg(target_os = "macos")]
fn path_to_c_string(
    path: &std::path::Path,
    name: &'static str,
    must_name_file: bool,
) -> Result<std::ffi::CString, &'static str> {
    use std::os::unix::ffi::OsStrExt;

    if !path.is_absolute()
        || path.as_os_str().is_empty()
        || (must_name_file && path.file_name().is_none())
    {
        return Err(name);
    }

    std::ffi::CString::new(path.as_os_str().as_bytes()).map_err(|_| name)
}

#[cfg(target_os = "macos")]
unsafe fn copy_bridge_string(value: *const std::ffi::c_char) -> String {
    if value.is_null() {
        return String::new();
    }

    // SAFETY: Bridge strings remain alive for the duration of their callback
    // and are terminated with a NUL byte.
    unsafe { std::ffi::CStr::from_ptr(value) }
        .to_string_lossy()
        .into_owned()
}

#[cfg(target_os = "macos")]
unsafe fn copy_bridge_bytes(value: crate::ffi::WardVzByteSlice) -> Option<Vec<u8>> {
    if value.length == 0 {
        return Some(Vec::new());
    }
    if value.data.is_null() {
        return None;
    }

    // SAFETY: The bridge keeps the byte slice alive for the duration of the
    // callback that supplied it.
    Some(unsafe { std::slice::from_raw_parts(value.data, value.length) }.to_vec())
}

/// Returns whether the current host supports Virtualization.framework virtual
/// machines.
#[cfg(target_os = "macos")]
pub fn is_supported() -> bool {
    // SAFETY: The bridge function accepts no arguments and returns a
    // C-compatible boolean value.
    unsafe { crate::ffi::ward_vz_is_supported() }
}

/// Returns whether the current host supports Virtualization.framework virtual
/// machines.
#[cfg(not(target_os = "macos"))]
pub const fn is_supported() -> bool {
    false
}
