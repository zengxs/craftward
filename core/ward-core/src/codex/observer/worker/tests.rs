// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use ward_codex::{
    Activity, ActivityKind, ActivityStatus, CodexError, CodexHistoryCancellation,
    InteractionDecision, InteractionId, PendingInteraction, PendingInteractionKind,
    ThreadActiveFlag, ThreadItem, ThreadStreamEvent, ThreadSubscription, Turn, TurnOptions,
    TurnStatus,
};
use ward_codex_test_support::{FakeCodexAppServer, FakeCodexAppServerOptions};

use super::super::test_support::{CapturedEvent, event_sink, thread};
use super::*;
use crate::codex::observer::COMMAND_QUEUE_CAPACITY;
use crate::codex::observer::commands::{
    CommandUpdate, ObserverCommand, ThreadStartRequest, TurnSteerRequest,
};
use crate::codex::wire;

#[tokio::test]
async fn starts_a_persisted_thread_and_adopts_its_writer() {
    let fake_app_server = FakeCodexAppServer::default();
    let captured = Mutex::new(CapturedEvent::default());
    let sink = event_sink(&captured);
    let mut state = ObserverState::new(fake_app_server.source(), CodexHistoryCancellation::new());

    let started_thread_id = state
        .start_thread(
            ThreadStartRequest {
                working_directory: PathBuf::from("/workspace"),
            },
            &sink,
        )
        .await;

    assert_eq!(started_thread_id.as_deref(), Some("thread-new"));
    assert_eq!(
        state.writer.as_ref().map(CodexThreadWriter::thread_id),
        Some("thread-new")
    );
    assert_eq!(state.live.runtime(), LiveRuntimeState::Idle);

    {
        let captured = captured.lock().unwrap();
        assert_eq!(captured.events.len(), 4);
        assert_eq!(
            captured.events[0].kind,
            wire::HistoryEventKind::ThreadStarted as i32
        );
        assert_eq!(captured.events[0].thread_id.as_deref(), Some("thread-new"));
        let Some(wire::history_event::Body::Conversation(conversation)) =
            captured.events[0].body.as_ref()
        else {
            panic!("the start event must contain the initial conversation");
        };
        assert!(conversation.timeline.is_empty());
        assert_eq!(
            captured.events[1].kind,
            wire::HistoryEventKind::PendingInteractionsUpdated as i32
        );
        assert_eq!(
            captured.events[2].kind,
            wire::HistoryEventKind::ThreadRuntimeStateChanged as i32
        );
        assert_eq!(
            captured.events[3].kind,
            wire::HistoryEventKind::ThreadWriteStateChanged as i32
        );
    }

    state.shutdown().await;
}

#[tokio::test]
async fn keeps_a_new_conversation_singular_as_persisted_history_catches_up() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        initial_thread_read_failures: 1,
        renumber_persisted_first_turn: true,
        ..FakeCodexAppServerOptions::default()
    });
    let captured = Mutex::new(CapturedEvent::default());
    let sink = event_sink(&captured);
    let mut state = ObserverState::new(fake_app_server.source(), CodexHistoryCancellation::new());

    let started_thread_id = state
        .start_thread(
            ThreadStartRequest {
                working_directory: PathBuf::from("/workspace"),
            },
            &sink,
        )
        .await
        .expect("the fake app-server should start a thread");
    let (_commands, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let result = state
        .run_turn(
            TurnRequest {
                thread_id: started_thread_id.clone(),
                prompt: "Hello".to_owned(),
                options: TurnOptions::default(),
            },
            &sink,
            &mut receiver,
            vec![],
        )
        .await;
    assert!(matches!(
        result,
        OperationDrive::Completed {
            output: true,
            deferred: None,
        }
    ));

    {
        let captured = captured.lock().unwrap();
        let conversation = latest_conversation(&captured);
        assert_eq!(conversation.timeline.len(), 2);
        assert!(
            conversation
                .timeline
                .iter()
                .all(|item| item.turn_id == "live-turn-1")
        );
    }

    assert!(state.poll_conversation(&started_thread_id, &sink).await);
    assert!(state.poll_conversation(&started_thread_id, &sink).await);

    {
        let captured = captured.lock().unwrap();
        assert!(
            captured
                .events
                .iter()
                .all(|event| { event.kind != wire::HistoryEventKind::ConversationError as i32 })
        );
        let conversation = latest_conversation(&captured);
        assert_eq!(conversation.timeline.len(), 2);
        assert!(
            conversation
                .timeline
                .iter()
                .all(|item| item.turn_id == "persisted-turn-1")
        );
        let messages = conversation
            .timeline
            .iter()
            .map(|item| match item.body.as_ref() {
                Some(wire::timeline_item::Body::Message(message)) => message,
                _ => panic!("the fake turn should contain only messages"),
            })
            .collect::<Vec<_>>();
        assert_eq!(messages[0].message_id, "persisted-user-1");
        assert_eq!(messages[0].text, "Hello");
        assert_eq!(messages[1].message_id, "persisted-agent-1");
        assert_eq!(messages[1].text, "Done.");
    }

    state.shutdown().await;
}

#[tokio::test]
async fn steers_an_active_turn_and_reports_the_outcome() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        renumber_persisted_first_turn: true,
        wait_for_turn_steer: true,
        ..FakeCodexAppServerOptions::default()
    });
    let captured = Mutex::new(CapturedEvent::default());
    let sink = event_sink(&captured);
    let mut state = ObserverState::new(fake_app_server.source(), CodexHistoryCancellation::new());
    let thread_id = state
        .start_thread(
            ThreadStartRequest {
                working_directory: PathBuf::from("/workspace"),
            },
            &sink,
        )
        .await
        .expect("the fake app-server should start a thread");
    let (_commands, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);

    let result = state
        .run_turn(
            TurnRequest {
                thread_id: thread_id.clone(),
                prompt: "Implement the change".to_owned(),
                options: TurnOptions::default(),
            },
            &sink,
            &mut receiver,
            vec![ThreadControlRequest::Steer(TurnSteerRequest {
                thread_id: thread_id.clone(),
                expected_turn_id: "live-turn-1".to_owned(),
                prompt: "Use the existing test seam".to_owned(),
            })],
        )
        .await;

    assert!(matches!(
        result,
        OperationDrive::Completed {
            output: true,
            deferred: None,
        }
    ));
    assert!(state.poll_conversation(&thread_id, &sink).await);
    {
        let captured = captured.lock().unwrap();
        let steered_index = captured
            .events
            .iter()
            .position(|event| {
                event.kind == wire::HistoryEventKind::TurnSteered as i32
                    && event.thread_id.as_deref() == Some("thread-new")
            })
            .expect("the accepted guidance should be confirmed");
        let idle_index = captured
            .events
            .iter()
            .enumerate()
            .skip(steered_index + 1)
            .find_map(|(index, event)| match event.body.as_ref() {
                Some(wire::history_event::Body::ThreadRuntimeState(state))
                    if state.status == wire::ThreadRuntimeStatus::Idle as i32 =>
                {
                    Some(index)
                }
                _ => None,
            })
            .expect("the guided turn should become idle after confirmation");
        let completed_index = captured
            .events
            .iter()
            .position(|event| event.kind == wire::HistoryEventKind::TurnCompleted as i32)
            .expect("the guided turn should report completion");

        assert!(steered_index < idle_index);
        assert!(steered_index < completed_index);
        assert!(
            captured
                .events
                .iter()
                .all(|event| event.kind != wire::HistoryEventKind::TurnSteerError as i32)
        );
        let conversation = latest_conversation(&captured);
        assert_eq!(conversation.timeline.len(), 3);
        assert!(
            conversation
                .timeline
                .iter()
                .all(|item| item.turn_id == "persisted-turn-1")
        );
        let messages = conversation
            .timeline
            .iter()
            .filter_map(|item| match item.body.as_ref() {
                Some(wire::timeline_item::Body::Message(message)) => Some(message),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            messages
                .iter()
                .map(|message| message.text.as_str())
                .collect::<Vec<_>>(),
            [
                "Implement the change",
                "Use the existing test seam",
                "Adjusted."
            ]
        );
        assert_eq!(
            messages
                .iter()
                .map(|message| message.message_id.as_str())
                .collect::<Vec<_>>(),
            [
                "persisted-user-1",
                "persisted-steer-user-1",
                "persisted-agent-1"
            ]
        );
    }

    state.shutdown().await;
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

#[tokio::test]
async fn preserves_the_current_writer_when_a_new_thread_fails_to_start() {
    let fake_app_server = FakeCodexAppServer::default();
    let captured = Mutex::new(CapturedEvent::default());
    let sink = event_sink(&captured);
    let mut state = ObserverState::new(fake_app_server.source(), CodexHistoryCancellation::new());
    let _ = state
        .start_thread(
            ThreadStartRequest {
                working_directory: PathBuf::from("/workspace"),
            },
            &sink,
        )
        .await;
    captured.lock().unwrap().events.clear();
    state.source = PathBuf::from("/craftward-tests/missing-codex").into();

    let started_thread_id = state
        .start_thread(
            ThreadStartRequest {
                working_directory: PathBuf::from("/workspace/two"),
            },
            &sink,
        )
        .await;

    assert_eq!(started_thread_id, None);
    assert_eq!(
        state.writer.as_ref().map(CodexThreadWriter::thread_id),
        Some("thread-new")
    );
    assert_eq!(state.live.runtime(), LiveRuntimeState::Idle);
    {
        let captured = captured.lock().unwrap();
        assert_eq!(captured.events.len(), 1);
        assert_eq!(
            captured.events[0].kind,
            wire::HistoryEventKind::ThreadStartError as i32
        );
    }

    state.shutdown().await;
}

#[test]
fn publishes_pending_interactions_as_a_replaceable_snapshot() {
    let captured = Mutex::new(CapturedEvent::default());
    let sink = event_sink(&captured);
    let mut state = ObserverState::new(PathBuf::from("/codex"), CodexHistoryCancellation::new());
    state.live.attach(ThreadSubscription {
        thread: thread(),
        runtime_status: ward_codex::ThreadRuntimeStatus::Active {
            active_flags: vec![ThreadActiveFlag::WaitingOnApproval],
        },
    });

    state.accept_writer_event(
        "thread-1",
        ThreadStreamEvent::PendingInteractionsUpdated {
            thread_id: "thread-1".to_owned(),
            interactions: vec![PendingInteraction {
                id: InteractionId::new(17).unwrap(),
                thread_id: "thread-1".to_owned(),
                turn_id: Some("turn-2".to_owned()),
                item_id: Some("command-1".to_owned()),
                kind: PendingInteractionKind::CommandApproval,
                command: Some("cargo test".to_owned()),
                working_directory: Some(PathBuf::from("/workspace")),
                reason: None,
                grant_root: None,
                available_decisions: vec![
                    InteractionDecision::Accept,
                    InteractionDecision::Decline,
                ],
                questions: vec![],
                user_input_is_blocking: true,
            }],
        },
        &sink,
    );

    let captured = captured.lock().unwrap();
    let event = captured.events.last().unwrap();
    assert_eq!(
        event.kind,
        wire::HistoryEventKind::PendingInteractionsUpdated as i32
    );
    let Some(wire::history_event::Body::PendingInteractions(page)) = event.body.as_ref() else {
        panic!("the update must contain pending interactions");
    };
    assert_eq!(page.interactions.len(), 1);
    assert_eq!(page.interactions[0].interaction_id, 17);
    assert_eq!(page.interactions[0].command.as_deref(), Some("cargo test"));
}

#[test]
fn confirms_a_turn_start_before_publishing_its_active_runtime_state() {
    let captured = Mutex::new(CapturedEvent::default());
    let sink = event_sink(&captured);
    let mut state = ObserverState::new(PathBuf::from("/codex"), CodexHistoryCancellation::new());
    state.live.attach(ThreadSubscription {
        thread: thread(),
        runtime_status: ward_codex::ThreadRuntimeStatus::Idle,
    });

    state.accept_writer_event(
        "thread-1",
        ThreadStreamEvent::TurnStarted {
            thread_id: "thread-1".to_owned(),
            turn: Turn {
                id: "turn-2".to_owned(),
                status: TurnStatus::InProgress,
                items: vec![],
            },
        },
        &sink,
    );

    let captured = captured.lock().unwrap();
    assert_eq!(
        captured.events[0].kind,
        wire::HistoryEventKind::TurnStarted as i32
    );
    assert_eq!(
        captured.events[1].kind,
        wire::HistoryEventKind::ThreadRuntimeStateChanged as i32
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
    let mut state = ObserverState::new(PathBuf::from("/codex"), CodexHistoryCancellation::new());
    state.live.attach(ThreadSubscription {
        thread: thread(),
        runtime_status: ward_codex::ThreadRuntimeStatus::Idle,
    });
    state.accept_writer_event(
        "thread-1",
        ThreadStreamEvent::TurnStarted {
            thread_id: "thread-1".to_owned(),
            turn: Turn {
                id: "turn-2".to_owned(),
                status: TurnStatus::InProgress,
                items: vec![],
            },
        },
        &sink,
    );
    captured.lock().unwrap().events.clear();

    state.accept_writer_event(
        "thread-1",
        ThreadStreamEvent::AgentMessageDelta {
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-2".to_owned(),
            item_id: "agent-2".to_owned(),
            delta: "Latest text".to_owned(),
        },
        &sink,
    );

    assert!(state.pending_conversation_emit);
    assert!(captured.lock().unwrap().events.is_empty());
    state.flush_pending_live_conversation("thread-1", &sink);

    assert!(!state.pending_conversation_emit);
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
fn initial_read_grace_ends_after_the_first_successful_snapshot() {
    let thread_not_loaded = |thread_id: &str| CodexError::Server {
        method: "thread/read",
        code: -32600,
        message: format!("thread not loaded: {thread_id}"),
    };
    let mut initial_reads = InitialConversationReads::default();
    initial_reads.mark_started("thread-new");

    assert!(matches!(
        initial_reads.classify("thread-new", Err(thread_not_loaded("thread-new"))),
        Ok(PollSample::Unchanged)
    ));
    assert!(matches!(
        initial_reads.classify("thread-old", Err(thread_not_loaded("thread-old"))),
        Err(CodexError::Server { .. })
    ));
    assert!(matches!(
        initial_reads.classify("thread-new", Err(thread_not_loaded("thread-other"))),
        Err(CodexError::Server { .. })
    ));
    assert!(matches!(
        initial_reads.classify(
            "thread-new",
            Err(CodexError::Server {
                method: "thread/read",
                code: -32600,
                message: "invalid thread identifier".to_owned(),
            }),
        ),
        Err(CodexError::Server { .. })
    ));
    assert!(matches!(
        initial_reads.classify("thread-new", Ok(ThreadPoll::Unchanged)),
        Ok(PollSample::Unchanged)
    ));
    assert!(matches!(
        initial_reads.classify("thread-new", Err(thread_not_loaded("thread-new"))),
        Err(CodexError::Server { .. })
    ));
}

#[tokio::test]
async fn initial_read_grace_survives_switching_threads() {
    let mut state = ObserverState::new(PathBuf::from("/codex"), CodexHistoryCancellation::new());
    state.initial_conversation_reads.mark_started("thread-new");

    state.select_thread().await;

    assert!(matches!(
        state.initial_conversation_reads.classify(
            "thread-new",
            Err(CodexError::Server {
                method: "thread/read",
                code: -32600,
                message: "thread not loaded: thread-new".to_owned(),
            }),
        ),
        Ok(PollSample::Unchanged)
    ));
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
fn classifies_thread_start_failures_and_cancellation() {
    let start_error = || CodexError::Server {
        method: "thread/start",
        code: -1,
        message: "start failed".to_owned(),
    };

    assert!(matches!(
        classify_thread_start_result(Err(start_error()), false),
        ThreadStartEffect::Failed(message) if message.ends_with("start failed")
    ));
    assert!(matches!(
        classify_thread_start_result(Err(start_error()), true),
        ThreadStartEffect::Cancelled
    ));
}

#[tokio::test]
async fn emits_a_dedicated_thread_start_error() {
    let captured = Mutex::new(CapturedEvent::default());
    let sink = event_sink(&captured);
    let mut state = ObserverState::new(
        PathBuf::from("/craftward-tests/missing-codex"),
        CodexHistoryCancellation::new(),
    );

    let started_thread_id = state
        .start_thread(
            ThreadStartRequest {
                working_directory: PathBuf::from("/workspace"),
            },
            &sink,
        )
        .await;

    assert_eq!(started_thread_id, None);
    assert!(state.writer.is_none());
    let captured = captured.lock().unwrap();
    assert_eq!(captured.events.len(), 1);
    assert_eq!(
        captured.events[0].kind,
        wire::HistoryEventKind::ThreadStartError as i32
    );
    assert!(matches!(
        captured.events[0].body.as_ref(),
        Some(wire::history_event::Body::ErrorMessage(message))
            if message.contains("missing-codex")
    ));
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
