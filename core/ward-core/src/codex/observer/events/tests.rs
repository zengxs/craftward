// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Mutex;

use ward_codex::{ThreadActiveFlag, ThreadPage};

use super::super::test_support::{CapturedEvent, event_sink, thread};
use crate::codex::live::LiveRuntimeState;
use crate::codex::wire;

#[test]
fn serializes_thread_pages_for_the_callback_duration() {
    let captured = Mutex::new(CapturedEvent::default());
    event_sink(&captured).emit_threads_updated(
        ThreadPage {
            threads: vec![thread().summary],
            next_cursor: Some("next".to_owned()),
        },
        false,
    );

    let captured = captured.lock().unwrap();
    assert_eq!(captured.events.len(), 1);
    let event = &captured.events[0];
    assert_eq!(event.kind, wire::HistoryEventKind::ThreadsUpdated as i32);
    assert_eq!(event.thread_id, None);
    assert_eq!(event.archived, Some(false));
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
    let Some(wire::history_event::Body::Conversation(conversation)) = event.body.as_ref() else {
        panic!("the event must contain a conversation");
    };
    assert_eq!(conversation.title, "Example");
    assert_eq!(conversation.timeline.len(), 1);
    assert_eq!(conversation.timeline[0].turn_id, "turn-1");
    let Some(wire::timeline_item::Body::Message(message)) = conversation.timeline[0].body.as_ref()
    else {
        panic!("the timeline item must contain a message");
    };
    assert_eq!(message.message_id, "agent-1");
}

#[test]
fn serializes_thread_start_outcomes() {
    let captured = Mutex::new(CapturedEvent::default());
    let sink = event_sink(&captured);

    sink.emit_thread_started("thread-1", thread());
    sink.emit_thread_start_error("start failed");

    let captured = captured.lock().unwrap();
    assert_eq!(captured.events.len(), 2);
    let started = &captured.events[0];
    assert_eq!(started.kind, wire::HistoryEventKind::ThreadStarted as i32);
    assert_eq!(started.thread_id.as_deref(), Some("thread-1"));
    let Some(wire::history_event::Body::Conversation(conversation)) = started.body.as_ref() else {
        panic!("the started event must contain its initial conversation");
    };
    assert_eq!(conversation.title, "Example");

    let failed = &captured.events[1];
    assert_eq!(failed.kind, wire::HistoryEventKind::ThreadStartError as i32);
    assert_eq!(failed.thread_id, None);
    assert_eq!(
        failed.body,
        Some(wire::history_event::Body::ErrorMessage(
            "start failed".to_owned()
        ))
    );
}

#[test]
fn serializes_thread_lifecycle_errors_for_the_selected_thread() {
    let captured = Mutex::new(CapturedEvent::default());
    event_sink(&captured).emit_thread_lifecycle_error("thread-1", "archive failed");

    let captured = captured.lock().unwrap();
    let event = captured.events.first().unwrap();
    assert_eq!(
        event.kind,
        wire::HistoryEventKind::ThreadLifecycleError as i32
    );
    assert_eq!(event.thread_id.as_deref(), Some("thread-1"));
    assert_eq!(
        event.body,
        Some(wire::history_event::Body::ErrorMessage(
            "archive failed".to_owned()
        ))
    );
}

#[test]
fn serializes_turn_steer_outcomes_for_the_selected_thread() {
    let captured = Mutex::new(CapturedEvent::default());
    let sink = event_sink(&captured);

    sink.emit_turn_steered("thread-1");
    sink.emit_turn_steer_error("thread-1", "turn completed");

    let captured = captured.lock().unwrap();
    assert_eq!(captured.events.len(), 2);
    assert_eq!(
        captured.events[0].kind,
        wire::HistoryEventKind::TurnSteered as i32
    );
    assert_eq!(captured.events[0].thread_id.as_deref(), Some("thread-1"));
    assert_eq!(captured.events[0].body, None);
    assert_eq!(
        captured.events[1].kind,
        wire::HistoryEventKind::TurnSteerError as i32
    );
    assert_eq!(
        captured.events[1].body,
        Some(wire::history_event::Body::ErrorMessage(
            "turn completed".to_owned()
        ))
    );
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
fn emits_targeted_recovery_and_error_states_without_payloads() {
    let captured = Mutex::new(CapturedEvent::default());
    let sink = event_sink(&captured);

    sink.emit_threads_error("disconnected", true);
    {
        let captured = captured.lock().unwrap();
        assert_eq!(captured.events.len(), 1);
        let event = &captured.events[0];
        assert_eq!(event.kind, wire::HistoryEventKind::ThreadsError as i32);
        assert_eq!(event.thread_id, None);
        assert_eq!(event.archived, Some(true));
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
