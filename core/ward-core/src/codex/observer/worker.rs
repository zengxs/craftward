// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc::Receiver;
use tokio::time::{Instant as TokioInstant, MissedTickBehavior};
use ward_codex::{
    CodexError, CodexHistoryCancellation, CodexHistorySession, CodexThreadWriter, Thread,
    ThreadListOptions, ThreadPagePoll, ThreadPoll, ThreadStartOptions, ThreadStreamEvent,
    ThreadSubscription, TurnStatus,
};

use super::super::live::{LiveRuntimeState, LiveThreadProjection, event_is_incremental};
use super::super::wire;
use super::ObserverOperationGate;
use super::commands::{
    CommandUpdate, DrainedCommands, ObserverCommand, ThreadControlRequest, ThreadStartRequest,
    TurnRequest, WriteAccessRequest, drain_commands, merge_command,
};
use super::events::HistoryEventSink;

const THREAD_PAGE_POLL_INTERVAL: Duration = Duration::from_secs(2);
const CONVERSATION_POLL_INTERVAL: Duration = Duration::from_millis(500);
const LIVE_DELTA_EMIT_INTERVAL: Duration = Duration::from_millis(50);
const HISTORY_ERROR_RETRY_INTERVAL: Duration = Duration::from_secs(2);
const THREAD_PAGE_LIMIT: u32 = 100;

#[cfg(test)]
mod tests;

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

    async fn start_thread(
        &mut self,
        request: ThreadStartRequest,
        sink: &HistoryEventSink,
    ) -> Option<String> {
        let result = CodexThreadWriter::start_with_cancellation(
            &self.executable,
            self.cancellation.clone(),
            &request.working_directory,
            ThreadStartOptions::default(),
        )
        .await;
        match classify_thread_start_result(result, self.cancellation.is_cancelled()) {
            ThreadStartEffect::Started(started) => {
                let (writer, subscription) = *started;
                let thread_id = writer.thread_id().to_owned();
                self.select_thread().await;
                self.live.attach(subscription);
                self.writer = Some(writer);
                self.refresh();

                let thread = self
                    .live
                    .conversation()
                    .cloned()
                    .expect("a started writer always includes its thread snapshot");
                sink.emit_thread_started(&thread_id, thread);
                sink.emit_pending_interactions(&thread_id, std::iter::empty());
                sink.emit_thread_runtime_state(&thread_id, self.live.runtime());
                sink.emit_thread_write_state(&thread_id, wire::ThreadWriteStatus::Writable, None);
                Some(thread_id)
            }
            ThreadStartEffect::Failed(message) => {
                sink.emit_thread_start_error(&message);
                None
            }
            ThreadStartEffect::Cancelled => None,
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
                sink.emit_pending_interactions(thread_id, std::iter::empty());
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
        sink.emit_pending_interactions(thread_id, std::iter::empty());
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
        match &event {
            ThreadStreamEvent::PendingInteractionsUpdated {
                thread_id: event_thread_id,
                interactions,
            } if event_thread_id == thread_id => {
                sink.emit_pending_interactions(thread_id, interactions.iter().cloned());
            }
            ThreadStreamEvent::UnsupportedServerRequest {
                thread_id: event_thread_id,
                method,
            } if event_thread_id.as_deref().is_none_or(|id| id == thread_id) => {
                sink.emit_turn_notice(
                    thread_id,
                    &format!("Craftward does not support the server request {method}."),
                );
            }
            ThreadStreamEvent::RuntimeError {
                thread_id: event_thread_id,
                message,
                will_retry,
                ..
            } if event_thread_id == thread_id => {
                if *will_retry {
                    sink.emit_turn_notice(thread_id, message);
                } else {
                    self.terminal_error = Some(message.clone());
                }
            }
            _ => {}
        }

        let incremental = event_is_incremental(&event);
        let effect = self.live.apply_event(&event, thread_id);
        if effect.runtime_changed {
            sink.emit_thread_runtime_state(thread_id, self.live.runtime());
        }
        if effect.started_turn_id.is_some() {
            self.terminal_error = None;
            sink.emit_turn_started(thread_id);
        }
        if effect.conversation_changed {
            if incremental {
                self.pending_conversation_emit = true;
            } else {
                emit_live_conversation(&self.live, thread_id, sink);
                self.pending_conversation_emit = false;
            }
        }

        if let ThreadStreamEvent::TurnCompleted {
            thread_id: event_thread_id,
            turn,
        } = &event
            && event_thread_id == thread_id
        {
            if turn.status == TurnStatus::Failed {
                sink.emit_turn_error(
                    thread_id,
                    self.terminal_error
                        .as_deref()
                        .unwrap_or("The Codex turn failed."),
                );
            } else {
                sink.emit_turn_completed(thread_id);
            }
            self.terminal_error = None;
        }
    }

    fn flush_pending_live_conversation(&mut self, thread_id: &str, sink: &HistoryEventSink) {
        if !std::mem::take(&mut self.pending_conversation_emit) {
            return;
        }
        emit_live_conversation(&self.live, thread_id, sink);
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
        sink.emit_pending_interactions(thread_id, std::iter::empty());
    }

    async fn apply_thread_control(
        &mut self,
        control: ThreadControlRequest,
        sink: &HistoryEventSink,
    ) {
        match control {
            ThreadControlRequest::Interrupt(thread_id) => {
                let turn_id = match self.live.runtime() {
                    LiveRuntimeState::Active {
                        turn_id: Some(turn_id),
                        ..
                    } => turn_id,
                    _ => {
                        sink.emit_turn_notice(
                            &thread_id,
                            "The Codex turn is no longer available to stop.",
                        );
                        return;
                    }
                };
                let Some(writer) = self
                    .writer
                    .as_mut()
                    .filter(|writer| writer.thread_id() == thread_id)
                else {
                    sink.emit_turn_notice(
                        &thread_id,
                        "Writing access is no longer available for this conversation.",
                    );
                    return;
                };
                if let Err(error) = writer.interrupt_turn(&turn_id).await {
                    sink.emit_turn_notice(&thread_id, &error.to_string());
                }
            }
            ThreadControlRequest::ResolveInteraction(response) => {
                let Some(writer) = self.writer.as_mut() else {
                    return;
                };
                let thread_id = writer.thread_id().to_owned();
                match writer.resolve_interaction(response).await {
                    Ok(event) => self.accept_writer_event(&thread_id, event, sink),
                    Err(error) => {
                        sink.emit_turn_notice(&thread_id, &error.to_string());
                        sink.emit_pending_interactions(&thread_id, writer.pending_interactions());
                    }
                }
            }
        }
    }

    async fn apply_thread_controls(
        &mut self,
        controls: Vec<ThreadControlRequest>,
        sink: &HistoryEventSink,
    ) {
        for control in controls {
            self.apply_thread_control(control, sink).await;
        }
    }

    async fn run_turn(
        &mut self,
        request: TurnRequest,
        sink: &HistoryEventSink,
        receiver: &mut Receiver<ObserverCommand>,
        mut initial_controls: Vec<ThreadControlRequest>,
    ) -> OperationDrive<bool> {
        self.conversation_health.reset();
        let TurnRequest {
            thread_id,
            prompt,
            options,
        } = request;
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
            return OperationDrive::Completed {
                output: false,
                deferred: None,
            };
        }

        self.flush_pending_live_conversation(&thread_id, sink);
        self.terminal_error = None;
        if self.live.begin_turn() {
            sink.emit_thread_runtime_state(&thread_id, self.live.runtime());
        }
        let started = {
            let writer = self
                .writer
                .as_mut()
                .expect("the matching writer was checked above");
            drive_operation(
                writer.begin_text_turn(&prompt, options),
                receiver,
                &self.cancellation,
            )
            .await
        };
        let OperationDrive::Completed {
            output: result,
            deferred,
        } = started
        else {
            return OperationDrive::Stop;
        };
        let event = match result {
            Ok(event) => event,
            Err(_) if self.cancellation.is_cancelled() => {
                return OperationDrive::Completed {
                    output: false,
                    deferred,
                };
            }
            Err(error) => {
                self.fail_writer_stream(&thread_id, error, sink).await;
                return OperationDrive::Completed {
                    output: false,
                    deferred,
                };
            }
        };
        let turn_id = match &event {
            ThreadStreamEvent::TurnStarted { turn, .. } => turn.id.clone(),
            _ => unreachable!("beginning a turn always produces its started event"),
        };
        self.accept_writer_event(&thread_id, event, sink);

        let mut deferred = deferred.unwrap_or_default();
        initial_controls.append(&mut deferred.controls);
        self.apply_thread_controls(initial_controls, sink).await;

        let mut emit_interval = tokio::time::interval(LIVE_DELTA_EMIT_INTERVAL);
        emit_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        emit_interval.tick().await;
        let cancellation = self.cancellation.clone();
        loop {
            let wake = tokio::select! {
                biased;
                _ = cancellation.cancelled() => return OperationDrive::Stop,
                command = receiver.recv() => ObserverWake::Command(command),
                update = self.next_writer_update() => ObserverWake::Writer(Box::new(update)),
                _ = emit_interval.tick() => ObserverWake::Timer,
            };
            match wake {
                ObserverWake::Cancelled => return OperationDrive::Stop,
                ObserverWake::Command(None) => return OperationDrive::Stop,
                ObserverWake::Command(Some(command)) => match drain_commands(command, receiver) {
                    DrainedCommands::Stop => return OperationDrive::Stop,
                    DrainedCommands::Update(mut update) => {
                        self.apply_thread_controls(std::mem::take(&mut update.controls), sink)
                            .await;
                        deferred.merge(update);
                    }
                },
                ObserverWake::Writer(update) => match *update {
                    WriterStreamUpdate::Event {
                        thread_id: event_thread_id,
                        event,
                    } => {
                        let completed = matches!(
                            &event,
                            ThreadStreamEvent::TurnCompleted { thread_id, turn }
                                if thread_id == &event_thread_id && turn.id == turn_id
                        );
                        self.accept_writer_event(&event_thread_id, event, sink);
                        if completed {
                            self.flush_pending_live_conversation(&thread_id, sink);
                            return OperationDrive::Completed {
                                output: true,
                                deferred: (!deferred.is_empty()).then_some(deferred),
                            };
                        }
                    }
                    WriterStreamUpdate::Error {
                        thread_id: event_thread_id,
                        error,
                    } => {
                        if self.cancellation.is_cancelled() {
                            return OperationDrive::Stop;
                        }
                        self.fail_writer_stream(&event_thread_id, error, sink).await;
                        return OperationDrive::Completed {
                            output: false,
                            deferred: (!deferred.is_empty()).then_some(deferred),
                        };
                    }
                },
                ObserverWake::Timer => {
                    self.flush_pending_live_conversation(&thread_id, sink);
                }
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

enum ThreadStartEffect {
    Started(Box<(CodexThreadWriter, ThreadSubscription)>),
    Failed(String),
    Cancelled,
}

fn classify_thread_start_result(
    result: Result<(CodexThreadWriter, ThreadSubscription), CodexError>,
    cancelled: bool,
) -> ThreadStartEffect {
    match result {
        Ok(started) => ThreadStartEffect::Started(Box::new(started)),
        Err(_) if cancelled => ThreadStartEffect::Cancelled,
        Err(error) => ThreadStartEffect::Failed(error.to_string()),
    }
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

fn emit_live_conversation(live: &LiveThreadProjection, thread_id: &str, sink: &HistoryEventSink) {
    if let Some(thread) = live.conversation().cloned() {
        sink.emit_conversation_updated(thread_id, thread);
    }
}

pub(super) async fn run_observer(
    executable: PathBuf,
    mut receiver: Receiver<ObserverCommand>,
    sink: HistoryEventSink,
    cancellation: CodexHistoryCancellation,
    active_operation: Arc<ObserverOperationGate>,
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
                    if update.is_exclusive_operation_only()
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
                        thread_start,
                        turn,
                        controls,
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
                        if let Some(thread_id) = watched_thread.as_deref() {
                            sink.emit_pending_interactions(thread_id, std::iter::empty());
                        }
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
                    if let Some(request) = thread_start {
                        let started = drive_operation(
                            state.start_thread(request, &sink),
                            &mut receiver,
                            &cancellation,
                        )
                        .await;
                        active_operation.release();
                        let OperationDrive::Completed {
                            output: started_thread_id,
                            deferred,
                        } = started
                        else {
                            break 'observer;
                        };
                        if let Some(deferred) = deferred {
                            following.merge(deferred);
                        }
                        if let Some(thread_id) = started_thread_id {
                            watched_thread = Some(thread_id);
                            let now = TokioInstant::now();
                            threads_due = now;
                            conversation_due = Some(now + CONVERSATION_POLL_INTERVAL);
                            live_emit_due = None;
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
                        } = state
                            .run_turn(request, &sink, &mut receiver, controls)
                            .await
                        else {
                            break 'observer;
                        };
                        active_operation.release();
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
                    } else {
                        state.apply_thread_controls(controls, &sink).await;
                    }
                    if !following.is_empty() {
                        deferred_update = Some(following);
                    }
                }
            },
            None => break,
        }
    }
    active_operation.release();
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
