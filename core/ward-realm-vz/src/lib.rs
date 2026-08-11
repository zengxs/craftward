// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(target_os = "macos")]
mod ffi;
mod macos;

pub use macos::{
    DEFAULT_MACOS_DISK_SIZE, MacOsBundleInfo, MacOsBundleInstallationError,
    MacOsBundleInstallationRequest, MacOsBundlePreparationError, MacOsBundleRequest, MacOsVersion,
    MacOsVirtualMachine, MacOsVirtualMachineDisplay, MacOsVirtualMachineError,
    MacOsVirtualMachineEvent, MacOsVirtualMachineState, MacOsVirtualMachineStatus,
    install_macos_bundle, is_supported, prepare_macos_bundle,
};
