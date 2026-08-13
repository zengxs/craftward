// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{c_char, c_void};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use prost::Message as _;
use ward_codex::{
    CodexError, CodexHistoryCancellation, CodexHistorySession, Thread, ThreadListOptions,
    ThreadPage, ThreadPagePoll, ThreadPoll,
};

use super::{WardBuffer, clear_error, required_string, wire};
use crate::{WardError, write_error};

const THREAD_PAGE_POLL_INTERVAL: Duration = Duration::from_secs(2);
const CONVERSATION_POLL_INTERVAL: Duration = Duration::from_millis(500);
const HISTORY_ERROR_RETRY_INTERVAL: Duration = Duration::from_secs(2);
const THREAD_PAGE_LIMIT: u32 = 100;

type WardCodexHistoryEventCallback =
    unsafe extern "C" fn(context: *mut c_void, event: *const WardBuffer);

struct HistoryEventSink {
    callback: WardCodexHistoryEventCallback,
    context: *mut c_void,
}

// SAFETY: The C consumer promises that its callback context remains valid
// until `ward_core_codex_history_observer_destroy` returns. The callback
// decides how to marshal each borrowed event onto its own thread.
unsafe impl Send for HistoryEventSink {}

impl HistoryEventSink {
    fn emit_threads_updated(&self, page: ThreadPage) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ThreadsUpdated as i32,
            thread_id: None,
            body: Some(wire::history_event::Body::ThreadPage(page.into())),
        });
    }

    fn emit_conversation_updated(&self, thread_id: &str, thread: Thread) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ConversationUpdated as i32,
            thread_id: Some(thread_id.to_owned()),
            body: Some(wire::history_event::Body::Conversation(thread.into())),
        });
    }

    fn emit_threads_recovered(&self) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ThreadsRecovered as i32,
            thread_id: None,
            body: None,
        });
    }

    fn emit_conversation_recovered(&self, thread_id: &str) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ConversationRecovered as i32,
            thread_id: Some(thread_id.to_owned()),
            body: None,
        });
    }

    fn emit_threads_error(&self, message: &str) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ThreadsError as i32,
            thread_id: None,
            body: Some(wire::history_event::Body::ErrorMessage(message.to_owned())),
        });
    }

    fn emit_conversation_error(&self, thread_id: &str, message: &str) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ConversationError as i32,
            thread_id: Some(thread_id.to_owned()),
            body: Some(wire::history_event::Body::ErrorMessage(message.to_owned())),
        });
    }

    fn emit(&self, event: wire::HistoryEvent) {
        let buffer = WardBuffer {
            bytes: event.encode_to_vec().into_boxed_slice(),
        };

        // SAFETY: The borrowed event buffer remains valid for this callback. The
        // C consumer owns its context for the observer's lifetime.
        unsafe { (self.callback)(self.context, std::ptr::from_ref(&buffer)) };
    }
}

#[derive(Debug)]
enum ObserverCommand {
    Watch(String),
    Refresh,
    Stop,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct CommandUpdate {
    watched_thread: Option<String>,
    refresh: bool,
}

#[derive(Debug, Eq, PartialEq)]
enum DrainedCommands {
    Update(CommandUpdate),
    Stop,
}

#[derive(Debug, Eq, PartialEq)]
enum PollSample<T> {
    Updated(T),
    Unchanged,
}

#[derive(Debug, Eq, PartialEq)]
enum PollEffect<T> {
    Updated(T),
    Recovered,
    Unchanged,
    Error(String),
    RepeatedError,
    Cancelled,
}

impl<T> PollEffect<T> {
    fn is_successful(&self) -> bool {
        matches!(self, Self::Updated(_) | Self::Recovered | Self::Unchanged)
    }
}

#[derive(Default)]
struct PollHealth {
    last_error: Option<String>,
}

impl PollHealth {
    fn observe<T>(
        &mut self,
        result: Result<PollSample<T>, CodexError>,
        cancelled: bool,
    ) -> PollEffect<T> {
        match result {
            Ok(PollSample::Updated(value)) => {
                self.last_error = None;
                PollEffect::Updated(value)
            }
            Ok(PollSample::Unchanged) if self.last_error.take().is_some() => PollEffect::Recovered,
            Ok(PollSample::Unchanged) => PollEffect::Unchanged,
            Err(_) if cancelled => PollEffect::Cancelled,
            Err(error) => {
                let message = error.to_string();
                if self.last_error.as_deref() == Some(message.as_str()) {
                    PollEffect::RepeatedError
                } else {
                    self.last_error = Some(message.clone());
                    PollEffect::Error(message)
                }
            }
        }
    }

    fn reset(&mut self) {
        self.last_error = None;
    }
}

struct ObserverState {
    executable: PathBuf,
    cancellation: CodexHistoryCancellation,
    session: Option<CodexHistorySession>,
    thread_page_health: PollHealth,
    conversation_health: PollHealth,
}

impl ObserverState {
    fn new(executable: PathBuf, cancellation: CodexHistoryCancellation) -> Self {
        Self {
            executable,
            cancellation,
            session: None,
            thread_page_health: PollHealth::default(),
            conversation_health: PollHealth::default(),
        }
    }

    fn select_thread(&mut self) {
        self.conversation_health.reset();
        if let Some(session) = self.session.as_mut() {
            session.reset_thread_baseline();
        }
    }

    fn refresh(&mut self) {
        self.thread_page_health.reset();
        self.conversation_health.reset();
        if let Some(session) = self.session.as_mut() {
            session.reset_thread_page_baseline();
            session.reset_thread_baseline();
        }
    }

    fn poll_threads(&mut self, sink: &HistoryEventSink) -> bool {
        let result = self.poll_thread_page().map(|poll| match poll {
            ThreadPagePoll::Baseline(page) | ThreadPagePoll::Changed(page) => {
                PollSample::Updated(page)
            }
            _ => PollSample::Unchanged,
        });
        let effect = self
            .thread_page_health
            .observe(result, self.cancellation.is_cancelled());
        let succeeded = effect.is_successful();
        match effect {
            PollEffect::Updated(page) => sink.emit_threads_updated(page),
            PollEffect::Recovered => sink.emit_threads_recovered(),
            PollEffect::Error(message) => sink.emit_threads_error(&message),
            PollEffect::Unchanged | PollEffect::RepeatedError | PollEffect::Cancelled => {}
        }
        succeeded
    }

    fn poll_conversation(&mut self, thread_id: &str, sink: &HistoryEventSink) -> bool {
        let result = self.poll_thread(thread_id).map(|poll| match poll {
            ThreadPoll::Baseline(thread) | ThreadPoll::Changed(thread) => {
                PollSample::Updated(thread)
            }
            _ => PollSample::Unchanged,
        });
        let effect = self
            .conversation_health
            .observe(result, self.cancellation.is_cancelled());
        let succeeded = effect.is_successful();
        match effect {
            PollEffect::Updated(thread) => sink.emit_conversation_updated(thread_id, thread),
            PollEffect::Recovered => sink.emit_conversation_recovered(thread_id),
            PollEffect::Error(message) => sink.emit_conversation_error(thread_id, &message),
            PollEffect::Unchanged | PollEffect::RepeatedError | PollEffect::Cancelled => {}
        }
        succeeded
    }

    fn poll_thread_page(&mut self) -> Result<ThreadPagePoll, CodexError> {
        self.session()?.poll_thread_page(&ThreadListOptions {
            limit: Some(THREAD_PAGE_LIMIT),
            ..ThreadListOptions::default()
        })
    }

    fn poll_thread(&mut self, thread_id: &str) -> Result<ThreadPoll, CodexError> {
        self.session()?.poll_thread(thread_id)
    }

    fn session(&mut self) -> Result<&mut CodexHistorySession, CodexError> {
        if self.session.is_none() {
            self.session = Some(CodexHistorySession::spawn_with_cancellation(
                &self.executable,
                self.cancellation.clone(),
            )?);
        }
        Ok(self
            .session
            .as_mut()
            .expect("the history session was initialized above"))
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

/// Starts a background observer for persisted Codex history.
///
/// The callback receives a borrowed serialized event buffer from the observer
/// thread. Its context must remain valid until
/// [`ward_core_codex_history_observer_destroy`] returns.
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
            move || run_observer(PathBuf::from(executable), receiver, sink, cancellation)
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

fn send_command(
    observer: &WardCodexHistoryObserver,
    command: ObserverCommand,
    output_error: *mut *mut WardError,
) -> bool {
    if observer.commands.send(command).is_err() {
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
    receiver: Receiver<ObserverCommand>,
    sink: HistoryEventSink,
    cancellation: CodexHistoryCancellation,
) {
    let mut state = ObserverState::new(executable, cancellation.clone());
    let mut watched_thread: Option<String> = None;
    let mut threads_due = Instant::now();
    let mut conversation_due: Option<Instant> = None;

    loop {
        let now = Instant::now();
        if now >= threads_due {
            let succeeded = state.poll_threads(&sink);
            if cancellation.is_cancelled() {
                break;
            }
            threads_due = Instant::now()
                + if succeeded {
                    THREAD_PAGE_POLL_INTERVAL
                } else {
                    HISTORY_ERROR_RETRY_INTERVAL
                };
        }

        if let (Some(thread_id), Some(due)) = (watched_thread.as_deref(), conversation_due)
            && Instant::now() >= due
        {
            let succeeded = state.poll_conversation(thread_id, &sink);
            if cancellation.is_cancelled() {
                break;
            }
            conversation_due = Some(
                Instant::now()
                    + if succeeded {
                        CONVERSATION_POLL_INTERVAL
                    } else {
                        HISTORY_ERROR_RETRY_INTERVAL
                    },
            );
        }

        let next_due = conversation_due.map_or(threads_due, |due| due.min(threads_due));
        let timeout = next_due.saturating_duration_since(Instant::now());
        match receiver.recv_timeout(timeout) {
            Ok(command) => match drain_commands(command, &receiver) {
                DrainedCommands::Stop => break,
                DrainedCommands::Update(update) => {
                    let now = Instant::now();
                    if let Some(thread_id) = update.watched_thread {
                        state.select_thread();
                        watched_thread = Some(thread_id);
                        conversation_due = Some(now);
                    }
                    if update.refresh {
                        state.refresh();
                        threads_due = now;
                        if watched_thread.is_some() {
                            conversation_due = Some(now);
                        }
                    }
                }
            },
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
    }
}

fn drain_commands(first: ObserverCommand, receiver: &Receiver<ObserverCommand>) -> DrainedCommands {
    let mut update = CommandUpdate::default();
    let mut command = first;
    loop {
        match command {
            ObserverCommand::Watch(thread_id) => update.watched_thread = Some(thread_id),
            ObserverCommand::Refresh => update.refresh = true,
            ObserverCommand::Stop => return DrainedCommands::Stop,
        }

        match receiver.try_recv() {
            Ok(next) => command = next,
            Err(TryRecvError::Empty) => return DrainedCommands::Update(update),
            Err(TryRecvError::Disconnected) => return DrainedCommands::Stop,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use prost::Message as _;
    use ward_codex::{AgentMessagePhase, ThreadItem, ThreadSummary, Turn, TurnStatus};

    use super::*;

    #[derive(Default)]
    struct CapturedEvent {
        events: Vec<wire::HistoryEvent>,
    }

    unsafe extern "C" fn capture_event(context: *mut c_void, event: *const WardBuffer) {
        // SAFETY: This callback is used only with the live mutex and buffer
        // supplied by `HistoryEventSink::emit` below.
        let captured = unsafe { &*(context.cast::<Mutex<CapturedEvent>>()) };
        // SAFETY: The event buffer is valid for this callback.
        let event = unsafe { &*event };
        let event = wire::HistoryEvent::decode(event.bytes.as_ref()).unwrap();
        captured.lock().unwrap().events.push(event);
    }

    fn event_sink(captured: &Mutex<CapturedEvent>) -> HistoryEventSink {
        HistoryEventSink {
            callback: capture_event,
            context: std::ptr::from_ref(captured).cast_mut().cast(),
        }
    }

    fn thread() -> Thread {
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
        }
    }

    #[test]
    fn serializes_thread_pages_for_the_callback_duration() {
        let captured = Mutex::new(CapturedEvent::default());
        event_sink(&captured).emit_threads_updated(ThreadPage {
            threads: vec![thread().summary],
            next_cursor: Some("next".to_owned()),
        });

        let captured = captured.lock().unwrap();
        assert_eq!(captured.events.len(), 1);
        let event = &captured.events[0];
        assert_eq!(event.kind, wire::HistoryEventKind::ThreadsUpdated as i32);
        assert_eq!(event.thread_id, None);
        let Some(wire::history_event::Body::ThreadPage(page)) = event.body.as_ref() else {
            panic!("the event must contain a thread page");
        };
        assert_eq!(page.threads[0].thread_id, "thread-1");
        assert_eq!(page.next_cursor.as_deref(), Some("next"));
    }

    #[test]
    fn serializes_conversations_for_the_callback_duration() {
        let captured = Mutex::new(CapturedEvent::default());
        event_sink(&captured).emit_conversation_updated("thread-1", thread());

        let captured = captured.lock().unwrap();
        assert_eq!(captured.events.len(), 1);
        let event = &captured.events[0];
        assert_eq!(
            event.kind,
            wire::HistoryEventKind::ConversationUpdated as i32
        );
        assert_eq!(event.thread_id.as_deref(), Some("thread-1"));
        let Some(wire::history_event::Body::Conversation(conversation)) = event.body.as_ref()
        else {
            panic!("the event must contain a conversation");
        };
        assert_eq!(conversation.title, "Example");
        assert_eq!(conversation.timeline.len(), 1);
        assert_eq!(conversation.timeline[0].turn_id, "turn-1");
        let Some(wire::timeline_item::Body::Message(message)) =
            conversation.timeline[0].body.as_ref()
        else {
            panic!("the timeline item must contain a message");
        };
        assert_eq!(message.message_id, "agent-1");
    }

    #[test]
    fn emits_targeted_recovery_and_error_states_without_payloads() {
        let captured = Mutex::new(CapturedEvent::default());
        let sink = event_sink(&captured);

        sink.emit_threads_error("disconnected");
        {
            let captured = captured.lock().unwrap();
            assert_eq!(captured.events.len(), 1);
            let event = &captured.events[0];
            assert_eq!(event.kind, wire::HistoryEventKind::ThreadsError as i32);
            assert_eq!(event.thread_id, None);
            assert_eq!(
                event.body,
                Some(wire::history_event::Body::ErrorMessage(
                    "disconnected".to_owned()
                ))
            );
        }

        sink.emit_conversation_recovered("thread-1");
        let captured = captured.lock().unwrap();
        assert_eq!(captured.events.len(), 2);
        let event = &captured.events[1];
        assert_eq!(
            event.kind,
            wire::HistoryEventKind::ConversationRecovered as i32
        );
        assert_eq!(event.thread_id.as_deref(), Some("thread-1"));
        assert_eq!(event.body, None);
    }

    #[test]
    fn classifies_poll_health_transitions() {
        let mut health = PollHealth::default();
        let error = |message: &str| CodexError::Server {
            method: "thread/read",
            code: -1,
            message: message.to_owned(),
        };

        assert_eq!(
            health.observe(Ok(PollSample::<()>::Unchanged), false),
            PollEffect::Unchanged
        );
        assert!(matches!(
            health.observe::<()>(Err(error("offline")), false),
            PollEffect::Error(message) if message.ends_with("offline")
        ));
        assert_eq!(
            health.observe::<()>(Err(error("offline")), false),
            PollEffect::RepeatedError
        );
        health.reset();
        assert!(matches!(
            health.observe::<()>(Err(error("offline")), false),
            PollEffect::Error(message) if message.ends_with("offline")
        ));
        assert_eq!(
            health.observe(Ok(PollSample::<()>::Unchanged), false),
            PollEffect::Recovered
        );
        assert!(matches!(
            health.observe::<()>(Err(error("unavailable")), false),
            PollEffect::Error(message) if message.ends_with("unavailable")
        ));
        assert_eq!(
            health.observe(Ok(PollSample::Updated(7)), false),
            PollEffect::Updated(7)
        );
        assert_eq!(
            health.observe::<()>(Err(error("offline")), true),
            PollEffect::Cancelled
        );
    }

    #[test]
    fn suppresses_repeated_identical_errors_for_each_target() {
        let captured = Mutex::new(CapturedEvent::default());
        let sink = event_sink(&captured);
        let mut state = ObserverState::new(
            PathBuf::from("/craftward-tests/missing-codex"),
            CodexHistoryCancellation::new(),
        );

        assert!(!state.poll_threads(&sink));
        assert!(!state.poll_threads(&sink));
        state.select_thread();
        assert!(!state.poll_conversation("thread-1", &sink));
        assert!(!state.poll_conversation("thread-1", &sink));

        let captured = captured.lock().unwrap();
        assert_eq!(captured.events.len(), 2);
        assert_eq!(
            captured.events.last().unwrap().kind,
            wire::HistoryEventKind::ConversationError as i32
        );
    }

    #[test]
    fn coalesces_commands_and_prioritizes_stop() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(ObserverCommand::Watch("thread-2".to_owned()))
            .unwrap();
        sender.send(ObserverCommand::Refresh).unwrap();
        assert_eq!(
            drain_commands(ObserverCommand::Watch("thread-1".to_owned()), &receiver),
            DrainedCommands::Update(CommandUpdate {
                watched_thread: Some("thread-2".to_owned()),
                refresh: true,
            })
        );

        sender.send(ObserverCommand::Stop).unwrap();
        assert_eq!(
            drain_commands(ObserverCommand::Refresh, &receiver),
            DrainedCommands::Stop
        );
    }
}
