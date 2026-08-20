// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{CStr, c_char, c_void};
use std::path::PathBuf;

use ward_realm_vz::{
    MacOsVirtualMachine, MacOsVirtualMachineDisplay, MacOsVirtualMachineEvent,
    MacOsVirtualMachineState, MacOsVirtualMachineStatus,
};

use super::error::{WardError, c_string, clear_error, write_error};
use crate::macos::open_macos_realm;

/// A Realm lifecycle state passed through Ward Core's private C interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub enum WardRealmState {
    Stopped = 0,
    Running = 1,
    Paused = 2,
    Error = 3,
    Starting = 4,
    Pausing = 5,
    Resuming = 6,
    Stopping = 7,
    Saving = 8,
    Restoring = 9,
    Suspended = 10,
}

/// A lifecycle snapshot passed through Ward Core's private C interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub struct WardRealmStatus {
    pub state: WardRealmState,
    pub can_start: bool,
    pub can_pause: bool,
    pub can_resume: bool,
    pub can_request_stop: bool,
    pub can_force_stop: bool,
    pub can_suspend: bool,
    pub can_restore: bool,
    pub can_discard_saved_state: bool,
}

/// A borrowed Realm lifecycle event passed to a Realm callback.
///
/// The event and optional error message remain valid only until the callback
/// returns. The callback must copy anything it needs to retain.
#[repr(C)]
pub struct WardRealmEvent {
    pub status: WardRealmStatus,
    pub error_message: *const c_char,
}

/// An opaque realm handle passed through Ward Core's private C interface.
pub struct WardRealm {
    display: Option<MacOsVirtualMachineDisplay>,
    virtual_machine: MacOsVirtualMachine,
}

/// Receives borrowed Realm lifecycle events.
///
/// The callback may run on any thread and may run before
/// [`ward_core_realm_open`] returns. `context` must remain valid until
/// [`ward_core_realm_destroy`] returns. The callback must not destroy its Realm
/// handle and must not unwind across the C interface.
pub type WardRealmEventCallback =
    Option<unsafe extern "C" fn(context: *mut c_void, event: *const WardRealmEvent)>;

type WardRealmEventFn = unsafe extern "C" fn(context: *mut c_void, event: *const WardRealmEvent);

struct RealmEventSink {
    callback: WardRealmEventFn,
    context: *mut c_void,
}

// SAFETY: The C consumer promises that its callback context remains valid
// until `ward_core_realm_destroy` returns. The callback decides how to marshal
// the event onto its own thread.
unsafe impl Send for RealmEventSink {}

impl RealmEventSink {
    fn emit(&self, event: MacOsVirtualMachineEvent) {
        let status = ward_realm_status(event.status);
        let error_message = event.error.map(|error| c_string(error.to_string()));
        let event = WardRealmEvent {
            status,
            error_message: error_message
                .as_ref()
                .map_or(std::ptr::null(), |message| message.as_ptr()),
        };

        // SAFETY: The event and optional error string remain valid for this
        // callback. The C consumer owns its context for the Realm's lifetime.
        unsafe { (self.callback)(self.context, &raw const event) };
    }
}

fn ward_realm_status(status: MacOsVirtualMachineStatus) -> WardRealmStatus {
    let state = match status.state {
        MacOsVirtualMachineState::Stopped => WardRealmState::Stopped,
        MacOsVirtualMachineState::Running => WardRealmState::Running,
        MacOsVirtualMachineState::Paused => WardRealmState::Paused,
        MacOsVirtualMachineState::Error => WardRealmState::Error,
        MacOsVirtualMachineState::Starting => WardRealmState::Starting,
        MacOsVirtualMachineState::Pausing => WardRealmState::Pausing,
        MacOsVirtualMachineState::Resuming => WardRealmState::Resuming,
        MacOsVirtualMachineState::Stopping => WardRealmState::Stopping,
        MacOsVirtualMachineState::Saving => WardRealmState::Saving,
        MacOsVirtualMachineState::Restoring => WardRealmState::Restoring,
        MacOsVirtualMachineState::Suspended => WardRealmState::Suspended,
        _ => WardRealmState::Error,
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

/// Opens an installed realm bundle without starting it.
///
/// The event callback may run on any thread and may run before this function
/// returns. Its borrowed event remains valid only for the duration of each
/// call. The callback context must remain valid until
/// [`ward_core_realm_destroy`] returns, after which no more callbacks occur. If
/// this function returns null, no callback occurs after the function returns.
///
/// # Safety
///
/// `bundle_path` must point to a NUL-terminated string. `output_error`, when
/// non-null, must be writable. The callback and its context must satisfy the
/// lifetime requirements described above.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_open(
    bundle_path: *const c_char,
    callback: WardRealmEventCallback,
    callback_context: *mut c_void,
    output_error: *mut *mut WardError,
) -> *mut WardRealm {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    let Some(callback) = callback else {
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
        callback,
        context: callback_context,
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

unsafe fn enqueue_realm_command(
    realm: *mut WardRealm,
    output_error: *mut *mut WardError,
    command: fn(&MacOsVirtualMachine),
) -> bool {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    // SAFETY: A non-null pointer names a live handle owned by the caller.
    let Some(realm) = (unsafe { realm.as_ref() }) else {
        // SAFETY: The caller supplied the optional output pointer.
        unsafe { write_error(output_error, "the realm handle is missing") };
        return false;
    };
    command(&realm.virtual_machine);
    true
}

/// Queues a start command for a Realm.
///
/// A `true` return means the command was accepted; its final outcome is
/// delivered through the Realm callback. A `false` return means immediate
/// rejection, and `output_error` receives an owned error when non-null.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`]. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_start_async(
    realm: *mut WardRealm,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller satisfies the documented handle and output contract.
    unsafe { enqueue_realm_command(realm, output_error, MacOsVirtualMachine::start) }
}

/// Queues a pause command for a Realm.
///
/// A `true` return means the command was accepted; its final outcome is
/// delivered through the Realm callback. A `false` return means immediate
/// rejection, and `output_error` receives an owned error when non-null.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`]. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_pause_async(
    realm: *mut WardRealm,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller satisfies the documented handle and output contract.
    unsafe { enqueue_realm_command(realm, output_error, MacOsVirtualMachine::pause) }
}

/// Queues a resume command for a Realm.
///
/// A `true` return means the command was accepted; its final outcome is
/// delivered through the Realm callback. A `false` return means immediate
/// rejection, and `output_error` receives an owned error when non-null.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`]. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_resume_async(
    realm: *mut WardRealm,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller satisfies the documented handle and output contract.
    unsafe { enqueue_realm_command(realm, output_error, MacOsVirtualMachine::resume) }
}

/// Queues an orderly shutdown request for a Realm guest.
///
/// A `true` return means the command was accepted; its final outcome is
/// delivered through the Realm callback. A `false` return means immediate
/// rejection, and `output_error` receives an owned error when non-null.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`]. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_request_stop_async(
    realm: *mut WardRealm,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller satisfies the documented handle and output contract.
    unsafe { enqueue_realm_command(realm, output_error, MacOsVirtualMachine::request_stop) }
}

/// Queues a destructive stop command for a Realm.
///
/// A `true` return means the command was accepted; its final outcome is
/// delivered through the Realm callback. A `false` return means immediate
/// rejection, and `output_error` receives an owned error when non-null.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`]. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_force_stop_async(
    realm: *mut WardRealm,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller satisfies the documented handle and output contract.
    unsafe { enqueue_realm_command(realm, output_error, MacOsVirtualMachine::force_stop) }
}

/// Queues a command to save a Realm's runtime state and release its resources.
///
/// A `true` return means the command was accepted; its final outcome is
/// delivered through the Realm callback. A `false` return means immediate
/// rejection, and `output_error` receives an owned error when non-null.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`]. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_suspend_async(
    realm: *mut WardRealm,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller satisfies the documented handle and output contract.
    unsafe { enqueue_realm_command(realm, output_error, MacOsVirtualMachine::suspend) }
}

/// Queues a command to restore and resume a Realm from saved runtime state.
///
/// A `true` return means the command was accepted; its final outcome is
/// delivered through the Realm callback. A `false` return means immediate
/// rejection, and `output_error` receives an owned error when non-null.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`]. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_restore_async(
    realm: *mut WardRealm,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller satisfies the documented handle and output contract.
    unsafe { enqueue_realm_command(realm, output_error, MacOsVirtualMachine::restore) }
}

/// Queues a command to discard a stopped Realm's saved runtime state.
///
/// A `true` return means the command was accepted; its final outcome is
/// delivered through the Realm callback. A `false` return means immediate
/// rejection, and `output_error` receives an owned error when non-null.
///
/// # Safety
///
/// `realm` must be null or a live handle returned by
/// [`ward_core_realm_open`]. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_realm_discard_saved_state_async(
    realm: *mut WardRealm,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller satisfies the documented handle and output contract.
    unsafe {
        enqueue_realm_command(
            realm,
            output_error,
            MacOsVirtualMachine::discard_saved_state,
        )
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
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
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

#[cfg(test)]
mod tests {
    use std::ffi::{CStr, c_void};

    use ward_realm_vz::{MacOsVirtualMachineError, MacOsVirtualMachineEvent};

    use super::super::error::{WardError, ward_core_error_destroy, ward_core_error_message};
    use super::{
        RealmEventSink, WardRealm, WardRealmEvent, WardRealmState,
        ward_core_realm_discard_saved_state_async, ward_core_realm_force_stop_async,
        ward_core_realm_pause_async, ward_core_realm_request_stop_async,
        ward_core_realm_restore_async, ward_core_realm_resume_async, ward_core_realm_start_async,
        ward_core_realm_suspend_async,
    };

    struct CapturedRealmEvent {
        state: WardRealmState,
        error_message: String,
    }

    unsafe extern "C" fn capture_realm_event(context: *mut c_void, event: *const WardRealmEvent) {
        // SAFETY: The test passes live pointers to both values for the duration
        // of this callback.
        let captured = unsafe { &mut *context.cast::<Option<CapturedRealmEvent>>() };
        // SAFETY: The Realm event sink supplies a live borrowed event.
        let event = unsafe { &*event };
        let error_message = if event.error_message.is_null() {
            String::new()
        } else {
            // SAFETY: The Realm event sink keeps the NUL-terminated message
            // alive until this callback returns.
            unsafe { CStr::from_ptr(event.error_message) }
                .to_string_lossy()
                .into_owned()
        };
        *captured = Some(CapturedRealmEvent {
            state: event.status.state,
            error_message,
        });
    }

    #[test]
    fn realm_event_callback_receives_a_borrowed_error_payload() {
        let mut captured: Option<CapturedRealmEvent> = None;
        let sink = RealmEventSink {
            callback: capture_realm_event,
            context: std::ptr::from_mut(&mut captured).cast(),
        };

        sink.emit(MacOsVirtualMachineEvent {
            status: ward_realm_vz::MacOsVirtualMachineStatus {
                state: ward_realm_vz::MacOsVirtualMachineState::Error,
                can_start: false,
                can_pause: false,
                can_resume: false,
                can_request_stop: false,
                can_force_stop: true,
                can_suspend: false,
                can_restore: false,
                can_discard_saved_state: false,
            },
            error: Some(MacOsVirtualMachineError::Native {
                domain: "app.craftward.tests".to_owned(),
                code: 7,
                message: "the Realm command failed".to_owned(),
            }),
        });

        let captured = captured.expect("the Realm callback should receive the event");
        assert_eq!(captured.state, WardRealmState::Error);
        assert_eq!(
            captured.error_message,
            "the Realm command failed (app.craftward.tests, code 7)"
        );
    }

    #[test]
    fn realm_async_commands_return_owned_immediate_errors() {
        type RealmCommand = unsafe extern "C" fn(*mut WardRealm, *mut *mut WardError) -> bool;
        let commands: [RealmCommand; 8] = [
            ward_core_realm_start_async,
            ward_core_realm_pause_async,
            ward_core_realm_resume_async,
            ward_core_realm_request_stop_async,
            ward_core_realm_force_stop_async,
            ward_core_realm_suspend_async,
            ward_core_realm_restore_async,
            ward_core_realm_discard_saved_state_async,
        ];

        for command in commands {
            let mut error = std::ptr::null_mut();
            // SAFETY: A null Realm is an explicitly supported rejection case,
            // and the error output pointer is writable.
            assert!(!unsafe { command(std::ptr::null_mut(), &mut error) });
            assert!(!error.is_null());
            // SAFETY: The command returned a live owned Ward error.
            let message = unsafe { ward_core_error_message(error) };
            assert!(!message.is_null());
            // SAFETY: The message is borrowed from the live error and is
            // NUL-terminated.
            assert_eq!(
                unsafe { CStr::from_ptr(message) }.to_string_lossy(),
                "the realm handle is missing"
            );
            // SAFETY: The test transfers each returned error exactly once.
            unsafe { ward_core_error_destroy(error) };
        }
    }
}
