// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{c_char, c_void};

#[repr(C)]
pub(super) struct WardVzMacOsBundleInfo {
    pub(super) build_version: *const c_char,
    pub(super) os_version_major: u64,
    pub(super) os_version_minor: u64,
    pub(super) os_version_patch: u64,
    pub(super) minimum_cpu_count: u64,
    pub(super) minimum_memory_size: u64,
    pub(super) disk_size: u64,
}

#[repr(C)]
pub(super) struct WardVzError {
    pub(super) domain: *const c_char,
    pub(super) code: i64,
    pub(super) message: *const c_char,
}

pub(super) type WardVzPrepareMacOsBundleCompletion = unsafe extern "C" fn(
    context: *mut c_void,
    bundle_info: *const WardVzMacOsBundleInfo,
    error: *const WardVzError,
);

pub(super) type WardVzMacOsInstallationProgress =
    unsafe extern "C" fn(context: *mut c_void, fraction_completed: f64);

pub(super) type WardVzInstallMacOsBundleCompletion =
    unsafe extern "C" fn(context: *mut c_void, error: *const WardVzError);

unsafe extern "C" {
    pub(super) fn ward_vz_is_supported() -> bool;

    pub(super) fn ward_vz_prepare_macos_bundle(
        restore_image_path: *const c_char,
        destination_path: *const c_char,
        disk_size: u64,
        completion: WardVzPrepareMacOsBundleCompletion,
        context: *mut c_void,
    );

    pub(super) fn ward_vz_install_macos_bundle(
        restore_image_path: *const c_char,
        bundle_path: *const c_char,
        progress: WardVzMacOsInstallationProgress,
        completion: WardVzInstallMacOsBundleCompletion,
        context: *mut c_void,
    );
}
