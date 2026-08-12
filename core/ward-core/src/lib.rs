// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

mod cli;
mod macos;

use std::ffi::{CStr, CString, c_char, c_void};
use std::path::PathBuf;

use macos::open_macos_realm;
pub use macos::{
    DEFAULT_MACOS_DISK_SIZE, MacOsBundleInfo, MacOsBundleInstallationError,
    MacOsBundleInstallationRequest, MacOsBundlePreparationError, MacOsBundleRequest, MacOsVersion,
    install_macos_bundle, prepare_macos_bundle,
};
use ward_realm_vz::{
    MacOsVirtualMachine, MacOsVirtualMachineDisplay, MacOsVirtualMachineEvent,
    MacOsVirtualMachineState, MacOsVirtualMachineStatus,
};

const WARD_REALM_STATE_STOPPED: i32 = 0;
const WARD_REALM_STATE_RUNNING: i32 = 1;
const WARD_REALM_STATE_PAUSED: i32 = 2;
const WARD_REALM_STATE_ERROR: i32 = 3;
const WARD_REALM_STATE_STARTING: i32 = 4;
const WARD_REALM_STATE_PAUSING: i32 = 5;
const WARD_REALM_STATE_RESUMING: i32 = 6;
const WARD_REALM_STATE_STOPPING: i32 = 7;
const WARD_REALM_STATE_SAVING: i32 = 8;
const WARD_REALM_STATE_RESTORING: i32 = 9;
const WARD_REALM_STATE_SUSPENDED: i32 = 10;

/// A lifecycle snapshot passed through Ward Core's private C interface.
#[repr(C)]
pub struct WardRealmStatus {
    state: i32,
    can_start: bool,
    can_pause: bool,
    can_resume: bool,
    can_request_stop: bool,
    can_force_stop: bool,
    can_suspend: bool,
    can_restore: bool,
    can_discard_saved_state: bool,
}

/// An opaque realm handle passed through Ward Core's private C interface.
pub struct WardRealm {
    display: Option<MacOsVirtualMachineDisplay>,
    virtual_machine: MacOsVirtualMachine,
}

/// An owned error passed through Ward Core's private C interface.
pub struct WardError {
    message: CString,
}

type WardRealmEvent = unsafe extern "C" fn(
    context: *mut c_void,
    status: *const WardRealmStatus,
    error_message: *const c_char,
);

struct RealmEventSink {
    event: WardRealmEvent,
    context: *mut c_void,
}

// SAFETY: The C consumer promises that its callback context remains valid
// until `ward_core_realm_destroy` returns. The callback decides how to marshal
// the event onto its own thread.
unsafe impl Send for RealmEventSink {}

impl RealmEventSink {
    fn emit(&self, event: MacOsVirtualMachineEvent) {
        let status = ward_realm_status(event.status);
        let error_message = event
            .error
            .map(|error| c_string(error.to_string()))
            .map_or(std::ptr::null(), |message| message.into_raw().cast_const());

        // SAFETY: The status and optional error string remain valid for this
        // callback. The C consumer owns its context for the realm's lifetime.
        unsafe { (self.event)(self.context, &raw const status, error_message) };

        if !error_message.is_null() {
            // SAFETY: `error_message` was allocated with `CString::into_raw`
            // immediately above and the callback has returned.
            drop(unsafe { CString::from_raw(error_message.cast_mut()) });
        }
    }
}

fn ward_realm_status(status: MacOsVirtualMachineStatus) -> WardRealmStatus {
    let state = match status.state {
        MacOsVirtualMachineState::Stopped => WARD_REALM_STATE_STOPPED,
        MacOsVirtualMachineState::Running => WARD_REALM_STATE_RUNNING,
        MacOsVirtualMachineState::Paused => WARD_REALM_STATE_PAUSED,
        MacOsVirtualMachineState::Error => WARD_REALM_STATE_ERROR,
        MacOsVirtualMachineState::Starting => WARD_REALM_STATE_STARTING,
        MacOsVirtualMachineState::Pausing => WARD_REALM_STATE_PAUSING,
        MacOsVirtualMachineState::Resuming => WARD_REALM_STATE_RESUMING,
        MacOsVirtualMachineState::Stopping => WARD_REALM_STATE_STOPPING,
        MacOsVirtualMachineState::Saving => WARD_REALM_STATE_SAVING,
        MacOsVirtualMachineState::Restoring => WARD_REALM_STATE_RESTORING,
        MacOsVirtualMachineState::Suspended => WARD_REALM_STATE_SUSPENDED,
        _ => WARD_REALM_STATE_ERROR,
    };

    WardRealmStatus {
        state,
        can_start: status.can_start,
        can_pause: status.can_pause,
        can_resume: status.can_resume,
        can_request_stop: status.can_request_stop,
        can_force_stop: status.can_force_stop,
        can_suspend: status.can_suspend,
        can_restore: status.can_restore,
        can_discard_saved_state: status.can_discard_saved_state,
    }
}

fn c_string(value: impl AsRef<str>) -> CString {
    let value = value.as_ref().replace('\0', "�");
    CString::new(value).expect("NUL bytes were replaced before constructing the C string")
}

unsafe fn write_error(output: *mut *mut WardError, message: impl AsRef<str>) {
    if output.is_null() {
        return;
    }

    let error = Box::new(WardError {
        message: c_string(message),
    });
    // SAFETY: The caller provided a writable output pointer and takes
    // ownership of the resulting WardError.
    unsafe { *output = Box::into_raw(error) };
}

/// Opens an installed realm bundle without starting it.
///
/// The event callback may run on any thread and may run before this function
/// returns. Its string arguments are only valid for the duration of each call.
/// The callback context must remain valid until [`ward_core_realm_destroy`]
/// returns.
///
/// # Safety
///
/// `bundle_path` must point to a NUL-terminated string. `output_error`, when
/// non-null, must be writable. The callback and its context must satisfy the
/// lifetime requirements described above.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_open(
    bundle_path: *const c_char,
    event: Option<WardRealmEvent>,
    event_context: *mut c_void,
    output_error: *mut *mut WardError,
) -> *mut WardRealm {
    if !output_error.is_null() {
        // SAFETY: The non-null pointer is writable by the C caller.
        unsafe { *output_error = std::ptr::null_mut() };
    }
    let Some(event) = event else {
        // SAFETY: The caller supplied the optional output pointer.
        unsafe { write_error(output_error, "the realm event callback is missing") };
        return std::ptr::null_mut();
    };
    if bundle_path.is_null() {
        // SAFETY: The caller supplied the optional output pointer.
        unsafe { write_error(output_error, "the realm bundle path is missing") };
        return std::ptr::null_mut();
    }

    // SAFETY: The C interface requires a NUL-terminated path string.
    let bundle_path = unsafe { CStr::from_ptr(bundle_path) };
    let bundle_path = PathBuf::from(bundle_path.to_string_lossy().into_owned());
    let sink = RealmEventSink {
        event,
        context: event_context,
    };
    match open_macos_realm(bundle_path, move |event| sink.emit(event)) {
        Ok(virtual_machine) => Box::into_raw(Box::new(WardRealm {
            display: None,
            virtual_machine,
        })),
        Err(error) => {
            // SAFETY: The caller supplied the optional output pointer.
            unsafe { write_error(output_error, error.to_string()) };
            std::ptr::null_mut()
        }
    }
}

/// Stops callbacks and destroys a realm handle.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`], and ownership may be transferred only once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_destroy(realm: *mut WardRealm) {
    if !realm.is_null() {
        // SAFETY: The caller transfers the handle returned by
        // `ward_core_realm_open` exactly once.
        drop(unsafe { Box::from_raw(realm) });
    }
}

/// Enqueues a start command for a realm.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_start(realm: *mut WardRealm) {
    // SAFETY: A non-null pointer names a live handle owned by the caller.
    if let Some(realm) = unsafe { realm.as_ref() } {
        realm.virtual_machine.start();
    }
}

/// Enqueues a pause command for a realm.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_pause(realm: *mut WardRealm) {
    // SAFETY: A non-null pointer names a live handle owned by the caller.
    if let Some(realm) = unsafe { realm.as_ref() } {
        realm.virtual_machine.pause();
    }
}

/// Enqueues a resume command for a realm.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_resume(realm: *mut WardRealm) {
    // SAFETY: A non-null pointer names a live handle owned by the caller.
    if let Some(realm) = unsafe { realm.as_ref() } {
        realm.virtual_machine.resume();
    }
}

/// Requests an orderly shutdown from a realm's guest.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_request_stop(realm: *mut WardRealm) {
    // SAFETY: A non-null pointer names a live handle owned by the caller.
    if let Some(realm) = unsafe { realm.as_ref() } {
        realm.virtual_machine.request_stop();
    }
}

/// Enqueues a destructive stop command for a realm.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_force_stop(realm: *mut WardRealm) {
    // SAFETY: A non-null pointer names a live handle owned by the caller.
    if let Some(realm) = unsafe { realm.as_ref() } {
        realm.virtual_machine.force_stop();
    }
}

/// Saves a realm's runtime state and releases its virtual-machine resources.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_suspend(realm: *mut WardRealm) {
    // SAFETY: A non-null pointer names a live handle owned by the caller.
    if let Some(realm) = unsafe { realm.as_ref() } {
        realm.virtual_machine.suspend();
    }
}

/// Restores and resumes a realm from its saved runtime state.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_restore(realm: *mut WardRealm) {
    // SAFETY: A non-null pointer names a live handle owned by the caller.
    if let Some(realm) = unsafe { realm.as_ref() } {
        realm.virtual_machine.restore();
    }
}

/// Discards a stopped realm's saved runtime state.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_discard_saved_state(realm: *mut WardRealm) {
    // SAFETY: A non-null pointer names a live handle owned by the caller.
    if let Some(realm) = unsafe { realm.as_ref() } {
        realm.virtual_machine.discard_saved_state();
    }
}

/// Attaches a display frontend and returns its borrowed native view.
///
/// Repeated calls return the same view until
/// [`ward_core_realm_detach_display`] is called. The view remains owned by the
/// realm and must only be wrapped, never released, by the C consumer.
///
/// # Safety
///
/// `realm` must point to a live handle returned by [`ward_core_realm_open`].
/// This function must be called on the application's main thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_attach_display(
    realm: *mut WardRealm,
    output_error: *mut *mut WardError,
) -> *mut c_void {
    if !output_error.is_null() {
        // SAFETY: The non-null pointer is writable by the C caller.
        unsafe { *output_error = std::ptr::null_mut() };
    }
    // SAFETY: A non-null pointer names a live handle owned by the caller.
    let Some(realm) = (unsafe { realm.as_mut() }) else {
        // SAFETY: The caller supplied the optional output pointer.
        unsafe { write_error(output_error, "the realm handle is missing") };
        return std::ptr::null_mut();
    };

    if realm.display.is_none() {
        match realm.virtual_machine.create_display() {
            Ok(display) => realm.display = Some(display),
            Err(error) => {
                // SAFETY: The caller supplied the optional output pointer.
                unsafe { write_error(output_error, error.to_string()) };
                return std::ptr::null_mut();
            }
        }
    }

    realm.display.as_ref().map_or(
        std::ptr::null_mut(),
        MacOsVirtualMachineDisplay::native_view,
    )
}

/// Detaches and destroys a realm's display frontend.
///
/// The C consumer must first destroy every UI wrapper around the borrowed
/// native view returned by [`ward_core_realm_attach_display`].
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`]. This function must be called on the application's
/// main thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_detach_display(realm: *mut WardRealm) {
    // SAFETY: A non-null pointer names a live handle owned by the caller.
    if let Some(realm) = unsafe { realm.as_mut() } {
        realm.display = None;
    }
}

/// Returns a borrowed error message owned by `error`.
///
/// # Safety
///
/// `error` must be null or a live error returned by this interface.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_error_message(error: *const WardError) -> *const c_char {
    // SAFETY: A non-null pointer names a live error owned by the caller.
    unsafe { error.as_ref() }.map_or(std::ptr::null(), |error| error.message.as_ptr())
}

/// Destroys an error returned through Ward Core's private C interface.
///
/// # Safety
///
/// `error` must be null or a live error returned by this interface, and
/// ownership may be transferred only once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_error_destroy(error: *mut WardError) {
    if !error.is_null() {
        // SAFETY: The caller transfers an error handle exactly once.
        drop(unsafe { Box::from_raw(error) });
    }
}
