// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{c_char, c_void};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use prost::Message as _;
use ward_codex::{CodexError, CodexHistoryCancellation, CodexHistorySession, Thread, ThreadPoll};

use super::{WardBuffer, clear_error, required_string, wire};
use crate::{WardError, c_string, write_error};

const HISTORY_POLL_INTERVAL: Duration = Duration::from_millis(500);
const HISTORY_ERROR_RETRY_INTERVAL: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(C)]
pub enum WardCodexHistoryEventKind {
    Updated = 0,
    Recovered = 1,
    Error = 2,
}

/// A borrowed history event passed through Ward Core's private C interface.
#[repr(C)]
pub struct WardCodexHistoryEvent {
    kind: WardCodexHistoryEventKind,
    thread_id: *const c_char,
    conversation: *const WardBuffer,
    error_message: *const c_char,
}

type WardCodexHistoryEventCallback =
    unsafe extern "C" fn(context: *mut c_void, event: *const WardCodexHistoryEvent);

struct HistoryEventSink {
    callback: WardCodexHistoryEventCallback,
    context: *mut c_void,
}

// SAFETY: The C consumer promises that its callback context remains valid
// until `ward_core_codex_history_observer_destroy` returns. The callback
// decides how to marshal each borrowed event onto its own thread.
unsafe impl Send for HistoryEventSink {}

impl HistoryEventSink {
    fn emit_updated(&self, thread_id: &str, thread: Thread) {
        let message = wire::Conversation::from(thread);
        let buffer = WardBuffer {
            bytes: message.encode_to_vec().into_boxed_slice(),
        };
        self.emit(
            thread_id,
            WardCodexHistoryEventKind::Updated,
            Some(&buffer),
            None,
        );
    }

    fn emit_recovered(&self, thread_id: &str) {
        self.emit(thread_id, WardCodexHistoryEventKind::Recovered, None, None);
    }

    fn emit_error(&self, thread_id: &str, message: &str) {
        self.emit(
            thread_id,
            WardCodexHistoryEventKind::Error,
            None,
            Some(message),
        );
    }

    fn emit(
        &self,
        thread_id: &str,
        kind: WardCodexHistoryEventKind,
        conversation: Option<&WardBuffer>,
        error_message: Option<&str>,
    ) {
        let thread_id = c_string(thread_id);
        let error_message = error_message.map(c_string);
        let event = WardCodexHistoryEvent {
            kind,
            thread_id: thread_id.as_ptr(),
            conversation: conversation.map_or(std::ptr::null(), std::ptr::from_ref),
            error_message: error_message
                .as_ref()
                .map_or(std::ptr::null(), |message| message.as_ptr()),
        };

        // SAFETY: All borrowed event fields remain valid for this callback.
        // The C consumer owns its context for the observer's lifetime.
        unsafe { (self.callback)(self.context, std::ptr::from_ref(&event)) };
    }
}

enum ObserverCommand {
    Watch(String),
    Stop,
}

struct ObserverState {
    executable: PathBuf,
    cancellation: CodexHistoryCancellation,
    session: Option<CodexHistorySession>,
    last_error: Option<String>,
}

impl ObserverState {
    fn new(executable: PathBuf, cancellation: CodexHistoryCancellation) -> Self {
        Self {
            executable,
            cancellation,
            session: None,
            last_error: None,
        }
    }

    fn select_thread(&mut self) {
        self.last_error = None;
        if let Some(session) = self.session.as_mut() {
            session.reset_baseline();
        }
    }

    fn poll(&mut self, thread_id: &str, sink: &HistoryEventSink) -> bool {
        match self.poll_thread(thread_id) {
            Ok(ThreadPoll::Baseline(thread) | ThreadPoll::Changed(thread)) => {
                self.last_error = None;
                sink.emit_updated(thread_id, thread);
                true
            }
            Ok(ThreadPoll::Unchanged) | Ok(_) => {
                if self.last_error.take().is_some() {
                    sink.emit_recovered(thread_id);
                }
                true
            }
            Err(error) => {
                if self.cancellation.is_cancelled() {
                    return false;
                }
                let message = error.to_string();
                if self.last_error.as_deref() != Some(message.as_str()) {
                    sink.emit_error(thread_id, &message);
                }
                self.last_error = Some(message);
                false
            }
        }
    }

    fn poll_thread(&mut self, thread_id: &str) -> Result<ThreadPoll, CodexError> {
        if self.session.is_none() {
            self.session = Some(CodexHistorySession::spawn_with_cancellation(
                &self.executable,
                self.cancellation.clone(),
            )?);
        }
        self.session
            .as_mut()
            .expect("the history session was initialized above")
            .poll_thread(thread_id)
    }
}

/// An opaque asynchronous Codex history observer passed through Ward Core's
/// private C interface.
pub struct WardCodexHistoryObserver {
    commands: Sender<ObserverCommand>,
    cancellation: CodexHistoryCancellation,
    worker: Option<JoinHandle<()>>,
}

impl Drop for WardCodexHistoryObserver {
    fn drop(&mut self) {
        let _ = self.commands.send(ObserverCommand::Stop);
        self.cancellation.cancel();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Starts a background observer without starting a Codex app-server yet.
///
/// The callback receives borrowed event data from the observer thread. Its
/// context must remain valid until [`ward_core_codex_history_observer_destroy`]
/// returns.
///
/// # Safety
///
/// `executable` must point to a NUL-terminated string. `callback` must be a
/// valid function pointer. `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_open(
    executable: *const c_char,
    callback: Option<WardCodexHistoryEventCallback>,
    callback_context: *mut c_void,
    output_error: *mut *mut WardError,
) -> *mut WardCodexHistoryObserver {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
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

    let (commands, receiver) = mpsc::channel();
    let cancellation = CodexHistoryCancellation::new();
    let sink = HistoryEventSink {
        callback,
        context: callback_context,
    };
    let worker = thread::Builder::new()
        .name("ward-codex-history".to_owned())
        .spawn({
            let cancellation = cancellation.clone();
            move || {
                run_observer(
                    PathBuf::from(executable),
                    HISTORY_POLL_INTERVAL,
                    receiver,
                    sink,
                    cancellation,
                )
            }
        });

    match worker {
        Ok(worker) => Box::into_raw(Box::new(WardCodexHistoryObserver {
            commands,
            cancellation,
            worker: Some(worker),
        })),
        Err(error) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe {
                write_error(
                    output_error,
                    format!("failed to start the Codex history observer: {error}"),
                )
            };
            std::ptr::null_mut()
        }
    }
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

    if observer
        .commands
        .send(ObserverCommand::Watch(thread_id))
        .is_err()
    {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex history observer has stopped") };
        return false;
    }
    true
}

/// Stops and destroys a Codex history observer.
///
/// This function waits for any in-flight read and callback to finish before it
/// returns. It must not be called from the observer callback itself.
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

fn run_observer(
    executable: PathBuf,
    interval: Duration,
    receiver: Receiver<ObserverCommand>,
    sink: HistoryEventSink,
    cancellation: CodexHistoryCancellation,
) {
    let mut state = ObserverState::new(executable, cancellation);
    let mut watched_thread: Option<String> = None;

    loop {
        let thread_id = match watched_thread.as_ref() {
            Some(thread_id) => thread_id.clone(),
            None => match receiver.recv() {
                Ok(ObserverCommand::Watch(thread_id)) => {
                    let Some(thread_id) = latest_watch(&receiver, thread_id) else {
                        break;
                    };
                    state.select_thread();
                    watched_thread = Some(thread_id.clone());
                    thread_id
                }
                Ok(ObserverCommand::Stop) | Err(_) => break,
            },
        };

        let poll_succeeded = state.poll(&thread_id, &sink);
        // Persistent startup or state-database failures should not churn child
        // processes at the normal conversation refresh rate.
        let next_poll = if poll_succeeded {
            interval
        } else {
            HISTORY_ERROR_RETRY_INTERVAL
        };

        match receiver.recv_timeout(next_poll) {
            Ok(ObserverCommand::Watch(thread_id)) => {
                let Some(thread_id) = latest_watch(&receiver, thread_id) else {
                    break;
                };
                state.select_thread();
                watched_thread = Some(thread_id);
            }
            Ok(ObserverCommand::Stop) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }
    }
}

fn latest_watch(receiver: &Receiver<ObserverCommand>, mut thread_id: String) -> Option<String> {
    loop {
        match receiver.try_recv() {
            Ok(ObserverCommand::Watch(next_thread_id)) => thread_id = next_thread_id,
            Ok(ObserverCommand::Stop) | Err(TryRecvError::Disconnected) => return None,
            Err(TryRecvError::Empty) => return Some(thread_id),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use ward_codex::{AgentMessagePhase, ThreadItem, ThreadSummary, Turn, TurnStatus};

    use super::*;

    #[derive(Default)]
    struct CapturedEvent {
        event_count: usize,
        kind: Option<WardCodexHistoryEventKind>,
        thread_id: String,
        payload: Vec<u8>,
        error_message: String,
    }

    unsafe extern "C" fn capture_event(context: *mut c_void, event: *const WardCodexHistoryEvent) {
        // SAFETY: This callback is used only with the live mutex and event
        // supplied by `HistoryEventSink::emit` below.
        let captured = unsafe { &*(context.cast::<Mutex<CapturedEvent>>()) };
        // SAFETY: The event pointer and its borrowed fields are valid for this
        // callback.
        let event = unsafe { &*event };
        // SAFETY: The sink always supplies a valid NUL-terminated thread ID.
        let thread_id = unsafe { std::ffi::CStr::from_ptr(event.thread_id) }
            .to_string_lossy()
            .into_owned();
        let payload = unsafe { event.conversation.as_ref() }
            .map_or_else(Vec::new, |buffer| buffer.bytes.to_vec());
        let error_message = if event.error_message.is_null() {
            String::new()
        } else {
            // SAFETY: The sink supplies a valid NUL-terminated error string.
            unsafe { std::ffi::CStr::from_ptr(event.error_message) }
                .to_string_lossy()
                .into_owned()
        };
        let mut captured = captured.lock().unwrap();
        captured.event_count += 1;
        captured.kind = Some(event.kind);
        captured.thread_id = thread_id;
        captured.payload = payload;
        captured.error_message = error_message;
    }

    fn event_sink(captured: &Mutex<CapturedEvent>) -> HistoryEventSink {
        HistoryEventSink {
            callback: capture_event,
            context: std::ptr::from_ref(captured).cast_mut().cast(),
        }
    }

    #[test]
    fn serializes_updated_threads_only_for_the_callback_duration() {
        let captured = Mutex::new(CapturedEvent::default());
        event_sink(&captured).emit_updated(
            "thread-1",
            Thread {
                summary: ThreadSummary {
                    id: "thread-1".to_owned(),
                    name: Some("Example".to_owned()),
                    preview: "Preview".to_owned(),
                    cwd: PathBuf::from("/workspace"),
                    created_at_unix_seconds: 10,
                    updated_at_unix_seconds: 20,
                },
                turns: vec![Turn {
                    id: "turn-1".to_owned(),
                    status: TurnStatus::Completed,
                    items: vec![ThreadItem::AgentMessage {
                        id: "agent-1".to_owned(),
                        text: "Done".to_owned(),
                        phase: Some(AgentMessagePhase::FinalAnswer),
                    }],
                }],
            },
        );

        let captured = captured.lock().unwrap();
        assert_eq!(captured.kind, Some(WardCodexHistoryEventKind::Updated));
        assert_eq!(captured.thread_id, "thread-1");
        let conversation = wire::Conversation::decode(captured.payload.as_slice()).unwrap();
        assert_eq!(conversation.title, "Example");
        assert_eq!(conversation.messages[0].message_id, "agent-1");
    }

    #[test]
    fn emits_recovery_and_error_states_without_payloads() {
        let captured = Mutex::new(CapturedEvent::default());
        let sink = event_sink(&captured);

        sink.emit_error("thread-1", "disconnected");
        {
            let captured = captured.lock().unwrap();
            assert_eq!(captured.kind, Some(WardCodexHistoryEventKind::Error));
            assert_eq!(captured.error_message, "disconnected");
            assert!(captured.payload.is_empty());
        }

        sink.emit_recovered("thread-1");
        let captured = captured.lock().unwrap();
        assert_eq!(captured.kind, Some(WardCodexHistoryEventKind::Recovered));
        assert!(captured.error_message.is_empty());
        assert!(captured.payload.is_empty());
    }

    #[test]
    fn suppresses_repeated_identical_polling_errors() {
        let captured = Mutex::new(CapturedEvent::default());
        let sink = event_sink(&captured);
        let mut state = ObserverState::new(
            PathBuf::from("/craftward-tests/missing-codex"),
            CodexHistoryCancellation::new(),
        );

        assert!(!state.poll("thread-1", &sink));
        assert!(!state.poll("thread-1", &sink));

        let captured = captured.lock().unwrap();
        assert_eq!(captured.event_count, 1);
        assert_eq!(captured.kind, Some(WardCodexHistoryEventKind::Error));
    }

    #[test]
    fn coalesces_pending_thread_selections_and_prioritizes_stop() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(ObserverCommand::Watch("thread-2".to_owned()))
            .unwrap();
        assert_eq!(
            latest_watch(&receiver, "thread-1".to_owned()),
            Some("thread-2".to_owned())
        );

        sender.send(ObserverCommand::Stop).unwrap();
        assert_eq!(latest_watch(&receiver, "thread-3".to_owned()), None);
    }
}
