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

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct WardVzMacOsVirtualMachineStatus {
    pub(super) state: i32,
    pub(super) can_start: bool,
    pub(super) can_pause: bool,
    pub(super) can_resume: bool,
    pub(super) can_request_stop: bool,
    pub(super) can_force_stop: bool,
    pub(super) can_suspend: bool,
    pub(super) can_restore: bool,
    pub(super) can_discard_saved_state: bool,
}

#[repr(C)]
pub(super) struct WardVzMacOsVirtualMachine {
    _private: [u8; 0],
}

#[repr(C)]
pub(super) struct WardVzMacOsVirtualMachineDisplay {
    _private: [u8; 0],
}

pub(super) type WardVzMacOsVirtualMachineEvent = unsafe extern "C" fn(
    context: *mut c_void,
    status: *const WardVzMacOsVirtualMachineStatus,
    error: *const WardVzError,
);

pub(super) type WardVzCreateMacOsVirtualMachineCompletion = unsafe extern "C" fn(
    context: *mut c_void,
    virtual_machine: *mut WardVzMacOsVirtualMachine,
    status: *const WardVzMacOsVirtualMachineStatus,
    error: *const WardVzError,
);

pub(super) type WardVzCreateMacOsVirtualMachineDisplayCompletion = unsafe extern "C" fn(
    context: *mut c_void,
    display: *mut WardVzMacOsVirtualMachineDisplay,
    native_view: *mut c_void,
    error: *const WardVzError,
);

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

    pub(super) fn ward_vz_create_macos_virtual_machine(
        bundle_path: *const c_char,
        event: WardVzMacOsVirtualMachineEvent,
        event_context: *mut c_void,
        completion: WardVzCreateMacOsVirtualMachineCompletion,
        completion_context: *mut c_void,
    );

    pub(super) fn ward_vz_destroy_macos_virtual_machine(
        virtual_machine: *mut WardVzMacOsVirtualMachine,
    );

    pub(super) fn ward_vz_start_macos_virtual_machine(
        virtual_machine: *mut WardVzMacOsVirtualMachine,
    );

    pub(super) fn ward_vz_pause_macos_virtual_machine(
        virtual_machine: *mut WardVzMacOsVirtualMachine,
    );

    pub(super) fn ward_vz_resume_macos_virtual_machine(
        virtual_machine: *mut WardVzMacOsVirtualMachine,
    );

    pub(super) fn ward_vz_request_stop_macos_virtual_machine(
        virtual_machine: *mut WardVzMacOsVirtualMachine,
    );

    pub(super) fn ward_vz_force_stop_macos_virtual_machine(
        virtual_machine: *mut WardVzMacOsVirtualMachine,
    );

    pub(super) fn ward_vz_suspend_macos_virtual_machine(
        virtual_machine: *mut WardVzMacOsVirtualMachine,
    );

    pub(super) fn ward_vz_restore_macos_virtual_machine(
        virtual_machine: *mut WardVzMacOsVirtualMachine,
    );

    pub(super) fn ward_vz_discard_macos_virtual_machine_saved_state(
        virtual_machine: *mut WardVzMacOsVirtualMachine,
    );

    pub(super) fn ward_vz_create_macos_virtual_machine_display(
        virtual_machine: *mut WardVzMacOsVirtualMachine,
        completion: WardVzCreateMacOsVirtualMachineDisplayCompletion,
        context: *mut c_void,
    );

    pub(super) fn ward_vz_destroy_macos_virtual_machine_display(
        display: *mut WardVzMacOsVirtualMachineDisplay,
    );
}
