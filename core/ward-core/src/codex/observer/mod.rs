// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{c_char, c_int, c_void};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use prost::Message as _;
use tokio::runtime::Handle;
use tokio::sync::mpsc::{self, Sender};
use tokio::task::JoinHandle;
use ward_codex::{
    CodexAppServerSource, CodexHistoryCancellation, InferenceOverride, InteractionResponse,
    ReasoningEffort, TurnInput, TurnMode, TurnOptions, TurnPermissionPreset,
};

use self::commands::{
    ObserverCommand, ThreadForkRequest, ThreadLifecycleAction, ThreadLifecycleRequest,
    ThreadListScope, ThreadRenameRequest, ThreadStartRequest, TurnRequest, TurnSteerRequest,
};
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
const TURN_ATTACHMENT_LOCAL_IMAGE: c_int = 0;
const TURN_ATTACHMENT_LOCAL_AUDIO: c_int = 1;
const TURN_ATTACHMENT_MENTION: c_int = 2;

/// One typed local attachment borrowed through Ward Core's private C interface.
#[repr(C)]
pub struct WardCodexTurnAttachment {
    kind: c_int,
    name: *const c_char,
    path: *const c_char,
}

#[derive(Clone, Copy)]
#[repr(u8)]
enum ObserverOperation {
    Idle,
    ThreadStart,
    ThreadFork,
    Turn,
}

struct ObserverOperationGate(AtomicU8);

impl ObserverOperationGate {
    const fn new() -> Self {
        Self(AtomicU8::new(ObserverOperation::Idle as u8))
    }

    fn reserve(&self, requested: ObserverOperation) -> Result<(), ObserverOperation> {
        debug_assert!(!matches!(requested, ObserverOperation::Idle));
        self.0
            .compare_exchange(
                ObserverOperation::Idle as u8,
                requested as u8,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map(drop)
            .map_err(|active| match active {
                active if active == ObserverOperation::ThreadStart as u8 => {
                    ObserverOperation::ThreadStart
                }
                active if active == ObserverOperation::ThreadFork as u8 => {
                    ObserverOperation::ThreadFork
                }
                active if active == ObserverOperation::Turn as u8 => ObserverOperation::Turn,
                _ => unreachable!("the observer operation gate contains an invalid state"),
            })
    }

    fn release(&self) {
        self.0
            .store(ObserverOperation::Idle as u8, Ordering::Release);
    }
}

type WardCodexHistoryEventCallback =
    unsafe extern "C" fn(context: *mut c_void, event: *const WardBuffer);

/// An opaque asynchronous Codex history observer passed through Ward Core's
/// private C interface.
pub struct WardCodexHistoryObserver {
    commands: Sender<ObserverCommand>,
    cancellation: CodexHistoryCancellation,
    active_operation: Arc<ObserverOperationGate>,
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

/// Starts a background observer for Codex model metadata and persisted history.
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
    let active_operation = Arc::new(ObserverOperationGate::new());
    let sink = HistoryEventSink::new(callback, callback_context);
    let runtime = runtime.handle();
    let worker = runtime.spawn({
        let cancellation = cancellation.clone();
        let active_operation = Arc::clone(&active_operation);
        async move {
            run_observer(
                CodexAppServerSource::executable(PathBuf::from(executable)),
                receiver,
                sink,
                cancellation,
                active_operation,
            )
            .await;
        }
    });

    Box::into_raw(Box::new(WardCodexHistoryObserver {
        commands,
        cancellation,
        active_operation,
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

/// Switches the observer between active and archived persisted history.
///
/// The next successful list read is emitted as a scope-tagged authoritative
/// snapshot.
///
/// # Safety
///
/// `observer` must point to a live handle returned by
/// [`ward_core_codex_history_observer_open`]. `output_error`, when non-null,
/// must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_show_archived(
    observer: *mut WardCodexHistoryObserver,
    archived: bool,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    let Some(observer) = (unsafe { observer.as_ref() }) else {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex history observer is missing") };
        return false;
    };
    let scope = if archived {
        ThreadListScope::Archived
    } else {
        ThreadListScope::Active
    };
    send_command(
        observer,
        ObserverCommand::SetThreadListScope(scope),
        output_error,
    )
}

/// Renames one persisted Codex thread.
///
/// Updated thread and conversation snapshots are emitted asynchronously after
/// the rename succeeds.
///
/// # Safety
///
/// `observer` must point to a live handle returned by
/// [`ward_core_codex_history_observer_open`]. `thread_id` and `name` must point
/// to NUL-terminated strings. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_rename_thread(
    observer: *mut WardCodexHistoryObserver,
    thread_id: *const c_char,
    name: *const c_char,
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
    let Some(name) = (unsafe { required_string(name, "the Codex thread name", output_error) })
    else {
        return false;
    };
    if thread_id.trim().is_empty() || name.trim().is_empty() {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex thread target or name is empty") };
        return false;
    }

    send_command(
        observer,
        ObserverCommand::RenameThread(ThreadRenameRequest { thread_id, name }),
        output_error,
    )
}

/// Forks one active, loaded, and idle persisted Codex thread through a
/// completed turn.
///
/// The asynchronous result is emitted as either a thread-forked event carrying
/// the new conversation or a thread-fork-error event for the source thread.
///
/// # Safety
///
/// `observer` must point to a live handle returned by
/// [`ward_core_codex_history_observer_open`]. `thread_id` and `last_turn_id`
/// must point to NUL-terminated strings. `output_error`, when non-null, must be
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_fork_thread(
    observer: *mut WardCodexHistoryObserver,
    thread_id: *const c_char,
    last_turn_id: *const c_char,
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
    if thread_id.trim().is_empty() {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex thread identifier is empty") };
        return false;
    }
    // SAFETY: The private C interface requires the documented string pointer.
    let Some(last_turn_id) =
        (unsafe { required_string(last_turn_id, "the Codex turn identifier", output_error) })
    else {
        return false;
    };
    if last_turn_id.trim().is_empty() {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex turn identifier is empty") };
        return false;
    }

    send_exclusive_command(
        observer,
        ObserverOperation::ThreadFork,
        ObserverCommand::ForkThread(ThreadForkRequest {
            thread_id,
            last_turn_id,
        }),
        output_error,
    )
}

/// Moves one active persisted Codex thread into archived history.
///
/// The resulting active-history snapshot is emitted asynchronously.
///
/// # Safety
///
/// `observer` must point to a live handle returned by
/// [`ward_core_codex_history_observer_open`]. `thread_id` must point to a
/// NUL-terminated string. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_archive_thread(
    observer: *mut WardCodexHistoryObserver,
    thread_id: *const c_char,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller upholds the private C interface contract documented
    // above.
    unsafe {
        queue_thread_lifecycle(
            observer,
            thread_id,
            ThreadLifecycleAction::Archive,
            output_error,
        )
    }
}

/// Restores one archived persisted Codex thread to active history.
///
/// The resulting archived-history snapshot is emitted asynchronously.
///
/// # Safety
///
/// `observer` must point to a live handle returned by
/// [`ward_core_codex_history_observer_open`]. `thread_id` must point to a
/// NUL-terminated string. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_restore_thread(
    observer: *mut WardCodexHistoryObserver,
    thread_id: *const c_char,
    output_error: *mut *mut WardError,
) -> bool {
    // SAFETY: The caller upholds the private C interface contract documented
    // above.
    unsafe {
        queue_thread_lifecycle(
            observer,
            thread_id,
            ThreadLifecycleAction::Restore,
            output_error,
        )
    }
}

/// Starts and observes one persisted Codex thread.
///
/// The asynchronous result is emitted as either a thread-started event or a
/// thread-start-error event. A successful start also grants writing access and
/// makes the new thread the observer's selected thread.
///
/// # Safety
///
/// `observer` must point to a live handle returned by
/// [`ward_core_codex_history_observer_open`]. `working_directory` must point to
/// a NUL-terminated string. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_start_thread(
    observer: *mut WardCodexHistoryObserver,
    working_directory: *const c_char,
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
    let Some(working_directory) = (unsafe {
        required_string(
            working_directory,
            "the Codex working directory",
            output_error,
        )
    }) else {
        return false;
    };
    if working_directory.trim().is_empty() {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex working directory is empty") };
        return false;
    }
    send_exclusive_command(
        observer,
        ObserverOperation::ThreadStart,
        ObserverCommand::StartThread(ThreadStartRequest {
            working_directory: PathBuf::from(working_directory),
        }),
        output_error,
    )
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

/// Starts one turn on the selected persisted Codex thread.
///
/// The observer uses its previously acquired writer and emits ordered
/// conversation updates until the turn completes.
///
/// # Safety
///
/// `observer` must point to a live handle returned by
/// [`ward_core_codex_history_observer_open`]. `thread_id` and `prompt` must
/// point to NUL-terminated strings. When `attachment_count` is nonzero,
/// `attachments` must point to that many attachment records. Every record must
/// contain NUL-terminated `name` and `path` strings and a declared attachment
/// kind.
/// `model` may be null to preserve the thread's active model; otherwise it must
/// point to a non-empty NUL-terminated string. `reasoning_effort` follows the
/// same convention for the thread's active reasoning effort. `turn_mode` and
/// `permission_preset` must use values declared by the private C interface.
/// `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_start_turn(
    observer: *mut WardCodexHistoryObserver,
    thread_id: *const c_char,
    prompt: *const c_char,
    attachments: *const WardCodexTurnAttachment,
    attachment_count: usize,
    model: *const c_char,
    reasoning_effort: *const c_char,
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
    let raw_attachments = if attachment_count == 0 {
        &[][..]
    } else {
        if attachments.is_null() {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, "the Codex turn attachments are missing") };
            return false;
        }
        // SAFETY: The private C interface requires an array containing the
        // documented number of attachment records.
        unsafe { std::slice::from_raw_parts(attachments, attachment_count) }
    };
    let mut input = Vec::with_capacity(usize::from(!prompt.trim().is_empty()) + attachment_count);
    if !prompt.trim().is_empty() {
        input.push(TurnInput::Text(prompt));
    }
    for attachment in raw_attachments {
        // SAFETY: Every attachment must name NUL-terminated strings.
        let Some(name) = (unsafe {
            required_string(attachment.name, "the Codex attachment name", output_error)
        }) else {
            return false;
        };
        // SAFETY: Every attachment must name NUL-terminated strings.
        let Some(path) = (unsafe {
            required_string(attachment.path, "the Codex attachment path", output_error)
        }) else {
            return false;
        };
        if name.trim().is_empty() {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, "the Codex attachment name is empty") };
            return false;
        }
        if path.trim().is_empty() {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, "the Codex attachment path is empty") };
            return false;
        }
        let path = PathBuf::from(path);
        input.push(match attachment.kind {
            TURN_ATTACHMENT_LOCAL_IMAGE => TurnInput::LocalImage { path },
            TURN_ATTACHMENT_LOCAL_AUDIO => TurnInput::LocalAudio { path },
            TURN_ATTACHMENT_MENTION => TurnInput::Mention { name, path },
            _ => {
                // SAFETY: The caller supplied the optional error output pointer.
                unsafe { write_error(output_error, "the Codex attachment kind is invalid") };
                return false;
            }
        });
    }
    if input.is_empty() {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex turn input is empty") };
        return false;
    }
    let model = if model.is_null() {
        None
    } else {
        // SAFETY: The private C interface requires a non-null model pointer to
        // name a NUL-terminated string.
        let Some(model) = (unsafe { required_string(model, "the Codex model", output_error) })
        else {
            return false;
        };
        if model.trim().is_empty() {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, "the Codex model is empty") };
            return false;
        }
        Some(model)
    };
    let reasoning_effort = if reasoning_effort.is_null() {
        None
    } else {
        // SAFETY: The private C interface requires a non-null reasoning-effort
        // pointer to name a NUL-terminated string.
        let Some(reasoning_effort) = (unsafe {
            required_string(reasoning_effort, "the Codex reasoning effort", output_error)
        }) else {
            return false;
        };
        if reasoning_effort.trim().is_empty() {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, "the Codex reasoning effort is empty") };
            return false;
        }
        ReasoningEffort::new(reasoning_effort)
    };
    let mut options = match decode_turn_options(turn_mode, permission_preset) {
        Ok(options) => options,
        Err(message) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, message) };
            return false;
        }
    };
    options.inference = match (model, reasoning_effort) {
        (Some(model), Some(reasoning_effort)) => {
            Some(InferenceOverride::selection(model, reasoning_effort))
        }
        (Some(model), None) => Some(InferenceOverride::model(model)),
        (None, Some(reasoning_effort)) => {
            Some(InferenceOverride::reasoning_effort(reasoning_effort))
        }
        (None, None) => None,
    };

    send_exclusive_command(
        observer,
        ObserverOperation::Turn,
        ObserverCommand::StartTurn(TurnRequest {
            thread_id,
            input,
            options,
        }),
        output_error,
    )
}

/// Adds text guidance to the selected thread's expected active Codex turn.
///
/// The asynchronous result is emitted as either a turn-steered event or a
/// turn-steer-error event.
///
/// # Safety
///
/// `observer` must point to a live handle returned by
/// [`ward_core_codex_history_observer_open`]. `thread_id`, `expected_turn_id`,
/// and `prompt` must point to NUL-terminated strings. `output_error`, when
/// non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_steer_turn(
    observer: *mut WardCodexHistoryObserver,
    thread_id: *const c_char,
    expected_turn_id: *const c_char,
    prompt: *const c_char,
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
    let Some(expected_turn_id) = (unsafe {
        required_string(
            expected_turn_id,
            "the expected Codex turn identifier",
            output_error,
        )
    }) else {
        return false;
    };
    // SAFETY: The private C interface requires the documented string pointers.
    let Some(prompt) = (unsafe { required_string(prompt, "the Codex guidance", output_error) })
    else {
        return false;
    };
    if thread_id.trim().is_empty() || expected_turn_id.trim().is_empty() || prompt.trim().is_empty()
    {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex guidance target or text is empty") };
        return false;
    }

    send_command(
        observer,
        ObserverCommand::SteerTurn(TurnSteerRequest {
            thread_id,
            expected_turn_id,
            prompt,
        }),
        output_error,
    )
}

fn send_exclusive_command(
    observer: &WardCodexHistoryObserver,
    operation: ObserverOperation,
    command: ObserverCommand,
    output_error: *mut *mut WardError,
) -> bool {
    if !reserve_operation(observer, operation, output_error) {
        return false;
    }

    let sent = send_command(observer, command, output_error);
    if !sent {
        observer.active_operation.release();
    }
    sent
}

fn reserve_operation(
    observer: &WardCodexHistoryObserver,
    requested: ObserverOperation,
    output_error: *mut *mut WardError,
) -> bool {
    match observer.active_operation.reserve(requested) {
        Ok(()) => true,
        Err(active) => {
            let message = match active {
                ObserverOperation::ThreadStart => {
                    "a Codex thread start is already queued or running for this observer"
                }
                ObserverOperation::ThreadFork => {
                    "a Codex thread fork is already queued or running for this observer"
                }
                ObserverOperation::Turn => {
                    "a Codex turn is already queued or running for this observer"
                }
                ObserverOperation::Idle => unreachable!("an idle observer operation was reserved"),
            };
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, message) };
            false
        }
    }
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
        inference: None,
    })
}

/// Requests interruption of the selected thread's active Codex turn.
///
/// The interruption is asynchronous. Runtime and turn-completion events report
/// the eventual state.
///
/// # Safety
///
/// `observer` must point to a live handle returned by
/// [`ward_core_codex_history_observer_open`]. `thread_id` must point to a
/// NUL-terminated string. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_interrupt_turn(
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
        ObserverCommand::InterruptTurn(thread_id),
        output_error,
    )
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

unsafe fn queue_thread_lifecycle(
    observer: *mut WardCodexHistoryObserver,
    thread_id: *const c_char,
    action: ThreadLifecycleAction,
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
    if thread_id.trim().is_empty() {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex thread identifier is empty") };
        return false;
    }

    send_command(
        observer,
        ObserverCommand::ChangeThreadLifecycle(ThreadLifecycleRequest { thread_id, action }),
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
