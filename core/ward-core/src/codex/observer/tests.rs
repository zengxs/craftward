// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use prost::Message as _;
use tokio::sync::{mpsc, oneshot};
use ward_codex::{
    Activity, ActivityKind, ActivityStatus, AgentMessagePhase, CodexError,
    CodexHistoryCancellation, Thread, ThreadActiveFlag, ThreadItem, ThreadPage, ThreadStreamEvent,
    ThreadSubscription, ThreadSummary, Turn, TurnStatus,
};

use super::super::live::{LiveRuntimeState, LiveThreadProjection};
use super::super::{WardBuffer, wire};
use super::COMMAND_QUEUE_CAPACITY;
use super::commands::{
    CommandUpdate, DrainedCommands, ObserverCommand, TurnRequest, WriteAccessRequest,
    drain_commands, merge_command,
};
use super::events::HistoryEventSink;
use super::worker::{
    ObserverState, OperationDrive, PollEffect, PollHealth, PollSample, WriteAccessEffect,
    accept_live_event, classify_write_access_result, drive_operation,
    flush_pending_live_conversation,
};

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
    HistoryEventSink::new(
        capture_event,
        std::ptr::from_ref(captured).cast_mut().cast(),
    )
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
    let mut state = ObserverState::new(PathBuf::from("/codex"), CodexHistoryCancellation::new());
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
    let Some(wire::history_event::Body::Conversation(conversation)) = event.body.as_ref() else {
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
    let Some(wire::history_event::Body::Conversation(conversation)) = event.body.as_ref() else {
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
