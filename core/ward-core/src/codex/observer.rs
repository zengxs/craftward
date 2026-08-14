// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{c_char, c_void};
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use prost::Message as _;
use tokio::runtime::Handle;
use tokio::sync::mpsc::{self, Receiver, Sender};
use tokio::task::JoinHandle;
use tokio::time::{Instant as TokioInstant, MissedTickBehavior};
use ward_codex::{
    CodexError, CodexHistoryCancellation, CodexHistorySession, CodexThreadWriter, Thread,
    ThreadActiveFlag, ThreadListOptions, ThreadPage, ThreadPagePoll, ThreadPoll, ThreadStreamEvent,
    ThreadSubscription, TurnStatus,
};

use super::live::{LiveRuntimeState, LiveThreadProjection, event_is_incremental};
use super::{WardBuffer, clear_error, required_string, wire};
use crate::{WardError, write_error};

const THREAD_PAGE_POLL_INTERVAL: Duration = Duration::from_secs(2);
const CONVERSATION_POLL_INTERVAL: Duration = Duration::from_millis(500);
const LIVE_DELTA_EMIT_INTERVAL: Duration = Duration::from_millis(50);
const HISTORY_ERROR_RETRY_INTERVAL: Duration = Duration::from_secs(2);
const THREAD_PAGE_LIMIT: u32 = 100;
const COMMAND_QUEUE_CAPACITY: usize = 64;

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

// SAFETY: This private sink belongs to exactly one observer actor. Tokio may
// move that actor between workers while shared references live across an
// await, but the actor never invokes the callback concurrently.
unsafe impl Sync for HistoryEventSink {}

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

    fn emit_thread_runtime_state(&self, thread_id: &str, state: LiveRuntimeState) {
        let (status, turn_id, active_flags) = match state {
            LiveRuntimeState::Detached => (wire::ThreadRuntimeStatus::Detached, None, vec![]),
            LiveRuntimeState::Starting => (wire::ThreadRuntimeStatus::Starting, None, vec![]),
            LiveRuntimeState::Idle => (wire::ThreadRuntimeStatus::Idle, None, vec![]),
            LiveRuntimeState::Active {
                turn_id,
                active_flags,
            } => (
                wire::ThreadRuntimeStatus::Active,
                turn_id,
                active_flags.into_iter().map(active_flag_to_wire).collect(),
            ),
            LiveRuntimeState::SystemError => (wire::ThreadRuntimeStatus::SystemError, None, vec![]),
            LiveRuntimeState::Unknown(_) => (wire::ThreadRuntimeStatus::Unknown, None, vec![]),
        };
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ThreadRuntimeStateChanged as i32,
            thread_id: Some(thread_id.to_owned()),
            body: Some(wire::history_event::Body::ThreadRuntimeState(
                wire::ThreadRuntimeState {
                    status: status as i32,
                    turn_id,
                    active_flags,
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

fn active_flag_to_wire(flag: ThreadActiveFlag) -> i32 {
    (match flag {
        ThreadActiveFlag::WaitingOnApproval => wire::ThreadActiveFlag::WaitingOnApproval,
        ThreadActiveFlag::WaitingOnUserInput => wire::ThreadActiveFlag::WaitingOnUserInput,
        ThreadActiveFlag::Unknown(_) => wire::ThreadActiveFlag::Unknown,
        _ => wire::ThreadActiveFlag::Unknown,
    }) as i32
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
    turn: Option<TurnRequest>,
}

impl CommandUpdate {
    fn merge(&mut self, newer: Self) {
        if newer.watched_thread.is_some() {
            self.watched_thread = newer.watched_thread;
        }
        self.refresh |= newer.refresh;
        if newer.write_access.is_some() {
            self.write_access = newer.write_access;
        }
        if self.turn.is_none() {
            self.turn = newer.turn;
        }
    }

    fn is_empty(&self) -> bool {
        self.watched_thread.is_none()
            && !self.refresh
            && self.write_access.is_none()
            && self.turn.is_none()
    }

    fn is_turn_only(&self) -> bool {
        self.watched_thread.is_none()
            && !self.refresh
            && self.write_access.is_none()
            && self.turn.is_some()
    }
}

#[derive(Debug, Eq, PartialEq)]
enum DrainedCommands {
    Update(CommandUpdate),
    Stop,
}

enum OperationDrive<T> {
    Completed {
        output: T,
        deferred: Option<CommandUpdate>,
    },
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
    live: LiveThreadProjection,
    terminal_error: Option<String>,
    pending_conversation_emit: bool,
}

enum WriterStreamUpdate {
    Event {
        thread_id: String,
        event: ThreadStreamEvent,
    },
    Error {
        thread_id: String,
        error: CodexError,
    },
}

enum ObserverWake {
    Cancelled,
    Command(Option<ObserverCommand>),
    Writer(Box<WriterStreamUpdate>),
    Timer,
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
            live: LiveThreadProjection::default(),
            terminal_error: None,
            pending_conversation_emit: false,
        }
    }

    async fn select_thread(&mut self) {
        if let Some(writer) = self.writer.take() {
            writer.shutdown().await;
        }
        self.conversation_health.reset();
        self.live.reset();
        self.terminal_error = None;
        self.pending_conversation_emit = false;
        if let Some(session) = self.session.as_mut() {
            session.reset_thread_baseline();
        }
    }

    async fn acquire_write(&mut self, thread_id: &str, sink: &HistoryEventSink) -> bool {
        if self
            .writer
            .as_ref()
            .is_some_and(|writer| writer.thread_id() == thread_id)
        {
            sink.emit_thread_runtime_state(thread_id, self.live.runtime());
            sink.emit_thread_write_state(thread_id, wire::ThreadWriteStatus::Writable, None);
            return true;
        }

        if let Some(writer) = self.writer.take() {
            writer.shutdown().await;
        }
        sink.emit_thread_write_state(thread_id, wire::ThreadWriteStatus::Checking, None);
        let result = CodexThreadWriter::acquire_with_cancellation(
            &self.executable,
            self.cancellation.clone(),
            thread_id,
        )
        .await;
        match classify_write_access_result(result, self.cancellation.is_cancelled()) {
            WriteAccessEffect::Acquired(acquired) => {
                let (writer, subscription) = *acquired;
                self.live.attach(subscription);
                self.terminal_error = None;
                self.pending_conversation_emit = false;
                if let Some(thread) = self.live.conversation().cloned() {
                    sink.emit_conversation_updated(thread_id, thread);
                }
                sink.emit_thread_runtime_state(thread_id, self.live.runtime());
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

    async fn release_write(&mut self, thread_id: &str, sink: &HistoryEventSink) {
        if self
            .writer
            .as_ref()
            .is_some_and(|writer| writer.thread_id() == thread_id)
            && let Some(writer) = self.writer.take()
        {
            writer.shutdown().await;
        }
        self.live.detach();
        self.terminal_error = None;
        self.pending_conversation_emit = false;
        sink.emit_thread_runtime_state(thread_id, self.live.runtime());
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

    async fn poll_threads(&mut self, sink: &HistoryEventSink) -> bool {
        let result = self.poll_thread_page().await.map(|poll| match poll {
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

    async fn poll_conversation(&mut self, thread_id: &str, sink: &HistoryEventSink) -> bool {
        let result = self.poll_thread(thread_id).await.map(|poll| match poll {
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
        if self.live.accept_snapshot(thread)
            && let Some(thread) = self.live.conversation().cloned()
        {
            sink.emit_conversation_updated(thread_id, thread);
        }
    }

    async fn next_writer_update(&mut self) -> WriterStreamUpdate {
        let Some(writer) = self.writer.as_mut() else {
            return std::future::pending().await;
        };
        let thread_id = writer.thread_id().to_owned();
        match writer.next_subscription_event().await {
            Ok(event) => WriterStreamUpdate::Event { thread_id, event },
            Err(error) => WriterStreamUpdate::Error { thread_id, error },
        }
    }

    fn accept_writer_event(
        &mut self,
        thread_id: &str,
        event: ThreadStreamEvent,
        sink: &HistoryEventSink,
    ) {
        accept_live_event(
            &mut self.live,
            event,
            thread_id,
            sink,
            &mut self.terminal_error,
            &mut self.pending_conversation_emit,
        );
    }

    fn flush_pending_live_conversation(&mut self, thread_id: &str, sink: &HistoryEventSink) {
        flush_pending_live_conversation(
            &mut self.pending_conversation_emit,
            &self.live,
            thread_id,
            sink,
        );
    }

    async fn fail_writer_stream(
        &mut self,
        thread_id: &str,
        error: CodexError,
        sink: &HistoryEventSink,
    ) {
        let turn_was_active = matches!(
            self.live.runtime(),
            LiveRuntimeState::Starting | LiveRuntimeState::Active { .. }
        );
        let effect = self.live.fail_stream();
        if effect.conversation_changed {
            emit_live_conversation(&self.live, thread_id, sink);
        }
        let (status, message) = if error.is_thread_writer_conflict() {
            (wire::ThreadWriteStatus::Busy, None)
        } else {
            (
                wire::ThreadWriteStatus::Unavailable,
                Some(error.to_string()),
            )
        };
        sink.emit_thread_write_state(thread_id, status, message.as_deref());
        if effect.runtime_changed {
            sink.emit_thread_runtime_state(thread_id, self.live.runtime());
        }
        if let Some(writer) = self.writer.take() {
            writer.shutdown().await;
        }
        if turn_was_active {
            sink.emit_turn_error(thread_id, &error.to_string());
        }
        self.terminal_error = None;
        self.pending_conversation_emit = false;
    }

    async fn run_turn(&mut self, request: TurnRequest, sink: &HistoryEventSink) -> bool {
        self.conversation_health.reset();
        let TurnRequest { thread_id, prompt } = request;
        if !self
            .writer
            .as_ref()
            .is_some_and(|writer| writer.thread_id() == thread_id)
        {
            let message = "Writing access has not been acquired for this conversation.";
            self.live.detach();
            sink.emit_thread_runtime_state(&thread_id, self.live.runtime());
            sink.emit_thread_write_state(
                &thread_id,
                wire::ThreadWriteStatus::Unavailable,
                Some(message),
            );
            sink.emit_turn_error(&thread_id, message);
            return false;
        }

        self.flush_pending_live_conversation(&thread_id, sink);
        self.terminal_error = None;
        if self.live.begin_turn() {
            sink.emit_thread_runtime_state(&thread_id, self.live.runtime());
        }
        let result = {
            let writer = self
                .writer
                .as_mut()
                .expect("the matching writer was checked above");
            let (event_sender, mut event_receiver) = mpsc::unbounded_channel();
            let turn = writer.start_text_turn(&prompt, move |event| {
                let _ = event_sender.send(event);
            });
            tokio::pin!(turn);
            let mut emit_interval = tokio::time::interval(LIVE_DELTA_EMIT_INTERVAL);
            emit_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
            emit_interval.tick().await;

            loop {
                tokio::select! {
                    result = &mut turn => {
                        while let Ok(event) = event_receiver.try_recv() {
                            accept_live_event(
                                &mut self.live,
                                event,
                                &thread_id,
                                sink,
                                &mut self.terminal_error,
                                &mut self.pending_conversation_emit,
                            );
                        }
                        break result;
                    }
                    event = event_receiver.recv() => {
                        if let Some(event) = event {
                            accept_live_event(
                                &mut self.live,
                                event,
                                &thread_id,
                                sink,
                                &mut self.terminal_error,
                                &mut self.pending_conversation_emit,
                            );
                        }
                    }
                    _ = emit_interval.tick(), if self.pending_conversation_emit => {
                        flush_pending_live_conversation(
                            &mut self.pending_conversation_emit,
                            &self.live,
                            &thread_id,
                            sink,
                        );
                    }
                }
            }
        };

        self.flush_pending_live_conversation(&thread_id, sink);

        match result {
            Ok(()) => true,
            Err(_) if self.cancellation.is_cancelled() => false,
            Err(error) => {
                self.fail_writer_stream(&thread_id, error, sink).await;
                false
            }
        }
    }

    async fn poll_thread_page(&mut self) -> Result<ThreadPagePoll, CodexError> {
        self.ensure_session().await?;
        self.session
            .as_mut()
            .expect("the history session was initialized above")
            .poll_thread_page(&ThreadListOptions {
                limit: Some(THREAD_PAGE_LIMIT),
                ..ThreadListOptions::default()
            })
            .await
    }

    async fn poll_thread(&mut self, thread_id: &str) -> Result<ThreadPoll, CodexError> {
        self.ensure_session().await?;
        self.session
            .as_mut()
            .expect("the history session was initialized above")
            .poll_thread(thread_id)
            .await
    }

    async fn ensure_session(&mut self) -> Result<(), CodexError> {
        if self.session.is_none() {
            self.session = Some(
                CodexHistorySession::spawn_with_cancellation(
                    &self.executable,
                    self.cancellation.clone(),
                )
                .await?,
            );
        }
        Ok(())
    }

    async fn shutdown(mut self) {
        if let Some(writer) = self.writer.take() {
            writer.shutdown().await;
        }
        if let Some(session) = self.session.take() {
            session.shutdown().await;
        }
    }
}

enum WriteAccessEffect {
    Acquired(Box<(CodexThreadWriter, ThreadSubscription)>),
    Busy,
    Unavailable(String),
    Cancelled,
}

fn classify_write_access_result(
    result: Result<(CodexThreadWriter, ThreadSubscription), CodexError>,
    cancelled: bool,
) -> WriteAccessEffect {
    match result {
        Ok(acquired) => WriteAccessEffect::Acquired(Box::new(acquired)),
        Err(_) if cancelled => WriteAccessEffect::Cancelled,
        Err(error) if error.is_thread_writer_conflict() => WriteAccessEffect::Busy,
        Err(error) => WriteAccessEffect::Unavailable(error.to_string()),
    }
}

fn accept_live_event(
    live: &mut LiveThreadProjection,
    event: ThreadStreamEvent,
    expected_thread_id: &str,
    sink: &HistoryEventSink,
    terminal_error: &mut Option<String>,
    pending_conversation_emit: &mut bool,
) {
    match &event {
        ThreadStreamEvent::ApprovalDeclined { thread_id, method }
            if thread_id
                .as_deref()
                .is_none_or(|id| id == expected_thread_id) =>
        {
            sink.emit_turn_notice(
                expected_thread_id,
                &format!(
                    "Craftward declined {method} because approval controls are not available yet."
                ),
            );
        }
        ThreadStreamEvent::UnsupportedServerRequest { thread_id, method }
            if thread_id
                .as_deref()
                .is_none_or(|id| id == expected_thread_id) =>
        {
            sink.emit_turn_notice(
                expected_thread_id,
                &format!("Craftward does not support the server request {method}."),
            );
        }
        ThreadStreamEvent::RuntimeError {
            thread_id,
            message,
            will_retry,
            ..
        } if thread_id == expected_thread_id => {
            if *will_retry {
                sink.emit_turn_notice(expected_thread_id, message);
            } else {
                *terminal_error = Some(message.clone());
            }
        }
        _ => {}
    }

    let incremental = event_is_incremental(&event);
    let effect = live.apply_event(&event, expected_thread_id);
    if effect.runtime_changed {
        sink.emit_thread_runtime_state(expected_thread_id, live.runtime());
    }
    if effect.started_turn_id.is_some() {
        *terminal_error = None;
        sink.emit_turn_started(expected_thread_id);
    }
    if effect.conversation_changed {
        if incremental {
            *pending_conversation_emit = true;
        } else {
            emit_live_conversation(live, expected_thread_id, sink);
            *pending_conversation_emit = false;
        }
    }

    if let ThreadStreamEvent::TurnCompleted { thread_id, turn } = &event
        && thread_id == expected_thread_id
    {
        if turn.status == TurnStatus::Failed {
            sink.emit_turn_error(
                expected_thread_id,
                terminal_error
                    .as_deref()
                    .unwrap_or("The Codex turn failed."),
            );
        } else {
            sink.emit_turn_completed(expected_thread_id);
        }
        *terminal_error = None;
    }
}

fn emit_live_conversation(live: &LiveThreadProjection, thread_id: &str, sink: &HistoryEventSink) {
    if let Some(thread) = live.conversation().cloned() {
        sink.emit_conversation_updated(thread_id, thread);
    }
}

fn flush_pending_live_conversation(
    pending: &mut bool,
    live: &LiveThreadProjection,
    thread_id: &str,
    sink: &HistoryEventSink,
) {
    if !std::mem::take(pending) {
        return;
    }
    emit_live_conversation(live, thread_id, sink);
}

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
    let sink = HistoryEventSink {
        callback,
        context: callback_context,
    };
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
        ObserverCommand::StartTurn(TurnRequest { thread_id, prompt }),
        output_error,
    );
    if !sent {
        observer.turn_active.store(false, Ordering::Release);
    }
    sent
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

async fn run_observer(
    executable: PathBuf,
    mut receiver: Receiver<ObserverCommand>,
    sink: HistoryEventSink,
    cancellation: CodexHistoryCancellation,
    turn_active: Arc<AtomicBool>,
) {
    let mut state = ObserverState::new(executable, cancellation.clone());
    let mut watched_thread: Option<String> = None;
    let mut threads_due = TokioInstant::now();
    let mut conversation_due: Option<TokioInstant> = None;
    let mut live_emit_due: Option<TokioInstant> = None;
    let mut deferred_update = None;

    'observer: loop {
        if deferred_update.is_none() {
            let now = TokioInstant::now();
            if live_emit_due.is_some_and(|due| now >= due) {
                if let Some(thread_id) = watched_thread.as_deref() {
                    state.flush_pending_live_conversation(thread_id, &sink);
                }
                live_emit_due = None;
            }
            if now >= threads_due {
                let OperationDrive::Completed {
                    output: succeeded,
                    deferred,
                } = drive_operation(state.poll_threads(&sink), &mut receiver, &cancellation).await
                else {
                    break;
                };
                threads_due = TokioInstant::now()
                    + if succeeded {
                        THREAD_PAGE_POLL_INTERVAL
                    } else {
                        HISTORY_ERROR_RETRY_INTERVAL
                    };
                deferred_update = deferred;
            }

            if deferred_update.is_none()
                && let (Some(thread_id), Some(due)) = (watched_thread.as_deref(), conversation_due)
                && TokioInstant::now() >= due
            {
                let thread_id = thread_id.to_owned();
                let OperationDrive::Completed {
                    output: succeeded,
                    deferred,
                } = drive_operation(
                    state.poll_conversation(&thread_id, &sink),
                    &mut receiver,
                    &cancellation,
                )
                .await
                else {
                    break;
                };
                conversation_due = Some(
                    TokioInstant::now()
                        + if succeeded {
                            CONVERSATION_POLL_INTERVAL
                        } else {
                            HISTORY_ERROR_RETRY_INTERVAL
                        },
                );
                deferred_update = deferred;
            }
        }

        let drained = if let Some(update) = deferred_update.take() {
            Some(DrainedCommands::Update(update))
        } else {
            let next_due = [Some(threads_due), conversation_due, live_emit_due]
                .into_iter()
                .flatten()
                .min()
                .expect("the thread poll always has a deadline");
            let sleep = tokio::time::sleep_until(next_due);
            tokio::pin!(sleep);
            let wake = tokio::select! {
                biased;
                _ = cancellation.cancelled() => ObserverWake::Cancelled,
                command = receiver.recv() => ObserverWake::Command(command),
                update = state.next_writer_update() => ObserverWake::Writer(Box::new(update)),
                () = &mut sleep => ObserverWake::Timer,
            };
            match wake {
                ObserverWake::Cancelled | ObserverWake::Command(None) => None,
                ObserverWake::Command(Some(command)) => {
                    Some(drain_commands(command, &mut receiver))
                }
                ObserverWake::Writer(update) => {
                    match *update {
                        WriterStreamUpdate::Event { thread_id, event } => {
                            if watched_thread.as_deref() == Some(thread_id.as_str()) {
                                state.accept_writer_event(&thread_id, event, &sink);
                                if state.pending_conversation_emit {
                                    live_emit_due.get_or_insert_with(|| {
                                        TokioInstant::now() + LIVE_DELTA_EMIT_INTERVAL
                                    });
                                } else {
                                    live_emit_due = None;
                                }
                            }
                        }
                        WriterStreamUpdate::Error { thread_id, error } => {
                            if cancellation.is_cancelled() {
                                break;
                            }
                            state.fail_writer_stream(&thread_id, error, &sink).await;
                            live_emit_due = None;
                        }
                    }
                    continue;
                }
                ObserverWake::Timer => continue,
            }
        };
        match drained {
            Some(drained) => match drained {
                DrainedCommands::Stop => break,
                DrainedCommands::Update(mut update) => {
                    if update.is_turn_only()
                        && let Ok(command) = receiver.try_recv()
                    {
                        if !merge_command(&mut update, command) {
                            break 'observer;
                        }
                        while let Ok(command) = receiver.try_recv() {
                            if !merge_command(&mut update, command) {
                                break 'observer;
                            }
                        }
                    }
                    let now = TokioInstant::now();
                    let CommandUpdate {
                        watched_thread: requested_thread,
                        refresh,
                        write_access,
                        turn,
                    } = update;
                    let mut following = CommandUpdate::default();

                    if let Some(thread_id) = requested_thread {
                        let OperationDrive::Completed { deferred, .. } =
                            drive_operation(state.select_thread(), &mut receiver, &cancellation)
                                .await
                        else {
                            break 'observer;
                        };
                        if let Some(deferred) = deferred {
                            following.merge(deferred);
                        }
                        watched_thread = Some(thread_id);
                        conversation_due = Some(now);
                        live_emit_due = None;
                    }
                    if refresh {
                        state.refresh();
                        threads_due = now;
                        if watched_thread.is_some() {
                            conversation_due = Some(now);
                        }
                    }
                    if let Some(request) = write_access {
                        match request {
                            WriteAccessRequest::Acquire(thread_id)
                                if watched_thread.as_deref() == Some(thread_id.as_str()) =>
                            {
                                let OperationDrive::Completed { deferred, .. } = drive_operation(
                                    state.acquire_write(&thread_id, &sink),
                                    &mut receiver,
                                    &cancellation,
                                )
                                .await
                                else {
                                    break 'observer;
                                };
                                if let Some(deferred) = deferred {
                                    following.merge(deferred);
                                }
                            }
                            WriteAccessRequest::Acquire(_) => {}
                            WriteAccessRequest::Release(thread_id) => {
                                let OperationDrive::Completed { deferred, .. } = drive_operation(
                                    state.release_write(&thread_id, &sink),
                                    &mut receiver,
                                    &cancellation,
                                )
                                .await
                                else {
                                    break 'observer;
                                };
                                if let Some(deferred) = deferred {
                                    following.merge(deferred);
                                }
                                live_emit_due = None;
                            }
                        }
                    }
                    if let Some(request) = turn {
                        if watched_thread.as_deref() != Some(request.thread_id.as_str()) {
                            let OperationDrive::Completed { deferred, .. } = drive_operation(
                                state.select_thread(),
                                &mut receiver,
                                &cancellation,
                            )
                            .await
                            else {
                                break 'observer;
                            };
                            if let Some(deferred) = deferred {
                                following.merge(deferred);
                            }
                            watched_thread = Some(request.thread_id.clone());
                        }
                        let OperationDrive::Completed {
                            output: succeeded,
                            deferred,
                        } = drive_operation(
                            state.run_turn(request, &sink),
                            &mut receiver,
                            &cancellation,
                        )
                        .await
                        else {
                            break 'observer;
                        };
                        turn_active.store(false, Ordering::Release);
                        if let Some(update) = deferred {
                            following.merge(update);
                        }
                        threads_due = TokioInstant::now();
                        conversation_due = Some(
                            TokioInstant::now()
                                + if succeeded {
                                    CONVERSATION_POLL_INTERVAL
                                } else {
                                    HISTORY_ERROR_RETRY_INTERVAL
                                },
                        );
                    }
                    if !following.is_empty() {
                        deferred_update = Some(following);
                    }
                }
            },
            None => break,
        }
    }
    turn_active.store(false, Ordering::Release);
    state.shutdown().await;
}

async fn drive_operation<F>(
    operation: F,
    receiver: &mut Receiver<ObserverCommand>,
    cancellation: &CodexHistoryCancellation,
) -> OperationDrive<F::Output>
where
    F: Future,
{
    tokio::pin!(operation);
    let mut deferred = CommandUpdate::default();

    loop {
        if cancellation.is_cancelled() {
            let _ = operation.as_mut().await;
            return OperationDrive::Stop;
        }
        tokio::select! {
            _ = cancellation.cancelled() => {
                let _ = operation.as_mut().await;
                return OperationDrive::Stop;
            },
            command = receiver.recv() => {
                let keep_running = command
                    .is_some_and(|command| merge_command(&mut deferred, command));
                if !keep_running {
                    cancellation.cancel();
                    let _ = operation.as_mut().await;
                    return OperationDrive::Stop;
                }
            },
            output = &mut operation => {
                return OperationDrive::Completed {
                    output,
                    deferred: (!deferred.is_empty()).then_some(deferred),
                };
            }
        }
    }
}

fn drain_commands(
    first: ObserverCommand,
    receiver: &mut Receiver<ObserverCommand>,
) -> DrainedCommands {
    let mut update = CommandUpdate::default();
    if !merge_command(&mut update, first) {
        return DrainedCommands::Stop;
    }
    drain_available_commands(&mut update, receiver)
}

fn drain_available_commands(
    update: &mut CommandUpdate,
    receiver: &mut Receiver<ObserverCommand>,
) -> DrainedCommands {
    while let Ok(command) = receiver.try_recv() {
        if !merge_command(update, command) {
            return DrainedCommands::Stop;
        }
    }
    DrainedCommands::Update(std::mem::take(update))
}

fn merge_command(update: &mut CommandUpdate, command: ObserverCommand) -> bool {
    match command {
        ObserverCommand::Watch(thread_id) => update.watched_thread = Some(thread_id),
        ObserverCommand::Refresh => update.refresh = true,
        ObserverCommand::AcquireWrite(thread_id) => {
            update.write_access = Some(WriteAccessRequest::Acquire(thread_id));
        }
        ObserverCommand::ReleaseWrite(thread_id) => {
            update.write_access = Some(WriteAccessRequest::Release(thread_id));
        }
        ObserverCommand::StartTurn(request) if update.turn.is_none() => {
            update.turn = Some(request);
        }
        ObserverCommand::StartTurn(_) => {}
        ObserverCommand::Stop => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use prost::Message as _;
    use tokio::sync::oneshot;
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
    fn serializes_the_subscribed_runtime_state_and_active_flags() {
        let captured = Mutex::new(CapturedEvent::default());
        event_sink(&captured).emit_thread_runtime_state(
            "thread-1",
            LiveRuntimeState::Active {
                turn_id: Some("turn-2".to_owned()),
                active_flags: vec![
                    ThreadActiveFlag::WaitingOnApproval,
                    ThreadActiveFlag::WaitingOnUserInput,
                ],
            },
        );

        let captured = captured.lock().unwrap();
        let event = captured.events.first().unwrap();
        assert_eq!(
            event.kind,
            wire::HistoryEventKind::ThreadRuntimeStateChanged as i32
        );
        let Some(wire::history_event::Body::ThreadRuntimeState(state)) = event.body.as_ref() else {
            panic!("the event must contain a thread runtime state");
        };
        assert_eq!(state.status, wire::ThreadRuntimeStatus::Active as i32);
        assert_eq!(state.turn_id.as_deref(), Some("turn-2"));
        assert_eq!(
            state.active_flags,
            [
                wire::ThreadActiveFlag::WaitingOnApproval as i32,
                wire::ThreadActiveFlag::WaitingOnUserInput as i32,
            ]
        );
    }

    #[test]
    fn projects_an_idle_context_compaction_lifecycle_to_the_timeline() {
        let captured = Mutex::new(CapturedEvent::default());
        let sink = event_sink(&captured);
        let mut state =
            ObserverState::new(PathBuf::from("/codex"), CodexHistoryCancellation::new());
        state.live.attach(ThreadSubscription {
            thread: thread(),
            runtime_status: ward_codex::ThreadRuntimeStatus::Idle,
        });
        let compaction = |status| {
            ThreadItem::Activity(Activity {
                id: "compaction-1".to_owned(),
                kind: ActivityKind::ContextCompaction,
                status,
                started_at_unix_milliseconds: None,
                completed_at_unix_milliseconds: None,
                summary: String::new(),
                detail: None,
                context: None,
                command_actions: vec![],
            })
        };

        state.accept_writer_event(
            "thread-1",
            ThreadStreamEvent::ItemStarted {
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-2".to_owned(),
                item: compaction(ActivityStatus::InProgress),
            },
            &sink,
        );
        assert_projected_activity_status(&captured, wire::ActivityStatus::InProgress);

        state.accept_writer_event(
            "thread-1",
            ThreadStreamEvent::ItemCompleted {
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-2".to_owned(),
                item: compaction(ActivityStatus::Completed),
            },
            &sink,
        );
        assert_projected_activity_status(&captured, wire::ActivityStatus::Completed);
    }

    fn assert_projected_activity_status(
        captured: &Mutex<CapturedEvent>,
        expected: wire::ActivityStatus,
    ) {
        let captured = captured.lock().unwrap();
        let event = captured.events.last().unwrap();
        let Some(wire::history_event::Body::Conversation(conversation)) = event.body.as_ref()
        else {
            panic!("the live event must emit a conversation");
        };
        let Some(wire::timeline_item::Body::Activity(activity)) =
            conversation.timeline.last().unwrap().body.as_ref()
        else {
            panic!("the live timeline item must be an activity");
        };
        assert_eq!(activity.kind, wire::ActivityKind::ContextCompaction as i32);
        assert_eq!(activity.status, expected as i32);
    }

    #[test]
    fn flushes_the_latest_incremental_update_without_a_following_event() {
        let captured = Mutex::new(CapturedEvent::default());
        let sink = event_sink(&captured);
        let mut live = LiveThreadProjection::default();
        live.attach(ThreadSubscription {
            thread: thread(),
            runtime_status: ward_codex::ThreadRuntimeStatus::Idle,
        });
        let mut terminal_error = None;
        let mut pending = false;
        accept_live_event(
            &mut live,
            ThreadStreamEvent::TurnStarted {
                thread_id: "thread-1".to_owned(),
                turn: Turn {
                    id: "turn-2".to_owned(),
                    status: TurnStatus::InProgress,
                    items: vec![],
                },
            },
            "thread-1",
            &sink,
            &mut terminal_error,
            &mut pending,
        );
        captured.lock().unwrap().events.clear();

        accept_live_event(
            &mut live,
            ThreadStreamEvent::AgentMessageDelta {
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-2".to_owned(),
                item_id: "agent-2".to_owned(),
                delta: "Latest text".to_owned(),
            },
            "thread-1",
            &sink,
            &mut terminal_error,
            &mut pending,
        );

        assert!(pending);
        assert!(captured.lock().unwrap().events.is_empty());
        flush_pending_live_conversation(&mut pending, &live, "thread-1", &sink);

        assert!(!pending);
        let captured = captured.lock().unwrap();
        let event = captured.events.first().unwrap();
        let Some(wire::history_event::Body::Conversation(conversation)) = event.body.as_ref()
        else {
            panic!("the trailing flush must contain the latest conversation");
        };
        let Some(wire::timeline_item::Body::Message(message)) =
            conversation.timeline.last().unwrap().body.as_ref()
        else {
            panic!("the trailing item must be the live agent message");
        };
        assert_eq!(message.text, "Latest text");
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

    #[tokio::test]
    async fn suppresses_repeated_identical_errors_for_each_target() {
        let captured = Mutex::new(CapturedEvent::default());
        let sink = event_sink(&captured);
        let mut state = ObserverState::new(
            PathBuf::from("/craftward-tests/missing-codex"),
            CodexHistoryCancellation::new(),
        );

        assert!(!state.poll_threads(&sink).await);
        assert!(!state.poll_threads(&sink).await);
        state.select_thread().await;
        assert!(!state.poll_conversation("thread-1", &sink).await);
        assert!(!state.poll_conversation("thread-1", &sink).await);

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
        let (sender, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
        sender
            .try_send(ObserverCommand::Watch("thread-2".to_owned()))
            .unwrap();
        sender.try_send(ObserverCommand::Refresh).unwrap();
        sender
            .try_send(ObserverCommand::AcquireWrite("thread-2".to_owned()))
            .unwrap();
        sender
            .try_send(ObserverCommand::StartTurn(TurnRequest {
                thread_id: "thread-2".to_owned(),
                prompt: "Continue".to_owned(),
            }))
            .unwrap();
        assert_eq!(
            drain_commands(ObserverCommand::Watch("thread-1".to_owned()), &mut receiver),
            DrainedCommands::Update(CommandUpdate {
                watched_thread: Some("thread-2".to_owned()),
                refresh: true,
                write_access: Some(WriteAccessRequest::Acquire("thread-2".to_owned())),
                turn: Some(TurnRequest {
                    thread_id: "thread-2".to_owned(),
                    prompt: "Continue".to_owned(),
                }),
            })
        );

        sender.try_send(ObserverCommand::Stop).unwrap();
        assert_eq!(
            drain_commands(ObserverCommand::Refresh, &mut receiver),
            DrainedCommands::Stop
        );
    }

    #[test]
    fn keeps_only_the_latest_write_access_intent() {
        let (sender, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
        sender
            .try_send(ObserverCommand::ReleaseWrite("thread-1".to_owned()))
            .unwrap();

        assert_eq!(
            drain_commands(
                ObserverCommand::AcquireWrite("thread-1".to_owned()),
                &mut receiver,
            ),
            DrainedCommands::Update(CommandUpdate {
                write_access: Some(WriteAccessRequest::Release("thread-1".to_owned())),
                ..CommandUpdate::default()
            })
        );
    }

    #[test]
    fn merges_deferred_updates_without_replacing_the_reserved_turn() {
        let mut deferred = CommandUpdate {
            watched_thread: Some("thread-1".to_owned()),
            refresh: false,
            write_access: Some(WriteAccessRequest::Acquire("thread-1".to_owned())),
            turn: Some(TurnRequest {
                thread_id: "thread-1".to_owned(),
                prompt: "First".to_owned(),
            }),
        };

        deferred.merge(CommandUpdate {
            watched_thread: Some("thread-2".to_owned()),
            refresh: true,
            write_access: Some(WriteAccessRequest::Release("thread-1".to_owned())),
            turn: Some(TurnRequest {
                thread_id: "thread-2".to_owned(),
                prompt: "Second".to_owned(),
            }),
        });

        assert_eq!(
            deferred,
            CommandUpdate {
                watched_thread: Some("thread-2".to_owned()),
                refresh: true,
                write_access: Some(WriteAccessRequest::Release("thread-1".to_owned())),
                turn: Some(TurnRequest {
                    thread_id: "thread-1".to_owned(),
                    prompt: "First".to_owned(),
                }),
            }
        );
    }

    #[test]
    fn processes_control_commands_before_a_reserved_turn() {
        let (sender, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
        sender
            .try_send(ObserverCommand::Watch("thread-2".to_owned()))
            .unwrap();
        sender.try_send(ObserverCommand::Refresh).unwrap();
        let mut update = CommandUpdate {
            turn: Some(TurnRequest {
                thread_id: "thread-1".to_owned(),
                prompt: "Continue".to_owned(),
            }),
            ..CommandUpdate::default()
        };

        let first = receiver.try_recv().unwrap();
        assert!(merge_command(&mut update, first));
        while let Ok(command) = receiver.try_recv() {
            assert!(merge_command(&mut update, command));
        }

        assert_eq!(update.watched_thread.as_deref(), Some("thread-2"));
        assert!(update.refresh);
        assert_eq!(update.turn.unwrap().thread_id, "thread-1");
    }

    #[tokio::test]
    async fn accepts_and_coalesces_commands_while_an_operation_is_in_flight() {
        let (sender, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
        let cancellation = CodexHistoryCancellation::new();
        let (start_commands, commands_started) = oneshot::channel();
        let (commands_sent, wait_for_commands) = oneshot::channel();
        let producer = tokio::spawn(async move {
            commands_started.await.unwrap();
            sender
                .send(ObserverCommand::Watch("thread-2".to_owned()))
                .await
                .unwrap();
            sender.send(ObserverCommand::Refresh).await.unwrap();
            commands_sent.send(()).unwrap();
            std::future::pending::<()>().await;
        });
        let operation = async move {
            start_commands.send(()).unwrap();
            wait_for_commands.await.unwrap();
            tokio::time::sleep(Duration::from_millis(10)).await;
            42
        };

        let result = drive_operation(operation, &mut receiver, &cancellation).await;
        producer.abort();

        let OperationDrive::Completed { output, deferred } = result else {
            panic!("the operation should complete");
        };
        assert_eq!(output, 42);
        assert_eq!(
            deferred,
            Some(CommandUpdate {
                watched_thread: Some("thread-2".to_owned()),
                refresh: true,
                ..CommandUpdate::default()
            })
        );
    }
}
