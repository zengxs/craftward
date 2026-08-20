// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

mod ffi;
mod macos;

pub use ffi::*;
pub use macos::{
    DEFAULT_MACOS_DISK_SIZE, MacOsBundleInfo, MacOsBundleInstallationError,
    MacOsBundleInstallationRequest, MacOsBundlePreparationError, MacOsBundleRequest, MacOsVersion,
    install_macos_bundle, prepare_macos_bundle,
};
