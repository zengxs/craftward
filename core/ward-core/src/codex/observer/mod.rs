// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{c_char, c_int, c_void};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use prost::Message as _;
use tokio::runtime::Handle;
use tokio::sync::mpsc::{self, Sender};
use tokio::task::JoinHandle;
use ward_codex::{
    CodexHistoryCancellation, InteractionResponse, TurnMode, TurnOptions, TurnPermissionPreset,
};

use self::commands::{ObserverCommand, TurnRequest};
use self::events::HistoryEventSink;
use self::worker::run_observer;
use super::{WardBuffer, clear_error, required_string, wire};
use crate::{WardError, write_error};

mod commands;
mod events;
mod worker;

#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

const COMMAND_QUEUE_CAPACITY: usize = 64;

type WardCodexHistoryEventCallback =
    unsafe extern "C" fn(context: *mut c_void, event: *const WardBuffer);

/// An opaque asynchronous Codex history observer passed through Ward Core's
/// private C interface.
pub struct WardCodexHistoryObserver {
    commands: Sender<ObserverCommand>,
    cancellation: CodexHistoryCancellation,
    turn_active: Arc<AtomicBool>,
    runtime: Handle,
    worker: Option<JoinHandle<()>>,
}

impl Drop for WardCodexHistoryObserver {
    fn drop(&mut self) {
        let _ = self.commands.try_send(ObserverCommand::Stop);
        self.cancellation.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = self.runtime.block_on(worker);
        }
    }
}

/// Starts a background observer for persisted Codex history.
///
/// The callback receives a borrowed serialized event buffer from the observer
/// thread. Its context must remain valid until
/// [`ward_core_codex_history_observer_destroy`] returns.
///
/// # Safety
///
/// `runtime` must point to a live Ward runtime that outlives the returned
/// observer. `executable` must point to a NUL-terminated string. `callback`
/// must be a valid function pointer. `output_error`, when non-null, must be
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_open(
    runtime: *const crate::WardRuntime,
    executable: *const c_char,
    callback: Option<WardCodexHistoryEventCallback>,
    callback_context: *mut c_void,
    output_error: *mut *mut WardError,
) -> *mut WardCodexHistoryObserver {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    let Some(runtime) = (unsafe { runtime.as_ref() }) else {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Ward async runtime is missing") };
        return std::ptr::null_mut();
    };
    // SAFETY: The private C interface requires the documented string pointer.
    let Some(executable) =
        (unsafe { required_string(executable, "the Codex executable", output_error) })
    else {
        return std::ptr::null_mut();
    };
    let Some(callback) = callback else {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex history callback is missing") };
        return std::ptr::null_mut();
    };

    let (commands, receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let cancellation = CodexHistoryCancellation::new();
    let turn_active = Arc::new(AtomicBool::new(false));
    let sink = HistoryEventSink::new(callback, callback_context);
    let runtime = runtime.handle();
    let worker = runtime.spawn({
        let cancellation = cancellation.clone();
        let turn_active = Arc::clone(&turn_active);
        async move {
            run_observer(
                PathBuf::from(executable),
                receiver,
                sink,
                cancellation,
                turn_active,
            )
            .await;
        }
    });

    Box::into_raw(Box::new(WardCodexHistoryObserver {
        commands,
        cancellation,
        turn_active,
        runtime,
        worker: Some(worker),
    }))
}

/// Selects the persisted thread observed by a Codex history observer.
///
/// The first successful read is emitted immediately as an updated event.
///
/// # Safety
///
/// `observer` must point to a live handle returned by
/// [`ward_core_codex_history_observer_open`]. `thread_id` must point to a
/// NUL-terminated string. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_watch(
    observer: *mut WardCodexHistoryObserver,
    thread_id: *const c_char,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    let Some(observer) = (unsafe { observer.as_ref() }) else {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex history observer is missing") };
        return false;
    };
    // SAFETY: The private C interface requires the documented string pointer.
    let Some(thread_id) =
        (unsafe { required_string(thread_id, "the Codex thread identifier", output_error) })
    else {
        return false;
    };

    send_command(observer, ObserverCommand::Watch(thread_id), output_error)
}

/// Requests an immediate history refresh while preserving the observer.
///
/// # Safety
///
/// `observer` must point to a live handle returned by
/// [`ward_core_codex_history_observer_open`]. `output_error`, when non-null,
/// must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_refresh(
    observer: *mut WardCodexHistoryObserver,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    let Some(observer) = (unsafe { observer.as_ref() }) else {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex history observer is missing") };
        return false;
    };

    send_command(observer, ObserverCommand::Refresh, output_error)
}

/// Attempts to acquire exclusive writing access for one persisted thread.
///
/// The asynchronous result is emitted as a thread write-state event. A
/// successful acquisition remains active until it is released, another thread
/// is selected, or the observer is destroyed.
///
/// # Safety
///
/// `observer` must point to a live handle returned by
/// [`ward_core_codex_history_observer_open`]. `thread_id` must point to a
/// NUL-terminated string. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_acquire_write(
    observer: *mut WardCodexHistoryObserver,
    thread_id: *const c_char,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    let Some(observer) = (unsafe { observer.as_ref() }) else {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex history observer is missing") };
        return false;
    };
    // SAFETY: The private C interface requires the documented string pointer.
    let Some(thread_id) =
        (unsafe { required_string(thread_id, "the Codex thread identifier", output_error) })
    else {
        return false;
    };

    send_command(
        observer,
        ObserverCommand::AcquireWrite(thread_id),
        output_error,
    )
}

/// Releases writing access previously acquired for one persisted thread.
///
/// The release is asynchronous. Its completion is emitted as a thread
/// write-state event.
///
/// # Safety
///
/// `observer` must point to a live handle returned by
/// [`ward_core_codex_history_observer_open`]. `thread_id` must point to a
/// NUL-terminated string. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_release_write(
    observer: *mut WardCodexHistoryObserver,
    thread_id: *const c_char,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    let Some(observer) = (unsafe { observer.as_ref() }) else {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex history observer is missing") };
        return false;
    };
    // SAFETY: The private C interface requires the documented string pointer.
    let Some(thread_id) =
        (unsafe { required_string(thread_id, "the Codex thread identifier", output_error) })
    else {
        return false;
    };

    send_command(
        observer,
        ObserverCommand::ReleaseWrite(thread_id),
        output_error,
    )
}

/// Starts one text turn on the selected persisted Codex thread.
///
/// The observer uses its previously acquired writer and emits ordered
/// conversation updates until the turn completes.
///
/// # Safety
///
/// `observer` must point to a live handle returned by
/// [`ward_core_codex_history_observer_open`]. `thread_id` and `prompt` must
/// point to NUL-terminated strings. `turn_mode` and `permission_preset` must
/// use values declared by the private C interface. `output_error`, when
/// non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_start_turn(
    observer: *mut WardCodexHistoryObserver,
    thread_id: *const c_char,
    prompt: *const c_char,
    turn_mode: c_int,
    permission_preset: c_int,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    let Some(observer) = (unsafe { observer.as_ref() }) else {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex history observer is missing") };
        return false;
    };
    // SAFETY: The private C interface requires the documented string pointers.
    let Some(thread_id) =
        (unsafe { required_string(thread_id, "the Codex thread identifier", output_error) })
    else {
        return false;
    };
    // SAFETY: The private C interface requires the documented string pointers.
    let Some(prompt) = (unsafe { required_string(prompt, "the Codex prompt", output_error) })
    else {
        return false;
    };
    if prompt.trim().is_empty() {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex prompt is empty") };
        return false;
    }
    let options = match decode_turn_options(turn_mode, permission_preset) {
        Ok(options) => options,
        Err(message) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, message) };
            return false;
        }
    };

    if observer
        .turn_active
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe {
            write_error(
                output_error,
                "a Codex turn is already queued or running for this observer",
            )
        };
        return false;
    }

    let sent = send_command(
        observer,
        ObserverCommand::StartTurn(TurnRequest {
            thread_id,
            prompt,
            options,
        }),
        output_error,
    );
    if !sent {
        observer.turn_active.store(false, Ordering::Release);
    }
    sent
}

fn decode_turn_options(
    turn_mode: c_int,
    permission_preset: c_int,
) -> Result<TurnOptions, &'static str> {
    let mode = match turn_mode {
        0 => TurnMode::Default,
        1 => TurnMode::Plan,
        _ => return Err("the Codex turn mode is invalid"),
    };
    let permission_preset = match permission_preset {
        0 => TurnPermissionPreset::Inherit,
        1 => TurnPermissionPreset::RequestApproval,
        2 => TurnPermissionPreset::ReadOnly,
        _ => return Err("the Codex permission preset is invalid"),
    };
    Ok(TurnOptions {
        mode,
        permission_preset,
    })
}

/// Sends one structured response to a pending Codex interaction.
///
/// # Safety
///
/// `observer` must point to a live handle returned by
/// [`ward_core_codex_history_observer_open`]. `response_data` must point to
/// `response_size` readable bytes containing a serialized
/// `PendingInteractionResponse`. `output_error`, when non-null, must be
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_resolve_interaction(
    observer: *mut WardCodexHistoryObserver,
    response_data: *const u8,
    response_size: usize,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    let Some(observer) = (unsafe { observer.as_ref() }) else {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex history observer is missing") };
        return false;
    };
    if response_data.is_null() {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex interaction response is missing") };
        return false;
    }
    // SAFETY: The private C interface requires a readable buffer with the
    // documented length for the duration of this call.
    let bytes = unsafe { std::slice::from_raw_parts(response_data, response_size) };
    let response = match wire::PendingInteractionResponse::decode(bytes) {
        Ok(response) => response,
        Err(error) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe {
                write_error(
                    output_error,
                    format!("the Codex interaction response is invalid: {error}"),
                )
            };
            return false;
        }
    };
    let response = match InteractionResponse::try_from(response) {
        Ok(response) => response,
        Err(error) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, error) };
            return false;
        }
    };

    send_command(
        observer,
        ObserverCommand::ResolveInteraction(response),
        output_error,
    )
}

fn send_command(
    observer: &WardCodexHistoryObserver,
    command: ObserverCommand,
    output_error: *mut *mut WardError,
) -> bool {
    match observer.commands.try_send(command) {
        Ok(()) => true,
        Err(mpsc::error::TrySendError::Full(_)) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, "the Codex history command queue is full") };
            false
        }
        Err(mpsc::error::TrySendError::Closed(_)) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, "the Codex history observer has stopped") };
            false
        }
    }
}

/// Stops and destroys a Codex history observer.
///
/// This function waits for any in-flight read and callback to finish before it
/// returns. It must not be called from the observer callback itself or from a
/// worker thread belonging to the observer's Ward runtime.
///
/// # Safety
///
/// `observer` must be null or a live handle returned by
/// [`ward_core_codex_history_observer_open`], and ownership may be transferred
/// only once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_destroy(
    observer: *mut WardCodexHistoryObserver,
) {
    if !observer.is_null() {
        // SAFETY: The caller transfers the live handle exactly once.
        drop(unsafe { Box::from_raw(observer) });
    }
}
