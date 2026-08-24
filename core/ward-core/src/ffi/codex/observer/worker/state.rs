// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;

use tokio::sync::mpsc::Receiver;
use ward_codex::{
    CodexAppServerSource, CodexError, CodexHistoryCancellation, CodexHistorySession,
    CodexThreadWriter, Thread, ThreadListOptions, ThreadPage, ThreadPagePoll, ThreadPoll,
    ThreadStartOptions, ThreadStreamEvent, ThreadSubscription, ThreadSummary,
};

use super::super::super::live::LiveRuntimeState;
use super::super::super::wire;
use super::super::commands::{
    ObserverCommand, ThreadControlRequest, ThreadForkRequest, ThreadLifecycleAction,
    ThreadLifecycleRequest, ThreadListScope, ThreadRenameRequest, ThreadStartRequest, TurnRequest,
};
use super::super::events::HistoryEventSink;
use super::operation::OperationDrive;
use super::polling::{
    ConversationPollOutcome, InitialConversationReads, PollEffect, PollHealth, PollSample,
    ThreadPagePollOutcome,
};
use super::writer::{WriterRuntime, WriterStreamUpdate};

const THREAD_LIST_PAGE_LIMIT: u32 = 100;

pub(super) struct ObserverState {
    source: CodexAppServerSource,
    cancellation: CodexHistoryCancellation,
    session: Option<CodexHistorySession>,
    thread_list_scope: ThreadListScope,
    model_catalog_health: PollHealth,
    thread_page_health: PollHealth,
    conversation_health: PollHealth,
    initial_conversation_reads: InitialConversationReads,
    polled_conversation_is_active: bool,
    thread_summaries: HashMap<String, ThreadSummary>,
    writer: WriterRuntime,
}

#[cfg(test)]
pub(super) struct ObserverStateSnapshot {
    pub(super) writer_thread_id: Option<String>,
    pub(super) active_model: Option<String>,
    pub(super) active_reasoning_effort: Option<String>,
    pub(super) runtime: LiveRuntimeState,
    pub(super) conversation_thread_id: Option<String>,
}

impl ObserverState {
    pub(super) fn new(
        source: impl Into<CodexAppServerSource>,
        cancellation: CodexHistoryCancellation,
    ) -> Self {
        Self {
            source: source.into(),
            cancellation,
            session: None,
            writer: WriterRuntime::default(),
            thread_list_scope: ThreadListScope::Active,
            model_catalog_health: PollHealth::default(),
            thread_page_health: PollHealth::default(),
            conversation_health: PollHealth::default(),
            initial_conversation_reads: InitialConversationReads::default(),
            polled_conversation_is_active: false,
            thread_summaries: HashMap::new(),
        }
    }

    pub(super) fn has_pending_conversation_emit(&self) -> bool {
        self.writer.has_pending_conversation_emit()
    }

    #[cfg(test)]
    pub(super) fn test_snapshot(&self) -> ObserverStateSnapshot {
        ObserverStateSnapshot {
            writer_thread_id: self.writer.writer_thread_id().map(str::to_owned),
            active_model: self.writer.active_model().map(str::to_owned),
            active_reasoning_effort: self.writer.active_reasoning_effort().map(str::to_owned),
            runtime: self.writer.runtime(),
            conversation_thread_id: self
                .writer
                .conversation()
                .map(|thread| thread.summary.id.clone()),
        }
    }

    #[cfg(test)]
    pub(super) fn replace_source(&mut self, source: impl Into<CodexAppServerSource>) {
        self.source = source.into();
    }

    pub(super) async fn select_thread(&mut self) {
        self.writer.reset().await;
        self.conversation_health.reset();
        self.polled_conversation_is_active = false;
        if let Some(session) = self.session.as_mut() {
            session.reset_thread_baseline();
        }
    }

    pub(super) fn emit_writer_model_state(&self, thread_id: &str, sink: &HistoryEventSink) {
        self.writer.emit_writer_model_state(thread_id, sink);
    }

    pub(super) fn set_thread_list_scope(&mut self, scope: ThreadListScope) -> bool {
        if self.thread_list_scope == scope {
            return false;
        }
        self.thread_list_scope = scope;
        self.thread_page_health.reset();
        self.thread_summaries.clear();
        if let Some(session) = self.session.as_mut() {
            session.reset_thread_page_baseline();
        }
        true
    }

    pub(super) async fn start_thread(
        &mut self,
        request: ThreadStartRequest,
        sink: &HistoryEventSink,
    ) -> Option<String> {
        if self.thread_list_scope.is_archived() {
            sink.emit_thread_start_error("Archived Codex history is read-only.");
            return None;
        }
        let result = CodexThreadWriter::start_on(
            &self.source,
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
                self.writer.attach(writer, subscription);
                self.initial_conversation_reads.mark_started(&thread_id);
                self.refresh();

                let thread = self
                    .writer
                    .conversation()
                    .cloned()
                    .expect("a started writer always includes its thread snapshot");
                sink.emit_thread_started(
                    &thread_id,
                    thread,
                    self.writer.forkable_turn_ids().to_vec(),
                );
                self.emit_writer_model_state(&thread_id, sink);
                sink.emit_pending_interactions(&thread_id, std::iter::empty());
                sink.emit_thread_runtime_state(&thread_id, self.writer.runtime());
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

    pub(super) async fn acquire_write(&mut self, thread_id: &str, sink: &HistoryEventSink) -> bool {
        if self.thread_list_scope.is_archived() {
            sink.emit_thread_write_state(
                thread_id,
                wire::ThreadWriteStatus::Unavailable,
                Some("Archived Codex conversations are read-only."),
            );
            return false;
        }
        if self.writer.writer_matches(thread_id) {
            self.emit_writer_model_state(thread_id, sink);
            sink.emit_thread_runtime_state(thread_id, self.writer.runtime());
            sink.emit_thread_write_state(thread_id, wire::ThreadWriteStatus::Writable, None);
            return true;
        }

        self.writer.shutdown_writer().await;
        sink.emit_thread_write_state(thread_id, wire::ThreadWriteStatus::Checking, None);
        let result =
            CodexThreadWriter::acquire_on(&self.source, self.cancellation.clone(), thread_id).await;
        match classify_write_access_result(result, self.cancellation.is_cancelled()) {
            WriteAccessEffect::Acquired(acquired) => {
                let (writer, subscription) = *acquired;
                self.writer.attach(writer, subscription);
                if let Some(thread) = self.writer.conversation().cloned() {
                    sink.emit_conversation_updated(
                        thread_id,
                        thread,
                        self.writer.forkable_turn_ids().to_vec(),
                    );
                }
                sink.emit_pending_interactions(thread_id, std::iter::empty());
                sink.emit_thread_runtime_state(thread_id, self.writer.runtime());
                self.emit_writer_model_state(thread_id, sink);
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

    pub(super) async fn release_write(&mut self, thread_id: &str, sink: &HistoryEventSink) {
        if self.writer.writer_matches(thread_id) {
            self.writer.shutdown_writer().await;
        }
        self.writer.detach();
        sink.emit_pending_interactions(thread_id, std::iter::empty());
        sink.emit_thread_runtime_state(thread_id, self.writer.runtime());
        sink.emit_thread_write_state(thread_id, wire::ThreadWriteStatus::Idle, None);
    }

    pub(super) fn refresh_thread_page(&mut self) {
        self.thread_page_health.reset();
        if let Some(session) = self.session.as_mut() {
            session.reset_thread_page_baseline();
        }
    }

    pub(super) fn refresh_model_catalog(&mut self) {
        self.model_catalog_health.reset();
    }

    pub(super) fn refresh(&mut self) {
        self.refresh_thread_page();
        self.conversation_health.reset();
        if let Some(session) = self.session.as_mut() {
            session.reset_thread_baseline();
        }
    }

    pub(super) async fn load_model_catalog(&mut self, sink: &HistoryEventSink) -> bool {
        let result = match self.ensure_session().await {
            Ok(()) => {
                self.session
                    .as_mut()
                    .expect("the history session was initialized above")
                    .load_model_catalog()
                    .await
            }
            Err(error) => Err(error),
        }
        .map(PollSample::Updated);
        let effect = self
            .model_catalog_health
            .observe(result, self.cancellation.is_cancelled());
        let succeeded = effect.is_successful();
        match effect {
            PollEffect::Updated(catalog) => sink.emit_model_catalog_updated(catalog),
            PollEffect::Error(message) => sink.emit_model_catalog_error(&message),
            PollEffect::Recovered
            | PollEffect::Unchanged
            | PollEffect::RepeatedError
            | PollEffect::Cancelled => {}
        }
        succeeded
    }

    pub(super) async fn rename_thread(
        &mut self,
        request: ThreadRenameRequest,
        sink: &HistoryEventSink,
    ) -> bool {
        let ThreadRenameRequest { thread_id, name } = request;
        if self.thread_list_scope.is_archived() {
            sink.emit_conversation_error(&thread_id, "Archived Codex conversations are read-only.");
            return false;
        }
        let result = match self.ensure_session().await {
            Ok(()) => {
                self.session
                    .as_mut()
                    .expect("the history session was initialized above")
                    .rename_thread(&thread_id, &name)
                    .await
            }
            Err(error) => Err(error),
        };
        match result {
            Ok(()) => {
                self.refresh();
                true
            }
            Err(_) if self.cancellation.is_cancelled() => false,
            Err(error) => {
                sink.emit_conversation_error(&thread_id, &error.to_string());
                false
            }
        }
    }

    pub(super) async fn fork_thread(
        &mut self,
        request: ThreadForkRequest,
        sink: &HistoryEventSink,
    ) -> Option<String> {
        let ThreadForkRequest {
            thread_id,
            last_turn_id,
        } = request;
        self.refresh_thread_page();
        if self.thread_list_scope.is_archived() {
            sink.emit_thread_fork_error(&thread_id, "Archived Codex history is read-only.");
            return None;
        }
        let loaded = self
            .writer
            .conversation()
            .is_some_and(|thread| thread.summary.id == thread_id);
        if !loaded {
            sink.emit_thread_fork_error(
                &thread_id,
                "The conversation must be loaded before it can be forked.",
            );
            return None;
        }

        let source_writer_was_present = self.writer.writer_matches(&thread_id);
        if !source_writer_was_present {
            self.writer.shutdown_writer().await;
            let result =
                CodexThreadWriter::acquire_on(&self.source, self.cancellation.clone(), &thread_id)
                    .await;
            match classify_write_access_result(result, self.cancellation.is_cancelled()) {
                WriteAccessEffect::Acquired(acquired) => {
                    let (writer, subscription) = *acquired;
                    self.writer.attach(writer, subscription);
                }
                WriteAccessEffect::Busy => {
                    self.writer.detach();
                    sink.emit_thread_fork_error(
                        &thread_id,
                        "The conversation is open in another Codex client.",
                    );
                    return None;
                }
                WriteAccessEffect::Unavailable(message) => {
                    self.writer.detach();
                    sink.emit_thread_fork_error(&thread_id, &message);
                    return None;
                }
                WriteAccessEffect::Cancelled => return None,
            }
        }
        if self.writer.runtime() != LiveRuntimeState::Idle {
            if !source_writer_was_present {
                self.writer.shutdown_writer().await;
                self.writer.detach();
            }
            sink.emit_thread_fork_error(
                &thread_id,
                "The conversation must be idle before it can be forked.",
            );
            return None;
        }

        let result = CodexThreadWriter::fork_on(
            &self.source,
            self.cancellation.clone(),
            &thread_id,
            Some(&last_turn_id),
        )
        .await;
        match result {
            Ok((writer, subscription)) => {
                let forked_thread_id = writer.thread_id().to_owned();
                self.select_thread().await;
                self.writer.attach(writer, subscription);
                self.initial_conversation_reads
                    .mark_started(&forked_thread_id);
                self.refresh();
                let thread = self
                    .writer
                    .conversation()
                    .cloned()
                    .expect("a forked writer always includes its thread snapshot");
                sink.emit_thread_forked(
                    &forked_thread_id,
                    thread,
                    self.writer.forkable_turn_ids().to_vec(),
                );
                self.emit_writer_model_state(&forked_thread_id, sink);
                sink.emit_pending_interactions(&forked_thread_id, std::iter::empty());
                sink.emit_thread_runtime_state(&forked_thread_id, self.writer.runtime());
                sink.emit_thread_write_state(
                    &forked_thread_id,
                    wire::ThreadWriteStatus::Writable,
                    None,
                );
                Some(forked_thread_id)
            }
            Err(_) if self.cancellation.is_cancelled() => None,
            Err(error) => {
                if !source_writer_was_present {
                    self.writer.shutdown_writer().await;
                    self.writer.detach();
                }
                // A lost response can hide a fork that the app-server already
                // created. Refresh the list, but never retry the mutation.
                self.refresh();
                sink.emit_thread_fork_error(&thread_id, &error.to_string());
                None
            }
        }
    }

    pub(super) async fn change_thread_lifecycle(
        &mut self,
        request: ThreadLifecycleRequest,
        sink: &HistoryEventSink,
    ) -> bool {
        let ThreadLifecycleRequest { thread_id, action } = request;
        let valid_scope = matches!(
            (self.thread_list_scope, action),
            (ThreadListScope::Active, ThreadLifecycleAction::Archive)
                | (ThreadListScope::Archived, ThreadLifecycleAction::Restore)
        );
        if !valid_scope {
            sink.emit_thread_lifecycle_error(
                &thread_id,
                "The conversation no longer belongs to the displayed history.",
            );
            return false;
        }

        if self.writer.writer_matches(&thread_id) {
            self.release_write(&thread_id, sink).await;
        }
        let result = match self.ensure_session().await {
            Ok(()) => {
                let session = self
                    .session
                    .as_mut()
                    .expect("the history session was initialized above");
                match action {
                    ThreadLifecycleAction::Archive => session.archive_thread(&thread_id).await,
                    ThreadLifecycleAction::Restore => {
                        session.unarchive_thread(&thread_id).await.map(drop)
                    }
                }
            }
            Err(error) => Err(error),
        };
        match result {
            Ok(()) => {
                self.refresh();
                true
            }
            Err(_) if self.cancellation.is_cancelled() => false,
            Err(error) => {
                sink.emit_thread_lifecycle_error(&thread_id, &error.to_string());
                false
            }
        }
    }

    pub(super) async fn poll_threads(
        &mut self,
        watched_thread: Option<&str>,
        sink: &HistoryEventSink,
    ) -> ThreadPagePollOutcome {
        let result = self.poll_all_threads().await.map(|poll| match poll {
            ThreadPagePoll::Baseline(page) | ThreadPagePoll::Changed(page) => {
                PollSample::Updated(page)
            }
            _ => PollSample::Unchanged,
        });
        let effect = self
            .thread_page_health
            .observe(result, self.cancellation.is_cancelled());
        match effect {
            PollEffect::Updated(page) => {
                let watched_thread_changed = self.update_thread_summaries(&page, watched_thread);
                sink.emit_threads_updated(page, self.thread_list_scope.is_archived());
                ThreadPagePollOutcome::Updated {
                    watched_thread_changed,
                }
            }
            PollEffect::Recovered => {
                sink.emit_threads_recovered(self.thread_list_scope.is_archived());
                ThreadPagePollOutcome::Stable
            }
            PollEffect::Error(message) => {
                sink.emit_threads_error(&message, self.thread_list_scope.is_archived());
                ThreadPagePollOutcome::Failed
            }
            PollEffect::Unchanged => ThreadPagePollOutcome::Stable,
            PollEffect::RepeatedError | PollEffect::Cancelled => ThreadPagePollOutcome::Failed,
        }
    }

    pub(super) async fn poll_conversation(
        &mut self,
        thread_id: &str,
        sink: &HistoryEventSink,
    ) -> ConversationPollOutcome {
        let result = if self.polled_conversation_is_active
            && !self.writer.needs_persisted_reconciliation()
        {
            self.poll_active_thread(thread_id).await
        } else {
            self.poll_thread(thread_id).await
        };
        let result = self.initial_conversation_reads.classify(thread_id, result);
        let effect = self
            .conversation_health
            .observe(result, self.cancellation.is_cancelled());
        let succeeded = effect.is_successful();
        match effect {
            PollEffect::Updated(thread) => {
                self.polled_conversation_is_active = thread
                    .turns
                    .last()
                    .is_some_and(|turn| turn.status.is_in_progress());
                self.accept_polled_conversation(thread_id, thread, sink);
            }
            PollEffect::Recovered => sink.emit_conversation_recovered(thread_id),
            PollEffect::Error(message) => sink.emit_conversation_error(thread_id, &message),
            PollEffect::Unchanged | PollEffect::RepeatedError | PollEffect::Cancelled => {}
        }
        if !succeeded {
            ConversationPollOutcome::Failed
        } else if self.initial_conversation_reads.is_pending(thread_id)
            || self.polled_conversation_is_active
            || self.writer.needs_persisted_reconciliation()
        {
            ConversationPollOutcome::FollowUp
        } else {
            ConversationPollOutcome::Settled
        }
    }

    fn update_thread_summaries(&mut self, page: &ThreadPage, watched_thread: Option<&str>) -> bool {
        let watched_thread_changed = watched_thread.is_some_and(|thread_id| {
            self.thread_summaries.get(thread_id)
                != page.threads.iter().find(|thread| thread.id == thread_id)
        });
        self.thread_summaries = page
            .threads
            .iter()
            .cloned()
            .map(|thread| (thread.id.clone(), thread))
            .collect();
        watched_thread_changed
    }

    pub(super) fn accept_polled_conversation(
        &mut self,
        thread_id: &str,
        thread: Thread,
        sink: &HistoryEventSink,
    ) {
        self.writer
            .accept_polled_conversation(thread_id, thread, sink);
    }

    pub(super) async fn next_writer_update(&mut self) -> WriterStreamUpdate {
        self.writer.next_update().await
    }

    pub(super) fn accept_writer_event(
        &mut self,
        thread_id: &str,
        event: ThreadStreamEvent,
        sink: &HistoryEventSink,
    ) {
        self.writer.accept_event(thread_id, event, sink);
    }

    pub(super) fn flush_pending_live_conversation(
        &mut self,
        thread_id: &str,
        sink: &HistoryEventSink,
    ) {
        self.writer.flush_pending_conversation(thread_id, sink);
    }

    pub(super) async fn fail_writer_stream(
        &mut self,
        thread_id: &str,
        error: CodexError,
        sink: &HistoryEventSink,
    ) {
        self.writer.fail_stream(thread_id, error, sink).await;
    }

    pub(super) async fn apply_thread_controls(
        &mut self,
        controls: Vec<ThreadControlRequest>,
        sink: &HistoryEventSink,
    ) {
        self.writer.apply_controls(controls, sink).await;
    }

    pub(super) async fn run_turn(
        &mut self,
        request: TurnRequest,
        sink: &HistoryEventSink,
        receiver: &mut Receiver<ObserverCommand>,
        initial_controls: Vec<ThreadControlRequest>,
    ) -> OperationDrive<bool> {
        self.conversation_health.reset();
        self.writer
            .run_turn(
                &self.cancellation,
                request,
                sink,
                receiver,
                initial_controls,
            )
            .await
    }

    pub(super) async fn poll_all_threads(&mut self) -> Result<ThreadPagePoll, CodexError> {
        self.ensure_session().await?;
        self.session
            .as_mut()
            .expect("the history session was initialized above")
            .poll_all_threads(&ThreadListOptions {
                limit: Some(THREAD_LIST_PAGE_LIMIT),
                archived: Some(self.thread_list_scope.is_archived()),
                ..ThreadListOptions::default()
            })
            .await
    }

    pub(super) async fn poll_thread(&mut self, thread_id: &str) -> Result<ThreadPoll, CodexError> {
        self.ensure_session().await?;
        self.session
            .as_mut()
            .expect("the history session was initialized above")
            .poll_thread(thread_id)
            .await
    }

    async fn poll_active_thread(&mut self, thread_id: &str) -> Result<ThreadPoll, CodexError> {
        self.ensure_session().await?;
        self.session
            .as_mut()
            .expect("the history session was initialized above")
            .poll_active_thread(thread_id)
            .await
    }

    pub(super) async fn ensure_session(&mut self) -> Result<(), CodexError> {
        if self.session.is_none() {
            self.session = Some(
                CodexHistorySession::connect_with_cancellation(
                    self.source.clone(),
                    self.cancellation.clone(),
                )
                .await?,
            );
        }
        Ok(())
    }

    pub(super) async fn shutdown(mut self) {
        self.writer.shutdown().await;
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

pub(super) enum ThreadStartEffect {
    Started(Box<(CodexThreadWriter, ThreadSubscription)>),
    Failed(String),
    Cancelled,
}

pub(super) fn classify_thread_start_result(
    result: Result<(CodexThreadWriter, ThreadSubscription), CodexError>,
    cancelled: bool,
) -> ThreadStartEffect {
    match result {
        Ok(started) => ThreadStartEffect::Started(Box::new(started)),
        Err(_) if cancelled => ThreadStartEffect::Cancelled,
        Err(error) => ThreadStartEffect::Failed(error.to_string()),
    }
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
