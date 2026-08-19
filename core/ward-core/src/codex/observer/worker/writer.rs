// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::time::Duration;

use tokio::sync::mpsc::Receiver;
use tokio::time::MissedTickBehavior;
use ward_codex::{
    CodexError, CodexHistoryCancellation, CodexThreadWriter, Thread, ThreadStreamEvent,
    ThreadSubscription, TurnStatus,
};

use super::super::super::live::{LiveRuntimeState, LiveThreadProjection, event_is_incremental};
use super::super::super::wire;
use super::super::commands::{
    DrainedCommands, ObserverCommand, ThreadControlRequest, TurnRequest, TurnSteerRequest,
    drain_commands,
};
use super::super::events::HistoryEventSink;
use super::operation::{OperationDrive, drive_operation};

pub(super) const LIVE_DELTA_EMIT_INTERVAL: Duration = Duration::from_millis(50);

pub(super) enum WriterStreamUpdate {
    Event {
        thread_id: String,
        event: ThreadStreamEvent,
    },
    Error {
        thread_id: String,
        error: CodexError,
    },
}

pub(super) enum ObserverWake {
    Cancelled,
    Command(Option<ObserverCommand>),
    Writer(Box<WriterStreamUpdate>),
    Timer,
}

#[derive(Default)]
pub(super) struct WriterRuntime {
    writer: Option<CodexThreadWriter>,
    live: LiveThreadProjection,
    terminal_error: Option<String>,
    pending_conversation_emit: bool,
}

impl WriterRuntime {
    pub(super) fn has_pending_conversation_emit(&self) -> bool {
        self.pending_conversation_emit
    }

    pub(super) fn writer_matches(&self, thread_id: &str) -> bool {
        self.writer
            .as_ref()
            .is_some_and(|writer| writer.thread_id() == thread_id)
    }

    #[cfg(test)]
    pub(super) fn writer_thread_id(&self) -> Option<&str> {
        self.writer.as_ref().map(CodexThreadWriter::thread_id)
    }

    #[cfg(test)]
    pub(super) fn active_model(&self) -> Option<&str> {
        self.writer
            .as_ref()
            .and_then(CodexThreadWriter::active_model)
    }

    #[cfg(test)]
    pub(super) fn active_reasoning_effort(&self) -> Option<&str> {
        self.writer
            .as_ref()
            .and_then(CodexThreadWriter::active_reasoning_effort)
    }

    pub(super) fn runtime(&self) -> LiveRuntimeState {
        self.live.runtime()
    }

    pub(super) fn conversation(&self) -> Option<&Thread> {
        self.live.conversation()
    }

    pub(super) fn forkable_turn_ids(&self) -> &[String] {
        self.live.forkable_turn_ids()
    }

    pub(super) fn attach(&mut self, writer: CodexThreadWriter, subscription: ThreadSubscription) {
        self.live.attach(subscription);
        self.writer = Some(writer);
        self.terminal_error = None;
        self.pending_conversation_emit = false;
    }

    #[cfg(test)]
    pub(super) fn attach_subscription(&mut self, subscription: ThreadSubscription) {
        self.live.attach(subscription);
        self.terminal_error = None;
        self.pending_conversation_emit = false;
    }

    pub(super) fn detach(&mut self) {
        self.live.detach();
        self.terminal_error = None;
        self.pending_conversation_emit = false;
    }

    pub(super) async fn reset(&mut self) {
        if let Some(writer) = self.writer.take() {
            writer.shutdown().await;
        }
        self.live.reset();
        self.terminal_error = None;
        self.pending_conversation_emit = false;
    }

    pub(super) async fn shutdown_writer(&mut self) {
        if let Some(writer) = self.writer.take() {
            writer.shutdown().await;
        }
    }

    pub(super) fn emit_writer_model_state(&self, thread_id: &str, sink: &HistoryEventSink) {
        let Some(writer) = self.writer.as_ref() else {
            return;
        };
        let Some(model) = writer.active_model() else {
            return;
        };
        sink.emit_thread_model_changed(thread_id, model, writer.active_reasoning_effort());
    }

    pub(super) fn accept_polled_conversation(
        &mut self,
        thread_id: &str,
        thread: Thread,
        sink: &HistoryEventSink,
    ) {
        if self.live.accept_snapshot(thread)
            && let Some(thread) = self.live.conversation().cloned()
        {
            sink.emit_conversation_updated(
                thread_id,
                thread,
                self.live.forkable_turn_ids().to_vec(),
            );
        }
    }

    pub(super) async fn next_update(&mut self) -> WriterStreamUpdate {
        let Some(writer) = self.writer.as_mut() else {
            return std::future::pending().await;
        };
        let thread_id = writer.thread_id().to_owned();
        match writer.next_subscription_event().await {
            Ok(event) => WriterStreamUpdate::Event { thread_id, event },
            Err(error) => WriterStreamUpdate::Error { thread_id, error },
        }
    }

    pub(super) fn accept_event(
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
        if effect.started_turn_id.is_some() {
            self.terminal_error = None;
            sink.emit_turn_started(thread_id);
        }
        if effect.runtime_changed {
            sink.emit_thread_runtime_state(thread_id, self.live.runtime());
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

    pub(super) fn flush_pending_conversation(&mut self, thread_id: &str, sink: &HistoryEventSink) {
        if !std::mem::take(&mut self.pending_conversation_emit) {
            return;
        }
        emit_live_conversation(&self.live, thread_id, sink);
    }

    pub(super) async fn fail_stream(
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
        self.shutdown_writer().await;
        if turn_was_active {
            sink.emit_turn_error(thread_id, &error.to_string());
        }
        self.terminal_error = None;
        self.pending_conversation_emit = false;
        sink.emit_pending_interactions(thread_id, std::iter::empty());
    }

    pub(super) async fn apply_control(
        &mut self,
        control: ThreadControlRequest,
        sink: &HistoryEventSink,
    ) {
        match control {
            ThreadControlRequest::Steer(request) => {
                let TurnSteerRequest {
                    thread_id,
                    expected_turn_id,
                    prompt,
                } = request;
                let active_turn_matches = matches!(
                    self.live.runtime(),
                    LiveRuntimeState::Active {
                        turn_id: Some(turn_id),
                        ..
                    } if turn_id == expected_turn_id
                );
                if !active_turn_matches {
                    sink.emit_turn_steer_error(
                        &thread_id,
                        "The active Codex turn changed before the guidance could be sent.",
                    );
                    return;
                }
                let Some(writer) = self
                    .writer
                    .as_mut()
                    .filter(|writer| writer.thread_id() == thread_id)
                else {
                    sink.emit_turn_steer_error(
                        &thread_id,
                        "Writing access is no longer available for this conversation.",
                    );
                    return;
                };
                match writer.steer_text_turn(&expected_turn_id, &prompt).await {
                    Ok(()) => sink.emit_turn_steered(&thread_id),
                    Err(error) => sink.emit_turn_steer_error(&thread_id, &error.to_string()),
                }
            }
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
                    Ok(event) => self.accept_event(&thread_id, event, sink),
                    Err(error) => {
                        sink.emit_turn_notice(&thread_id, &error.to_string());
                        sink.emit_pending_interactions(&thread_id, writer.pending_interactions());
                    }
                }
            }
        }
    }

    pub(super) async fn apply_controls(
        &mut self,
        controls: Vec<ThreadControlRequest>,
        sink: &HistoryEventSink,
    ) {
        for control in controls {
            self.apply_control(control, sink).await;
        }
    }

    pub(super) async fn run_turn(
        &mut self,
        cancellation: &CodexHistoryCancellation,
        request: TurnRequest,
        sink: &HistoryEventSink,
        receiver: &mut Receiver<ObserverCommand>,
        mut initial_controls: Vec<ThreadControlRequest>,
    ) -> OperationDrive<bool> {
        let TurnRequest {
            thread_id,
            prompt,
            options,
        } = request;
        let inference_override_requested = options.inference.is_some();
        if !self.writer_matches(&thread_id) {
            let message = "Writing access has not been acquired for this conversation.";
            self.detach();
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

        self.flush_pending_conversation(&thread_id, sink);
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
                cancellation,
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
            Err(_) if cancellation.is_cancelled() => {
                return OperationDrive::Completed {
                    output: false,
                    deferred,
                };
            }
            Err(error) => {
                self.fail_stream(&thread_id, error, sink).await;
                return OperationDrive::Completed {
                    output: false,
                    deferred,
                };
            }
        };
        if inference_override_requested {
            self.emit_writer_model_state(&thread_id, sink);
        }
        let turn_id = match &event {
            ThreadStreamEvent::TurnStarted { turn, .. } => turn.id.clone(),
            _ => unreachable!("beginning a turn always produces its started event"),
        };
        self.accept_event(&thread_id, event, sink);

        let mut deferred = deferred.unwrap_or_default();
        initial_controls.append(&mut deferred.controls);
        self.apply_controls(initial_controls, sink).await;

        let mut emit_interval = tokio::time::interval(LIVE_DELTA_EMIT_INTERVAL);
        emit_interval.set_missed_tick_behavior(MissedTickBehavior::Skip);
        emit_interval.tick().await;
        let cancellation = cancellation.clone();
        loop {
            let wake = tokio::select! {
                biased;
                _ = cancellation.cancelled() => return OperationDrive::Stop,
                command = receiver.recv() => ObserverWake::Command(command),
                update = self.next_update() => ObserverWake::Writer(Box::new(update)),
                _ = emit_interval.tick() => ObserverWake::Timer,
            };
            match wake {
                ObserverWake::Cancelled => return OperationDrive::Stop,
                ObserverWake::Command(None) => return OperationDrive::Stop,
                ObserverWake::Command(Some(command)) => match drain_commands(command, receiver) {
                    DrainedCommands::Stop => return OperationDrive::Stop,
                    DrainedCommands::Update(mut update) => {
                        self.apply_controls(std::mem::take(&mut update.controls), sink)
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
                        self.accept_event(&event_thread_id, event, sink);
                        if completed {
                            self.flush_pending_conversation(&thread_id, sink);
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
                        if cancellation.is_cancelled() {
                            return OperationDrive::Stop;
                        }
                        self.fail_stream(&event_thread_id, error, sink).await;
                        return OperationDrive::Completed {
                            output: false,
                            deferred: (!deferred.is_empty()).then_some(deferred),
                        };
                    }
                },
                ObserverWake::Timer => {
                    self.flush_pending_conversation(&thread_id, sink);
                }
            }
        }
    }

    pub(super) async fn shutdown(mut self) {
        self.shutdown_writer().await;
    }
}

fn emit_live_conversation(live: &LiveThreadProjection, thread_id: &str, sink: &HistoryEventSink) {
    if let Some(thread) = live.conversation().cloned() {
        sink.emit_conversation_updated(thread_id, thread, live.forkable_turn_ids().to_vec());
    }
}
