// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{c_char, c_void};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use prost::Message as _;
use ward_codex::{
    CodexError, CodexHistoryCancellation, CodexHistorySession, CodexThreadWriter, Thread,
    ThreadItem, ThreadListOptions, ThreadPage, ThreadPagePoll, ThreadPoll, Turn, TurnStatus,
    TurnStreamEvent,
};

use super::{WardBuffer, clear_error, required_string, wire};
use crate::{WardError, write_error};

const THREAD_PAGE_POLL_INTERVAL: Duration = Duration::from_secs(2);
const CONVERSATION_POLL_INTERVAL: Duration = Duration::from_millis(500);
const LIVE_DELTA_EMIT_INTERVAL: Duration = Duration::from_millis(50);
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

    fn emit_turn_started(&self, thread_id: &str) {
        self.emit_turn_event(wire::HistoryEventKind::TurnStarted, thread_id, None);
    }

    fn emit_turn_completed(&self, thread_id: &str) {
        self.emit_turn_event(wire::HistoryEventKind::TurnCompleted, thread_id, None);
    }

    fn emit_turn_error(&self, thread_id: &str, message: &str) {
        self.emit_turn_event(wire::HistoryEventKind::TurnError, thread_id, Some(message));
    }

    fn emit_turn_notice(&self, thread_id: &str, message: &str) {
        self.emit_turn_event(wire::HistoryEventKind::TurnNotice, thread_id, Some(message));
    }

    fn emit_thread_write_state(
        &self,
        thread_id: &str,
        status: wire::ThreadWriteStatus,
        message: Option<&str>,
    ) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ThreadWriteStateChanged as i32,
            thread_id: Some(thread_id.to_owned()),
            body: Some(wire::history_event::Body::ThreadWriteState(
                wire::ThreadWriteState {
                    status: status as i32,
                    message: message.map(str::to_owned),
                },
            )),
        });
    }

    fn emit_turn_event(
        &self,
        kind: wire::HistoryEventKind,
        thread_id: &str,
        message: Option<&str>,
    ) {
        self.emit(wire::HistoryEvent {
            kind: kind as i32,
            thread_id: Some(thread_id.to_owned()),
            body: message
                .map(|message| wire::history_event::Body::ErrorMessage(message.to_owned())),
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
    AcquireWrite(String),
    ReleaseWrite(String),
    StartTurn(TurnRequest),
    Stop,
}

#[derive(Debug, Eq, PartialEq)]
enum WriteAccessRequest {
    Acquire(String),
    Release(String),
}

#[derive(Debug, Eq, PartialEq)]
struct TurnRequest {
    thread_id: String,
    prompt: String,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct CommandUpdate {
    watched_thread: Option<String>,
    refresh: bool,
    write_access: Option<WriteAccessRequest>,
    turns: Vec<TurnRequest>,
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
    writer: Option<CodexThreadWriter>,
    thread_page_health: PollHealth,
    conversation_health: PollHealth,
    latest_conversation: Option<Thread>,
    retained_live_turn: Option<RetainedLiveTurn>,
}

struct RetainedLiveTurn {
    index: usize,
    turn: Turn,
}

impl ObserverState {
    fn new(executable: PathBuf, cancellation: CodexHistoryCancellation) -> Self {
        Self {
            executable,
            cancellation,
            session: None,
            writer: None,
            thread_page_health: PollHealth::default(),
            conversation_health: PollHealth::default(),
            latest_conversation: None,
            retained_live_turn: None,
        }
    }

    fn select_thread(&mut self) {
        self.writer = None;
        self.conversation_health.reset();
        self.latest_conversation = None;
        self.retained_live_turn = None;
        if let Some(session) = self.session.as_mut() {
            session.reset_thread_baseline();
        }
    }

    fn acquire_write(&mut self, thread_id: &str, sink: &HistoryEventSink) -> bool {
        if self
            .writer
            .as_ref()
            .is_some_and(|writer| writer.thread_id() == thread_id)
        {
            sink.emit_thread_write_state(thread_id, wire::ThreadWriteStatus::Writable, None);
            return true;
        }

        self.writer = None;
        sink.emit_thread_write_state(thread_id, wire::ThreadWriteStatus::Checking, None);
        let result = CodexThreadWriter::acquire_with_cancellation(
            &self.executable,
            self.cancellation.clone(),
            thread_id,
        );
        match classify_write_access_result(result, self.cancellation.is_cancelled()) {
            WriteAccessEffect::Acquired(acquired) => {
                let (writer, thread) = *acquired;
                self.latest_conversation = Some(thread.clone());
                self.retained_live_turn = None;
                sink.emit_conversation_updated(thread_id, thread);
                self.writer = Some(writer);
                sink.emit_thread_write_state(thread_id, wire::ThreadWriteStatus::Writable, None);
                true
            }
            WriteAccessEffect::Busy => {
                sink.emit_thread_write_state(thread_id, wire::ThreadWriteStatus::Busy, None);
                false
            }
            WriteAccessEffect::Unavailable(message) => {
                sink.emit_thread_write_state(
                    thread_id,
                    wire::ThreadWriteStatus::Unavailable,
                    Some(&message),
                );
                false
            }
            WriteAccessEffect::Cancelled => false,
        }
    }

    fn release_write(&mut self, thread_id: &str, sink: &HistoryEventSink) {
        if self
            .writer
            .as_ref()
            .is_some_and(|writer| writer.thread_id() == thread_id)
        {
            self.writer = None;
        }
        sink.emit_thread_write_state(thread_id, wire::ThreadWriteStatus::Idle, None);
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
            PollEffect::Updated(thread) => self.accept_polled_conversation(thread_id, thread, sink),
            PollEffect::Recovered => sink.emit_conversation_recovered(thread_id),
            PollEffect::Error(message) => sink.emit_conversation_error(thread_id, &message),
            PollEffect::Unchanged | PollEffect::RepeatedError | PollEffect::Cancelled => {}
        }
        succeeded
    }

    fn accept_polled_conversation(
        &mut self,
        thread_id: &str,
        thread: Thread,
        sink: &HistoryEventSink,
    ) {
        let Some(retained) = self.retained_live_turn.take() else {
            self.latest_conversation = Some(thread.clone());
            sink.emit_conversation_updated(thread_id, thread);
            return;
        };
        if thread
            .turns
            .iter()
            .find(|turn| turn.id == retained.turn.id)
            .is_some_and(|turn| turn_covers(turn, &retained.turn))
        {
            self.latest_conversation = Some(thread.clone());
            sink.emit_conversation_updated(thread_id, thread);
            return;
        }

        let merged = merge_retained_live_turn(thread, &retained);
        let changed = self.latest_conversation.as_ref() != Some(&merged);
        self.latest_conversation = Some(merged.clone());
        self.retained_live_turn = Some(retained);
        if changed {
            sink.emit_conversation_updated(thread_id, merged);
        }
    }

    fn run_turn(&mut self, request: TurnRequest, sink: &HistoryEventSink) -> bool {
        self.conversation_health.reset();
        let TurnRequest { thread_id, prompt } = request;
        let Some(writer) = self
            .writer
            .as_mut()
            .filter(|writer| writer.thread_id() == thread_id)
        else {
            let message = "Writing access has not been acquired for this conversation.";
            sink.emit_thread_write_state(
                &thread_id,
                wire::ThreadWriteStatus::Unavailable,
                Some(message),
            );
            sink.emit_turn_error(&thread_id, message);
            return false;
        };
        let mut live_conversation = self
            .latest_conversation
            .take()
            .filter(|thread| thread.summary.id == thread_id);
        let mut terminal_error = None;
        let mut live_turn_id = None;
        let mut last_live_emit = Instant::now()
            .checked_sub(LIVE_DELTA_EMIT_INTERVAL)
            .unwrap_or_else(Instant::now);
        let result = writer.start_text_turn(&prompt, |event| {
            match &event {
                TurnStreamEvent::TurnStarted {
                    thread_id: event_thread_id,
                    turn,
                    ..
                } if event_thread_id == &thread_id => {
                    if live_turn_id.is_none() {
                        live_turn_id = Some(turn.id.clone());
                        sink.emit_turn_started(&thread_id);
                    }
                }
                TurnStreamEvent::TurnCompleted {
                    thread_id: event_thread_id,
                    ..
                } if event_thread_id == &thread_id => {}
                TurnStreamEvent::ApprovalDeclined {
                    thread_id: event_thread_id,
                    method,
                } if event_thread_id.as_deref().is_none_or(|id| id == thread_id) => {
                    sink.emit_turn_notice(
                        &thread_id,
                        &format!(
                            "Craftward declined {method} because approval controls are not available yet."
                        ),
                    );
                }
                TurnStreamEvent::UnsupportedServerRequest {
                    thread_id: event_thread_id,
                    method,
                } if event_thread_id.as_deref().is_none_or(|id| id == thread_id) => {
                    sink.emit_turn_notice(
                        &thread_id,
                        &format!("Craftward does not support the server request {method}."),
                    );
                }
                TurnStreamEvent::RuntimeError {
                    thread_id: event_thread_id,
                    message,
                    will_retry,
                    ..
                } if event_thread_id == &thread_id => {
                    if *will_retry {
                        sink.emit_turn_notice(&thread_id, message);
                    } else {
                        terminal_error = Some(message.clone());
                    }
                }
                _ => {}
            }

            let conversation_changed =
                apply_turn_stream_event(&mut live_conversation, &event, &thread_id);
            let is_delta = matches!(
                &event,
                TurnStreamEvent::AgentMessageDelta { .. }
                    | TurnStreamEvent::ActivityOutputDelta { .. }
            );
            if conversation_changed
                && (!is_delta || last_live_emit.elapsed() >= LIVE_DELTA_EMIT_INTERVAL)
                && let Some(thread) = live_conversation.as_ref()
            {
                sink.emit_conversation_updated(&thread_id, thread.clone());
                last_live_emit = Instant::now();
            }
            if let TurnStreamEvent::TurnCompleted {
                thread_id: event_thread_id,
                turn,
            } = &event
                && event_thread_id == &thread_id
            {
                if turn.status == TurnStatus::Failed {
                    sink.emit_turn_error(
                        &thread_id,
                        terminal_error.as_deref().unwrap_or("The Codex turn failed."),
                    );
                } else {
                    sink.emit_turn_completed(&thread_id);
                }
            }
        });

        match result {
            Ok(()) => {
                self.latest_conversation = live_conversation.clone();
                self.retained_live_turn =
                    retained_live_turn(&live_conversation, live_turn_id.as_deref());
                true
            }
            Err(_) if self.cancellation.is_cancelled() => false,
            Err(error) => {
                if let (Some(thread), Some(turn_id)) =
                    (live_conversation.as_mut(), live_turn_id.as_deref())
                {
                    mark_turn_failed(thread, turn_id);
                    sink.emit_conversation_updated(&thread_id, thread.clone());
                }
                self.latest_conversation = live_conversation;
                self.retained_live_turn =
                    retained_live_turn(&self.latest_conversation, live_turn_id.as_deref());
                self.writer = None;
                let (status, message) = if error.is_thread_writer_conflict() {
                    (wire::ThreadWriteStatus::Busy, None)
                } else {
                    (
                        wire::ThreadWriteStatus::Unavailable,
                        Some(error.to_string()),
                    )
                };
                sink.emit_thread_write_state(&thread_id, status, message.as_deref());
                sink.emit_turn_error(&thread_id, &error.to_string());
                false
            }
        }
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

enum WriteAccessEffect {
    Acquired(Box<(CodexThreadWriter, Thread)>),
    Busy,
    Unavailable(String),
    Cancelled,
}

fn classify_write_access_result(
    result: Result<(CodexThreadWriter, Thread), CodexError>,
    cancelled: bool,
) -> WriteAccessEffect {
    match result {
        Ok(acquired) => WriteAccessEffect::Acquired(Box::new(acquired)),
        Err(_) if cancelled => WriteAccessEffect::Cancelled,
        Err(error) if error.is_thread_writer_conflict() => WriteAccessEffect::Busy,
        Err(error) => WriteAccessEffect::Unavailable(error.to_string()),
    }
}

fn apply_turn_stream_event(
    conversation: &mut Option<Thread>,
    event: &TurnStreamEvent,
    expected_thread_id: &str,
) -> bool {
    let Some(thread) = conversation.as_mut() else {
        return false;
    };
    match event {
        TurnStreamEvent::TurnStarted { thread_id, turn }
        | TurnStreamEvent::TurnCompleted { thread_id, turn }
            if thread_id == expected_thread_id =>
        {
            merge_turn(thread, turn);
            true
        }
        TurnStreamEvent::ItemStarted {
            thread_id,
            turn_id,
            item,
        }
        | TurnStreamEvent::ItemCompleted {
            thread_id,
            turn_id,
            item,
        } if thread_id == expected_thread_id => {
            let turn = turn_mut_or_insert(thread, turn_id);
            upsert_item(&mut turn.items, item.clone());
            true
        }
        TurnStreamEvent::AgentMessageDelta {
            thread_id,
            turn_id,
            item_id,
            delta,
        } if thread_id == expected_thread_id => {
            let turn = turn_mut_or_insert(thread, turn_id);
            if let Some(ThreadItem::AgentMessage { text, .. }) = turn
                .items
                .iter_mut()
                .find(|item| thread_item_id(item) == item_id)
            {
                text.push_str(delta);
            } else {
                turn.items.push(ThreadItem::AgentMessage {
                    id: item_id.clone(),
                    text: delta.clone(),
                    phase: None,
                });
            }
            true
        }
        TurnStreamEvent::ActivityOutputDelta {
            thread_id,
            turn_id,
            item_id,
            delta,
        } if thread_id == expected_thread_id => {
            let turn = turn_mut_or_insert(thread, turn_id);
            let Some(ThreadItem::Activity(activity)) = turn
                .items
                .iter_mut()
                .find(|item| thread_item_id(item) == item_id)
            else {
                return false;
            };
            activity.detail.get_or_insert_default().push_str(delta);
            true
        }
        _ => false,
    }
}

fn merge_turn(thread: &mut Thread, update: &Turn) {
    if let Some(turn) = thread.turns.iter_mut().find(|turn| turn.id == update.id) {
        turn.status = update.status.clone();
        for item in &update.items {
            upsert_item(&mut turn.items, item.clone());
        }
    } else {
        thread.turns.push(update.clone());
    }
}

fn turn_mut_or_insert<'a>(thread: &'a mut Thread, turn_id: &str) -> &'a mut Turn {
    if let Some(index) = thread.turns.iter().position(|turn| turn.id == turn_id) {
        return &mut thread.turns[index];
    }
    thread.turns.push(Turn {
        id: turn_id.to_owned(),
        status: TurnStatus::InProgress,
        items: Vec::new(),
    });
    thread
        .turns
        .last_mut()
        .expect("the missing turn was appended above")
}

fn upsert_item(items: &mut Vec<ThreadItem>, item: ThreadItem) {
    if let Some(index) = items
        .iter()
        .position(|existing| thread_item_id(existing) == thread_item_id(&item))
    {
        items[index] = item;
    } else {
        items.push(item);
    }
}

fn thread_item_id(item: &ThreadItem) -> &str {
    match item {
        ThreadItem::UserMessage { id, .. }
        | ThreadItem::AgentMessage { id, .. }
        | ThreadItem::Other { id, .. } => id,
        ThreadItem::Activity(activity) => &activity.id,
        _ => "",
    }
}

fn retained_live_turn(
    conversation: &Option<Thread>,
    turn_id: Option<&str>,
) -> Option<RetainedLiveTurn> {
    let conversation = conversation.as_ref()?;
    let index = conversation
        .turns
        .iter()
        .position(|turn| Some(turn.id.as_str()) == turn_id)?;
    Some(RetainedLiveTurn {
        index,
        turn: conversation.turns[index].clone(),
    })
}

fn turn_covers(snapshot: &Turn, retained: &Turn) -> bool {
    snapshot.id == retained.id
        && snapshot.status == retained.status
        && retained.items.iter().all(|retained_item| {
            snapshot.items.iter().any(|snapshot_item| {
                thread_item_id(snapshot_item) == thread_item_id(retained_item)
                    && snapshot_item == retained_item
            })
        })
}

fn merge_retained_live_turn(mut snapshot: Thread, retained: &RetainedLiveTurn) -> Thread {
    if let Some(index) = snapshot
        .turns
        .iter()
        .position(|turn| turn.id == retained.turn.id)
    {
        snapshot.turns[index] = retained.turn.clone();
    } else {
        snapshot.turns.insert(
            retained.index.min(snapshot.turns.len()),
            retained.turn.clone(),
        );
    }
    snapshot
}

fn mark_turn_failed(thread: &mut Thread, turn_id: &str) {
    let Some(turn) = thread.turns.iter_mut().find(|turn| turn.id == turn_id) else {
        return;
    };
    turn.status = TurnStatus::Failed;
    for item in &mut turn.items {
        if let ThreadItem::Activity(activity) = item
            && activity.status == ward_codex::ActivityStatus::InProgress
        {
            activity.status = ward_codex::ActivityStatus::Failed;
        }
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
/// point to NUL-terminated strings. `output_error`, when non-null, must be
/// writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_history_observer_start_turn(
    observer: *mut WardCodexHistoryObserver,
    thread_id: *const c_char,
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
    let Some(prompt) = (unsafe { required_string(prompt, "the Codex prompt", output_error) })
    else {
        return false;
    };
    if prompt.trim().is_empty() {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, "the Codex prompt is empty") };
        return false;
    }

    send_command(
        observer,
        ObserverCommand::StartTurn(TurnRequest { thread_id, prompt }),
        output_error,
    )
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
                    if let Some(request) = update.write_access {
                        match request {
                            WriteAccessRequest::Acquire(thread_id)
                                if watched_thread.as_deref() == Some(thread_id.as_str()) =>
                            {
                                state.acquire_write(&thread_id, &sink);
                            }
                            WriteAccessRequest::Acquire(_) => {}
                            WriteAccessRequest::Release(thread_id) => {
                                state.release_write(&thread_id, &sink);
                            }
                        }
                        if cancellation.is_cancelled() {
                            return;
                        }
                    }
                    for request in update.turns {
                        if watched_thread.as_deref() != Some(request.thread_id.as_str()) {
                            state.select_thread();
                            watched_thread = Some(request.thread_id.clone());
                        }
                        let succeeded = state.run_turn(request, &sink);
                        if cancellation.is_cancelled() {
                            return;
                        }
                        threads_due = Instant::now();
                        conversation_due = Some(
                            Instant::now()
                                + if succeeded {
                                    CONVERSATION_POLL_INTERVAL
                                } else {
                                    HISTORY_ERROR_RETRY_INTERVAL
                                },
                        );
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
            ObserverCommand::AcquireWrite(thread_id) => {
                update.write_access = Some(WriteAccessRequest::Acquire(thread_id));
            }
            ObserverCommand::ReleaseWrite(thread_id) => {
                update.write_access = Some(WriteAccessRequest::Release(thread_id));
            }
            ObserverCommand::StartTurn(request) => update.turns.push(request),
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
    use ward_codex::{
        Activity, ActivityKind, ActivityStatus, AgentMessagePhase, ThreadItem, ThreadSummary, Turn,
        TurnStatus,
    };

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
    fn serializes_thread_write_state_for_the_selected_thread() {
        let captured = Mutex::new(CapturedEvent::default());
        event_sink(&captured).emit_thread_write_state(
            "thread-1",
            wire::ThreadWriteStatus::Busy,
            Some("open elsewhere"),
        );

        let captured = captured.lock().unwrap();
        let event = captured.events.first().unwrap();
        assert_eq!(
            event.kind,
            wire::HistoryEventKind::ThreadWriteStateChanged as i32
        );
        assert_eq!(event.thread_id.as_deref(), Some("thread-1"));
        let Some(wire::history_event::Body::ThreadWriteState(state)) = event.body.as_ref() else {
            panic!("the event must contain a thread write state");
        };
        assert_eq!(state.status, wire::ThreadWriteStatus::Busy as i32);
        assert_eq!(state.message.as_deref(), Some("open elsewhere"));
    }

    #[test]
    fn applies_live_items_and_authoritative_completion_in_order() {
        let mut conversation = Some(thread());
        let command = |status, detail| {
            ThreadItem::Activity(Activity {
                id: "command-1".to_owned(),
                kind: ActivityKind::CommandExecution,
                status,
                summary: "cargo test".to_owned(),
                detail,
                context: Some("/workspace".to_owned()),
                command_actions: vec![],
            })
        };
        let started = TurnStreamEvent::TurnStarted {
            thread_id: "thread-1".to_owned(),
            turn: Turn {
                id: "turn-2".to_owned(),
                status: TurnStatus::InProgress,
                items: vec![],
            },
        };
        assert!(apply_turn_stream_event(
            &mut conversation,
            &started,
            "thread-1"
        ));
        assert!(apply_turn_stream_event(
            &mut conversation,
            &TurnStreamEvent::ItemStarted {
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-2".to_owned(),
                item: command(ActivityStatus::InProgress, None),
            },
            "thread-1"
        ));
        assert!(apply_turn_stream_event(
            &mut conversation,
            &TurnStreamEvent::ActivityOutputDelta {
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-2".to_owned(),
                item_id: "command-1".to_owned(),
                delta: "running\n".to_owned(),
            },
            "thread-1"
        ));
        assert!(apply_turn_stream_event(
            &mut conversation,
            &TurnStreamEvent::ItemCompleted {
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-2".to_owned(),
                item: command(ActivityStatus::Completed, Some("passed".to_owned())),
            },
            "thread-1"
        ));
        assert!(apply_turn_stream_event(
            &mut conversation,
            &TurnStreamEvent::ItemCompleted {
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-2".to_owned(),
                item: ThreadItem::AgentMessage {
                    id: "final-1".to_owned(),
                    text: "Done".to_owned(),
                    phase: Some(AgentMessagePhase::FinalAnswer),
                },
            },
            "thread-1"
        ));

        let thread = conversation.unwrap();
        let live_turn = thread.turns.last().unwrap();
        assert_eq!(live_turn.id, "turn-2");
        assert_eq!(live_turn.items.len(), 2);
        let ThreadItem::Activity(activity) = &live_turn.items[0] else {
            panic!("the first item should remain the command activity");
        };
        assert_eq!(activity.status, ActivityStatus::Completed);
        assert_eq!(activity.detail.as_deref(), Some("passed"));
        assert!(matches!(
            live_turn.items[1],
            ThreadItem::AgentMessage {
                phase: Some(AgentMessagePhase::FinalAnswer),
                ..
            }
        ));
    }

    #[test]
    fn retains_a_partial_live_turn_without_hiding_new_persisted_turns() {
        let retained = RetainedLiveTurn {
            index: 1,
            turn: Turn {
                id: "turn-2".to_owned(),
                status: TurnStatus::Completed,
                items: vec![ThreadItem::Activity(Activity {
                    id: "command-2".to_owned(),
                    kind: ActivityKind::CommandExecution,
                    status: ActivityStatus::Completed,
                    summary: "cargo test".to_owned(),
                    detail: Some("passed".to_owned()),
                    context: Some("/workspace".to_owned()),
                    command_actions: vec![],
                })],
            },
        };
        let mut persisted = thread();
        persisted.turns.push(Turn {
            id: "turn-2".to_owned(),
            status: TurnStatus::Completed,
            items: vec![],
        });
        persisted.turns.push(Turn {
            id: "turn-3".to_owned(),
            status: TurnStatus::Completed,
            items: vec![ThreadItem::AgentMessage {
                id: "final-3".to_owned(),
                text: "External answer".to_owned(),
                phase: Some(AgentMessagePhase::FinalAnswer),
            }],
        });

        assert!(!turn_covers(&persisted.turns[1], &retained.turn));
        let merged = merge_retained_live_turn(persisted, &retained);

        assert_eq!(
            merged
                .turns
                .iter()
                .map(|turn| turn.id.as_str())
                .collect::<Vec<_>>(),
            ["turn-1", "turn-2", "turn-3"]
        );
        assert_eq!(merged.turns[1], retained.turn);
        assert_eq!(merged.turns[2].items.len(), 1);
    }

    #[test]
    fn marks_only_the_interrupted_live_turn_as_failed() {
        let mut conversation = thread();
        conversation.turns[0].status = TurnStatus::InProgress;
        conversation.turns.push(Turn {
            id: "turn-2".to_owned(),
            status: TurnStatus::InProgress,
            items: vec![ThreadItem::Activity(Activity {
                id: "command-2".to_owned(),
                kind: ActivityKind::CommandExecution,
                status: ActivityStatus::InProgress,
                summary: "cargo test".to_owned(),
                detail: None,
                context: Some("/workspace".to_owned()),
                command_actions: vec![],
            })],
        });

        mark_turn_failed(&mut conversation, "turn-2");

        assert_eq!(conversation.turns[0].status, TurnStatus::InProgress);
        assert_eq!(conversation.turns[1].status, TurnStatus::Failed);
        let ThreadItem::Activity(activity) = &conversation.turns[1].items[0] else {
            panic!("the live turn should contain one activity");
        };
        assert_eq!(activity.status, ActivityStatus::Failed);
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
    fn classifies_an_active_writer_conflict_as_busy_write_access() {
        let effect = classify_write_access_result(
            Err(CodexError::Server {
                method: "thread/resume",
                code: -32600,
                message: "thread thread-1 already has an active writer".to_owned(),
            }),
            false,
        );

        assert!(matches!(effect, WriteAccessEffect::Busy));
    }

    #[test]
    fn coalesces_commands_and_prioritizes_stop() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(ObserverCommand::Watch("thread-2".to_owned()))
            .unwrap();
        sender.send(ObserverCommand::Refresh).unwrap();
        sender
            .send(ObserverCommand::AcquireWrite("thread-2".to_owned()))
            .unwrap();
        sender
            .send(ObserverCommand::StartTurn(TurnRequest {
                thread_id: "thread-2".to_owned(),
                prompt: "Continue".to_owned(),
            }))
            .unwrap();
        assert_eq!(
            drain_commands(ObserverCommand::Watch("thread-1".to_owned()), &receiver),
            DrainedCommands::Update(CommandUpdate {
                watched_thread: Some("thread-2".to_owned()),
                refresh: true,
                write_access: Some(WriteAccessRequest::Acquire("thread-2".to_owned())),
                turns: vec![TurnRequest {
                    thread_id: "thread-2".to_owned(),
                    prompt: "Continue".to_owned(),
                }],
            })
        );

        sender.send(ObserverCommand::Stop).unwrap();
        assert_eq!(
            drain_commands(ObserverCommand::Refresh, &receiver),
            DrainedCommands::Stop
        );
    }

    #[test]
    fn keeps_only_the_latest_write_access_intent() {
        let (sender, receiver) = mpsc::channel();
        sender
            .send(ObserverCommand::ReleaseWrite("thread-1".to_owned()))
            .unwrap();

        assert_eq!(
            drain_commands(
                ObserverCommand::AcquireWrite("thread-1".to_owned()),
                &receiver,
            ),
            DrainedCommands::Update(CommandUpdate {
                write_access: Some(WriteAccessRequest::Release("thread-1".to_owned())),
                ..CommandUpdate::default()
            })
        );
    }
}
