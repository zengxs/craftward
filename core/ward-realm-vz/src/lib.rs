// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(target_os = "macos")]
mod ffi;
mod macos;

pub use macos::{
    MacOsDiskConfiguration, MacOsDisplayConfiguration, MacOsInstallationError,
    MacOsInstallationRequest, MacOsPreparationError, MacOsPreparationInfo, MacOsPreparationRequest,
    MacOsSavedStateFiles, MacOsVersion, MacOsVirtualMachine, MacOsVirtualMachineConfiguration,
    MacOsVirtualMachineDisplay, MacOsVirtualMachineError, MacOsVirtualMachineEvent,
    MacOsVirtualMachineState, MacOsVirtualMachineStatus, install_macos, is_supported,
    prepare_macos,
};
