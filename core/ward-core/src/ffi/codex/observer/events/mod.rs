// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::c_void;

use prost::Message as _;
use ward_codex::{ModelCatalog, PendingInteraction, Thread, ThreadActiveFlag, ThreadPage};

use super::super::live::LiveRuntimeState;
use super::super::{WardBuffer, wire};
use super::WardCodexHistoryEventFn;

#[cfg(test)]
mod tests;

pub(super) struct HistoryEventSink {
    callback: WardCodexHistoryEventFn,
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
    pub(super) const fn new(callback: WardCodexHistoryEventFn, context: *mut c_void) -> Self {
        Self { callback, context }
    }

    pub(super) fn emit_threads_updated(&self, page: ThreadPage, archived: bool) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ThreadsUpdated as i32,
            thread_id: None,
            archived: Some(archived),
            body: Some(wire::history_event::Body::ThreadPage(page.into())),
        });
    }

    pub(super) fn emit_model_catalog_updated(&self, catalog: ModelCatalog) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ModelCatalogUpdated as i32,
            thread_id: None,
            archived: None,
            body: Some(wire::history_event::Body::ModelCatalog(catalog.into())),
        });
    }

    pub(super) fn emit_model_catalog_error(&self, message: &str) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ModelCatalogError as i32,
            thread_id: None,
            archived: None,
            body: Some(wire::history_event::Body::ErrorMessage(message.to_owned())),
        });
    }

    pub(super) fn emit_conversation_updated(
        &self,
        thread_id: &str,
        thread: Thread,
        forkable_turn_ids: Vec<String>,
    ) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ConversationUpdated as i32,
            thread_id: Some(thread_id.to_owned()),
            archived: None,
            body: Some(wire::history_event::Body::Conversation(
                wire::Conversation::from_thread(thread, forkable_turn_ids),
            )),
        });
    }

    pub(super) fn emit_thread_started(
        &self,
        thread_id: &str,
        thread: Thread,
        forkable_turn_ids: Vec<String>,
    ) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ThreadStarted as i32,
            thread_id: Some(thread_id.to_owned()),
            archived: None,
            body: Some(wire::history_event::Body::Conversation(
                wire::Conversation::from_thread(thread, forkable_turn_ids),
            )),
        });
    }

    pub(super) fn emit_thread_start_error(&self, message: &str) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ThreadStartError as i32,
            thread_id: None,
            archived: None,
            body: Some(wire::history_event::Body::ErrorMessage(message.to_owned())),
        });
    }

    pub(super) fn emit_thread_forked(
        &self,
        thread_id: &str,
        thread: Thread,
        forkable_turn_ids: Vec<String>,
    ) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ThreadForked as i32,
            thread_id: Some(thread_id.to_owned()),
            archived: None,
            body: Some(wire::history_event::Body::Conversation(
                wire::Conversation::from_thread(thread, forkable_turn_ids),
            )),
        });
    }

    pub(super) fn emit_thread_fork_error(&self, thread_id: &str, message: &str) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ThreadForkError as i32,
            thread_id: Some(thread_id.to_owned()),
            archived: None,
            body: Some(wire::history_event::Body::ErrorMessage(message.to_owned())),
        });
    }

    pub(super) fn emit_thread_lifecycle_error(&self, thread_id: &str, message: &str) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ThreadLifecycleError as i32,
            thread_id: Some(thread_id.to_owned()),
            archived: None,
            body: Some(wire::history_event::Body::ErrorMessage(message.to_owned())),
        });
    }

    pub(super) fn emit_threads_recovered(&self, archived: bool) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ThreadsRecovered as i32,
            thread_id: None,
            archived: Some(archived),
            body: None,
        });
    }

    pub(super) fn emit_conversation_recovered(&self, thread_id: &str) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ConversationRecovered as i32,
            thread_id: Some(thread_id.to_owned()),
            archived: None,
            body: None,
        });
    }

    pub(super) fn emit_threads_error(&self, message: &str, archived: bool) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ThreadsError as i32,
            thread_id: None,
            archived: Some(archived),
            body: Some(wire::history_event::Body::ErrorMessage(message.to_owned())),
        });
    }

    pub(super) fn emit_conversation_error(&self, thread_id: &str, message: &str) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ConversationError as i32,
            thread_id: Some(thread_id.to_owned()),
            archived: None,
            body: Some(wire::history_event::Body::ErrorMessage(message.to_owned())),
        });
    }

    pub(super) fn emit_turn_started(&self, thread_id: &str) {
        self.emit_turn_event(wire::HistoryEventKind::TurnStarted, thread_id, None);
    }

    pub(super) fn emit_turn_completed(&self, thread_id: &str) {
        self.emit_turn_event(wire::HistoryEventKind::TurnCompleted, thread_id, None);
    }

    pub(super) fn emit_turn_error(&self, thread_id: &str, message: &str) {
        self.emit_turn_event(wire::HistoryEventKind::TurnError, thread_id, Some(message));
    }

    pub(super) fn emit_turn_notice(&self, thread_id: &str, message: &str) {
        self.emit_turn_event(wire::HistoryEventKind::TurnNotice, thread_id, Some(message));
    }

    pub(super) fn emit_turn_steered(&self, thread_id: &str) {
        self.emit_turn_event(wire::HistoryEventKind::TurnSteered, thread_id, None);
    }

    pub(super) fn emit_turn_steer_error(&self, thread_id: &str, message: &str) {
        self.emit_turn_event(
            wire::HistoryEventKind::TurnSteerError,
            thread_id,
            Some(message),
        );
    }

    pub(super) fn emit_thread_write_state(
        &self,
        thread_id: &str,
        status: wire::ThreadWriteStatus,
        message: Option<&str>,
    ) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ThreadWriteStateChanged as i32,
            thread_id: Some(thread_id.to_owned()),
            archived: None,
            body: Some(wire::history_event::Body::ThreadWriteState(
                wire::ThreadWriteState {
                    status: status as i32,
                    message: message.map(str::to_owned),
                },
            )),
        });
    }

    pub(super) fn emit_thread_model_changed(
        &self,
        thread_id: &str,
        model: &str,
        reasoning_effort: Option<&str>,
    ) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::ThreadModelChanged as i32,
            thread_id: Some(thread_id.to_owned()),
            archived: None,
            body: Some(wire::history_event::Body::ThreadModelState(
                wire::ThreadModelState {
                    model: model.to_owned(),
                    reasoning_effort: reasoning_effort.map(str::to_owned),
                },
            )),
        });
    }

    pub(super) fn emit_thread_runtime_state(&self, thread_id: &str, state: LiveRuntimeState) {
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
            archived: None,
            body: Some(wire::history_event::Body::ThreadRuntimeState(
                wire::ThreadRuntimeState {
                    status: status as i32,
                    turn_id,
                    active_flags,
                },
            )),
        });
    }

    pub(super) fn emit_pending_interactions(
        &self,
        thread_id: &str,
        interactions: impl IntoIterator<Item = PendingInteraction>,
    ) {
        self.emit(wire::HistoryEvent {
            kind: wire::HistoryEventKind::PendingInteractionsUpdated as i32,
            thread_id: Some(thread_id.to_owned()),
            archived: None,
            body: Some(wire::history_event::Body::PendingInteractions(
                wire::PendingInteractionPage {
                    interactions: interactions.into_iter().map(Into::into).collect(),
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
            archived: None,
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
