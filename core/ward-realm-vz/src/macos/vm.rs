// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use super::{copy_bridge_string, is_supported, path_to_c_string};
#[cfg(target_os = "macos")]
use crate::ffi;

/// The execution state of a Virtualization.framework macOS guest.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MacOsVirtualMachineState {
    Stopped,
    Running,
    Paused,
    Error,
    Starting,
    Pausing,
    Resuming,
    Stopping,
    Saving,
    Restoring,
    Suspended,
}

/// A lifecycle snapshot reported by a macOS virtual machine.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MacOsVirtualMachineStatus {
    pub state: MacOsVirtualMachineState,
    pub can_start: bool,
    pub can_pause: bool,
    pub can_resume: bool,
    pub can_request_stop: bool,
    pub can_force_stop: bool,
    pub can_suspend: bool,
    pub can_restore: bool,
    pub can_discard_saved_state: bool,
}

/// An error encountered while opening or operating a macOS virtual machine.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum MacOsVirtualMachineError {
    /// The current host cannot run Virtualization.framework macOS guests.
    UnsupportedHost,
    /// The path was empty, relative, did not name a bundle, or contained a NUL
    /// byte.
    InvalidPath,
    /// The bundle, virtual machine configuration, or lifecycle operation was
    /// rejected.
    Native {
        domain: String,
        code: i64,
        message: String,
    },
}

impl fmt::Display for MacOsVirtualMachineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedHost => {
                formatter.write_str("this host cannot run a Virtualization.framework macOS guest")
            }
            Self::InvalidPath => formatter.write_str("the realm bundle path is invalid"),
            Self::Native {
                domain,
                code,
                message,
            } => write!(formatter, "{message} ({domain}, code {code})"),
        }
    }
}

impl std::error::Error for MacOsVirtualMachineError {}

/// A serialized lifecycle event from a macOS virtual machine.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MacOsVirtualMachineEvent {
    pub status: MacOsVirtualMachineStatus,
    pub error: Option<MacOsVirtualMachineError>,
}

#[cfg(target_os = "macos")]
type MacOsVirtualMachineEventHandler = Box<dyn FnMut(MacOsVirtualMachineEvent) + Send + 'static>;

#[cfg(target_os = "macos")]
struct MacOsVirtualMachineEventContext {
    handler: std::sync::Mutex<MacOsVirtualMachineEventHandler>,
}

#[cfg(target_os = "macos")]
impl MacOsVirtualMachineEventContext {
    fn emit(&self, event: MacOsVirtualMachineEvent) {
        use std::panic::{AssertUnwindSafe, catch_unwind};

        let Ok(mut handler) = self.handler.lock() else {
            return;
        };
        let _ = catch_unwind(AssertUnwindSafe(|| handler(event)));
    }
}

#[cfg(target_os = "macos")]
fn macos_virtual_machine_status_from_bridge(
    status: ffi::WardVzMacOsVirtualMachineStatus,
) -> MacOsVirtualMachineStatus {
    let state = match status.state {
        0 => MacOsVirtualMachineState::Stopped,
        1 => MacOsVirtualMachineState::Running,
        2 => MacOsVirtualMachineState::Paused,
        3 => MacOsVirtualMachineState::Error,
        4 => MacOsVirtualMachineState::Starting,
        5 => MacOsVirtualMachineState::Pausing,
        6 => MacOsVirtualMachineState::Resuming,
        7 => MacOsVirtualMachineState::Stopping,
        8 => MacOsVirtualMachineState::Saving,
        9 => MacOsVirtualMachineState::Restoring,
        10 => MacOsVirtualMachineState::Suspended,
        _ => MacOsVirtualMachineState::Error,
    };

    MacOsVirtualMachineStatus {
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

#[cfg(target_os = "macos")]
unsafe fn macos_virtual_machine_error_from_bridge(
    error: *const ffi::WardVzError,
) -> Option<MacOsVirtualMachineError> {
    if error.is_null() {
        return None;
    }

    // SAFETY: The bridge keeps the error and its strings alive for the
    // duration of the callback that calls this function.
    let error = unsafe { &*error };
    Some(MacOsVirtualMachineError::Native {
        // SAFETY: See the bridge error lifetime guarantee above.
        domain: unsafe { copy_bridge_string(error.domain) },
        code: error.code,
        // SAFETY: See the bridge error lifetime guarantee above.
        message: unsafe { copy_bridge_string(error.message) },
    })
}

/// A long-lived handle that owns a Virtualization.framework macOS guest.
///
/// All lifecycle operations are serialized by the adapter. The event handler
/// may run on an arbitrary thread, may run before [`Self::open`] returns, and
/// is never called after this handle has been dropped.
#[cfg(target_os = "macos")]
pub struct MacOsVirtualMachine {
    handle: std::ptr::NonNull<ffi::WardVzMacOsVirtualMachine>,
    event_context: std::sync::Arc<MacOsVirtualMachineEventContext>,
    native_event_context: std::ptr::NonNull<MacOsVirtualMachineEventContext>,
}

/// A main-thread display frontend attached to a macOS virtual machine.
///
/// The returned native view is borrowed from this handle. The handle must stay
/// alive until every UI wrapper around that view has been detached.
#[cfg(target_os = "macos")]
pub struct MacOsVirtualMachineDisplay {
    handle: std::ptr::NonNull<ffi::WardVzMacOsVirtualMachineDisplay>,
    native_view: std::ptr::NonNull<std::ffi::c_void>,
    _main_thread: std::marker::PhantomData<std::rc::Rc<()>>,
}

#[cfg(target_os = "macos")]
// SAFETY: The native adapter serializes every operation, callback, and
// destruction action on the virtual machine's private dispatch queue.
unsafe impl Send for MacOsVirtualMachine {}

#[cfg(target_os = "macos")]
// SAFETY: See the `Send` implementation. Shared references only enqueue
// operations and never access Virtualization.framework objects directly.
unsafe impl Sync for MacOsVirtualMachine {}

#[cfg(target_os = "macos")]
impl MacOsVirtualMachine {
    /// Opens an installed macOS realm bundle without starting its guest.
    pub fn open(
        bundle: impl Into<PathBuf>,
        event_handler: impl FnMut(MacOsVirtualMachineEvent) + Send + 'static,
    ) -> Result<Self, MacOsVirtualMachineError> {
        use std::ffi::c_void;
        use std::sync::Arc;

        struct CreationResult {
            handle: *mut ffi::WardVzMacOsVirtualMachine,
            status: Option<ffi::WardVzMacOsVirtualMachineStatus>,
            error: Option<MacOsVirtualMachineError>,
        }

        unsafe extern "C" fn virtual_machine_event(
            context: *mut c_void,
            status: *const ffi::WardVzMacOsVirtualMachineStatus,
            error: *const ffi::WardVzError,
        ) {
            if context.is_null() || status.is_null() {
                return;
            }

            let context = context.cast::<MacOsVirtualMachineEventContext>();
            // SAFETY: The native bridge holds one strong Arc reference until
            // destruction. Taking a temporary reference also makes dropping
            // the VM from inside its event handler safe.
            unsafe { Arc::increment_strong_count(context) };
            // SAFETY: The strong count was incremented immediately above.
            let context = unsafe { Arc::from_raw(context) };
            // SAFETY: A non-null status remains valid for this callback.
            let status = unsafe { *status };
            // SAFETY: A bridge error, when present, has the same lifetime.
            let error = unsafe { macos_virtual_machine_error_from_bridge(error) };
            context.emit(MacOsVirtualMachineEvent {
                status: macos_virtual_machine_status_from_bridge(status),
                error,
            });
        }

        unsafe extern "C" fn virtual_machine_created(
            context: *mut c_void,
            virtual_machine: *mut ffi::WardVzMacOsVirtualMachine,
            status: *const ffi::WardVzMacOsVirtualMachineStatus,
            error: *const ffi::WardVzError,
        ) {
            if context.is_null() {
                return;
            }

            // SAFETY: The context points to the stack-local CreationResult and
            // the bridge invokes this completion synchronously exactly once.
            let result = unsafe { &mut *context.cast::<CreationResult>() };
            result.handle = virtual_machine;
            result.status = if status.is_null() {
                None
            } else {
                // SAFETY: A non-null status remains valid for this callback.
                Some(unsafe { *status })
            };
            // SAFETY: A bridge error, when present, has the same lifetime.
            result.error = unsafe { macos_virtual_machine_error_from_bridge(error) };
        }

        let bundle = bundle.into();
        let bundle = path_to_c_string(&bundle, "bundle", true)
            .map_err(|_| MacOsVirtualMachineError::InvalidPath)?;
        if !is_supported() {
            return Err(MacOsVirtualMachineError::UnsupportedHost);
        }

        let event_context = Arc::new(MacOsVirtualMachineEventContext {
            handler: std::sync::Mutex::new(Box::new(event_handler)),
        });
        let native_event_context = Arc::into_raw(Arc::clone(&event_context));
        let mut creation = CreationResult {
            handle: std::ptr::null_mut(),
            status: None,
            error: None,
        };

        // SAFETY: The bridge copies the path, retains the event context until
        // explicit destruction, and completes synchronously before returning.
        unsafe {
            ffi::ward_vz_create_macos_virtual_machine(
                bundle.as_ptr(),
                virtual_machine_event,
                native_event_context.cast_mut().cast(),
                virtual_machine_created,
                (&raw mut creation).cast(),
            );
        }

        let Some(handle) = std::ptr::NonNull::new(creation.handle) else {
            // SAFETY: This balances the strong reference passed to the bridge,
            // which does not retain it after a failed creation.
            drop(unsafe { Arc::from_raw(native_event_context) });
            return Err(creation
                .error
                .unwrap_or_else(|| MacOsVirtualMachineError::Native {
                    domain: "app.craftward.ward-realm-vz.bridge".into(),
                    code: -1,
                    message: "the native bridge returned no virtual machine".into(),
                }));
        };
        let Some(status) = creation.status else {
            // SAFETY: A non-null handle was transferred to this caller.
            unsafe { ffi::ward_vz_destroy_macos_virtual_machine(handle.as_ptr()) };
            // SAFETY: Destruction has stopped native access to this reference.
            drop(unsafe { Arc::from_raw(native_event_context) });
            return Err(MacOsVirtualMachineError::Native {
                domain: "app.craftward.ward-realm-vz.bridge".into(),
                code: -1,
                message: "the native bridge returned no virtual machine status".into(),
            });
        };

        let virtual_machine = Self {
            handle,
            event_context,
            // SAFETY: Arc pointers are non-null.
            native_event_context: unsafe {
                std::ptr::NonNull::new_unchecked(native_event_context.cast_mut())
            },
        };
        virtual_machine
            .event_context
            .emit(MacOsVirtualMachineEvent {
                status: macos_virtual_machine_status_from_bridge(status),
                error: creation.error,
            });
        Ok(virtual_machine)
    }

    /// Starts the guest from its stopped state.
    pub fn start(&self) {
        // SAFETY: The handle remains valid for this method call.
        unsafe { ffi::ward_vz_start_macos_virtual_machine(self.handle.as_ptr()) };
    }

    /// Pauses a running guest while retaining its allocated memory.
    pub fn pause(&self) {
        // SAFETY: The handle remains valid for this method call.
        unsafe { ffi::ward_vz_pause_macos_virtual_machine(self.handle.as_ptr()) };
    }

    /// Resumes a paused guest.
    pub fn resume(&self) {
        // SAFETY: The handle remains valid for this method call.
        unsafe { ffi::ward_vz_resume_macos_virtual_machine(self.handle.as_ptr()) };
    }

    /// Requests an orderly shutdown from the guest operating system.
    pub fn request_stop(&self) {
        // SAFETY: The handle remains valid for this method call.
        unsafe { ffi::ward_vz_request_stop_macos_virtual_machine(self.handle.as_ptr()) };
    }

    /// Stops the guest without allowing its operating system to shut down.
    pub fn force_stop(&self) {
        // SAFETY: The handle remains valid for this method call.
        unsafe { ffi::ward_vz_force_stop_macos_virtual_machine(self.handle.as_ptr()) };
    }

    /// Saves the guest's paused runtime state and releases its resources.
    pub fn suspend(&self) {
        // SAFETY: The handle remains valid for this method call.
        unsafe { ffi::ward_vz_suspend_macos_virtual_machine(self.handle.as_ptr()) };
    }

    /// Restores and resumes a guest from its host-bound saved state.
    pub fn restore(&self) {
        // SAFETY: The handle remains valid for this method call.
        unsafe { ffi::ward_vz_restore_macos_virtual_machine(self.handle.as_ptr()) };
    }

    /// Discards a stopped guest's host-bound saved state.
    pub fn discard_saved_state(&self) {
        // SAFETY: The handle remains valid for this method call.
        unsafe { ffi::ward_vz_discard_macos_virtual_machine_saved_state(self.handle.as_ptr()) };
    }

    /// Attaches an interactive native display view to this virtual machine.
    ///
    /// This method must be called on the application's main thread. The
    /// returned display is intentionally neither `Send` nor `Sync` so that its
    /// native view is destroyed on the same thread.
    pub fn create_display(&self) -> Result<MacOsVirtualMachineDisplay, MacOsVirtualMachineError> {
        use std::ffi::c_void;

        struct CreationResult {
            display: *mut ffi::WardVzMacOsVirtualMachineDisplay,
            native_view: *mut c_void,
            error: Option<MacOsVirtualMachineError>,
        }

        unsafe extern "C" fn display_created(
            context: *mut c_void,
            display: *mut ffi::WardVzMacOsVirtualMachineDisplay,
            native_view: *mut c_void,
            error: *const ffi::WardVzError,
        ) {
            if context.is_null() {
                return;
            }

            // SAFETY: The context points to the stack-local CreationResult and
            // the bridge invokes this completion synchronously exactly once.
            let result = unsafe { &mut *context.cast::<CreationResult>() };
            result.display = display;
            result.native_view = native_view;
            // SAFETY: A bridge error, when present, remains valid for this
            // synchronous callback.
            result.error = unsafe { macos_virtual_machine_error_from_bridge(error) };
        }

        let mut creation = CreationResult {
            display: std::ptr::null_mut(),
            native_view: std::ptr::null_mut(),
            error: None,
        };
        // SAFETY: The machine handle remains valid and the bridge invokes the
        // completion synchronously before returning.
        unsafe {
            ffi::ward_vz_create_macos_virtual_machine_display(
                self.handle.as_ptr(),
                display_created,
                (&raw mut creation).cast(),
            );
        }

        if let Some(error) = creation.error {
            if !creation.display.is_null() {
                // SAFETY: A failed bridge call unexpectedly transferred a
                // display handle, so release it before returning the error.
                unsafe { ffi::ward_vz_destroy_macos_virtual_machine_display(creation.display) };
            }
            return Err(error);
        }

        let Some(handle) = std::ptr::NonNull::new(creation.display) else {
            return Err(MacOsVirtualMachineError::Native {
                domain: "app.craftward.ward-realm-vz.bridge".into(),
                code: -1,
                message: "the native bridge returned no virtual machine display".into(),
            });
        };
        let Some(native_view) = std::ptr::NonNull::new(creation.native_view) else {
            // SAFETY: A non-null handle was transferred to this caller.
            unsafe { ffi::ward_vz_destroy_macos_virtual_machine_display(handle.as_ptr()) };
            return Err(MacOsVirtualMachineError::Native {
                domain: "app.craftward.ward-realm-vz.bridge".into(),
                code: -1,
                message: "the native bridge returned no native display view".into(),
            });
        };

        Ok(MacOsVirtualMachineDisplay {
            handle,
            native_view,
            _main_thread: std::marker::PhantomData,
        })
    }
}

#[cfg(target_os = "macos")]
impl MacOsVirtualMachineDisplay {
    /// Returns the borrowed `NSView *` represented as an opaque pointer.
    #[must_use]
    pub fn native_view(&self) -> *mut std::ffi::c_void {
        self.native_view.as_ptr()
    }
}

#[cfg(target_os = "macos")]
impl Drop for MacOsVirtualMachineDisplay {
    fn drop(&mut self) {
        // SAFETY: This handle is uniquely owned and the type cannot be moved
        // away from the main thread where the native view was created.
        unsafe { ffi::ward_vz_destroy_macos_virtual_machine_display(self.handle.as_ptr()) };
    }
}

#[cfg(target_os = "macos")]
impl Drop for MacOsVirtualMachine {
    fn drop(&mut self) {
        // SAFETY: This handle is uniquely owned here. Destruction synchronizes
        // with the native queue and prevents future callbacks before returning.
        unsafe { ffi::ward_vz_destroy_macos_virtual_machine(self.handle.as_ptr()) };
        // SAFETY: This balances the strong reference transferred to the native
        // bridge after native access has ended.
        drop(unsafe { std::sync::Arc::from_raw(self.native_event_context.as_ptr()) });
    }
}

/// A macOS virtual machine cannot be constructed on non-macOS hosts.
#[cfg(not(target_os = "macos"))]
pub struct MacOsVirtualMachine {
    _private: (),
}

/// A macOS virtual-machine display cannot be created on non-macOS hosts.
#[cfg(not(target_os = "macos"))]
pub struct MacOsVirtualMachineDisplay {
    _private: (),
}

#[cfg(not(target_os = "macos"))]
impl MacOsVirtualMachine {
    /// Returns [`MacOsVirtualMachineError::UnsupportedHost`].
    pub fn open(
        _bundle: impl Into<PathBuf>,
        _event_handler: impl FnMut(MacOsVirtualMachineEvent) + Send + 'static,
    ) -> Result<Self, MacOsVirtualMachineError> {
        Err(MacOsVirtualMachineError::UnsupportedHost)
    }

    pub fn start(&self) {}

    pub fn pause(&self) {}

    pub fn resume(&self) {}

    pub fn request_stop(&self) {}

    pub fn force_stop(&self) {}

    pub fn suspend(&self) {}

    pub fn restore(&self) {}

    pub fn discard_saved_state(&self) {}

    pub fn create_display(&self) -> Result<MacOsVirtualMachineDisplay, MacOsVirtualMachineError> {
        Err(MacOsVirtualMachineError::UnsupportedHost)
    }
}

#[cfg(not(target_os = "macos"))]
impl MacOsVirtualMachineDisplay {
    #[must_use]
    pub fn native_view(&self) -> *mut std::ffi::c_void {
        std::ptr::null_mut()
    }
}
