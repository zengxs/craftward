// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use ward_codex::{
    Activity, ActivityKind, ActivityStatus, CodexError, CodexHistoryCancellation,
    CodexThreadWriter, InferenceOverride, InteractionDecision, InteractionId, InteractionResponse,
    InteractionResponseBody, PendingInteraction, PendingInteractionKind, ReasoningEffort,
    ThreadActiveFlag, ThreadItem, ThreadPoll, ThreadStartOptions, ThreadStreamEvent,
    ThreadSubscription, Turn, TurnOptions, TurnStatus,
};
use ward_codex_test_support::{FakeCodexAppServer, FakeCodexAppServerOptions, FakeTurnScenario};

use super::super::test_support::{CapturedEvent, event_sink, thread};
use super::actor::run_observer;
use super::operation::{OperationDrive, drive_operation};
use super::polling::{InitialConversationReads, PollEffect, PollHealth, PollSample};
use super::state::{
    ObserverState, ThreadStartEffect, WriteAccessEffect, classify_thread_start_result,
    classify_write_access_result,
};
use super::writer::WriterRuntime;
use crate::codex::live::LiveRuntimeState;
use crate::codex::observer::COMMAND_QUEUE_CAPACITY;
use crate::codex::observer::ObserverOperationGate;
use crate::codex::observer::commands::{
    CommandUpdate, ObserverCommand, ThreadControlRequest, ThreadForkRequest, ThreadLifecycleAction,
    ThreadLifecycleRequest, ThreadListScope, ThreadRenameRequest, ThreadStartRequest, TurnRequest,
    TurnSteerRequest,
};
use crate::codex::observer::events::HistoryEventSink;
use crate::codex::wire;

mod actor;
mod history;
mod operation;
mod polling;
mod writer;

fn assert_thread_page(captured: &Mutex<CapturedEvent>, archived: bool, thread_ids: &[&str]) {
    let captured = captured.lock().unwrap();
    let page = captured
        .events
        .iter()
        .rev()
        .find_map(|event| match event.body.as_ref() {
            Some(wire::history_event::Body::ThreadPage(page)) => Some(page),
            _ => None,
        })
        .expect("the observer should emit an authoritative thread page");
    assert_eq!(
        captured
            .events
            .iter()
            .rev()
            .find(|event| {
                matches!(
                    event.body.as_ref(),
                    Some(wire::history_event::Body::ThreadPage(_))
                )
            })
            .and_then(|event| event.archived),
        Some(archived)
    );
    assert_eq!(
        page.threads
            .iter()
            .map(|thread| thread.thread_id.as_str())
            .collect::<Vec<_>>(),
        thread_ids
    );
}

fn latest_conversation(captured: &CapturedEvent) -> &wire::Conversation {
    captured
        .events
        .iter()
        .rev()
        .find_map(|event| match event.body.as_ref() {
            Some(wire::history_event::Body::Conversation(conversation)) => Some(conversation),
            _ => None,
        })
        .expect("the conversation should be emitted")
}
