// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::mpsc::{self, Receiver};
use tokio::time::{Instant as TokioInstant, MissedTickBehavior};
use ward_codex::{
    CodexError, CodexHistoryCancellation, CodexHistorySession, CodexThreadWriter, Thread,
    ThreadListOptions, ThreadPagePoll, ThreadPoll, ThreadStreamEvent, ThreadSubscription,
    TurnStatus,
};

use super::super::live::{LiveRuntimeState, LiveThreadProjection, event_is_incremental};
use super::super::wire;
use super::commands::{
    CommandUpdate, DrainedCommands, ObserverCommand, TurnRequest, WriteAccessRequest,
    drain_commands, merge_command,
};
use super::events::HistoryEventSink;

const THREAD_PAGE_POLL_INTERVAL: Duration = Duration::from_secs(2);
const CONVERSATION_POLL_INTERVAL: Duration = Duration::from_millis(500);
const LIVE_DELTA_EMIT_INTERVAL: Duration = Duration::from_millis(50);
const HISTORY_ERROR_RETRY_INTERVAL: Duration = Duration::from_secs(2);
const THREAD_PAGE_LIMIT: u32 = 100;

pub(super) enum OperationDrive<T> {
    Completed {
        output: T,
        deferred: Option<CommandUpdate>,
    },
    Stop,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum PollSample<T> {
    Updated(T),
    Unchanged,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum PollEffect<T> {
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
pub(super) struct PollHealth {
    last_error: Option<String>,
}

impl PollHealth {
    pub(super) fn observe<T>(
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

    pub(super) fn reset(&mut self) {
        self.last_error = None;
    }
}

pub(super) struct ObserverState {
    executable: PathBuf,
    cancellation: CodexHistoryCancellation,
    session: Option<CodexHistorySession>,
    writer: Option<CodexThreadWriter>,
    thread_page_health: PollHealth,
    conversation_health: PollHealth,
    pub(super) live: LiveThreadProjection,
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
    pub(super) fn new(executable: PathBuf, cancellation: CodexHistoryCancellation) -> Self {
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

    pub(super) async fn select_thread(&mut self) {
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

    pub(super) async fn poll_threads(&mut self, sink: &HistoryEventSink) -> bool {
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

    pub(super) async fn poll_conversation(
        &mut self,
        thread_id: &str,
        sink: &HistoryEventSink,
    ) -> bool {
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

    pub(super) fn accept_writer_event(
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

pub(super) enum WriteAccessEffect {
    Acquired(Box<(CodexThreadWriter, ThreadSubscription)>),
    Busy,
    Unavailable(String),
    Cancelled,
}

pub(super) fn classify_write_access_result(
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

pub(super) fn accept_live_event(
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

pub(super) fn flush_pending_live_conversation(
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

pub(super) async fn run_observer(
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

pub(super) async fn drive_operation<F>(
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
