// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;

#[tokio::test]
async fn applies_history_scope_and_lifecycle_commands_through_the_observer_actor() {
    let fake_app_server = FakeCodexAppServer::default();
    let source = fake_app_server.source();
    let cancellation = CodexHistoryCancellation::new();
    let working_directory = PathBuf::from("/workspace");
    let (writer, _) = CodexThreadWriter::start_on(
        &source,
        cancellation.clone(),
        &working_directory,
        ThreadStartOptions::default(),
    )
    .await
    .expect("the fake app-server should seed persisted history");
    let thread_id = writer.thread_id().to_owned();
    writer.shutdown().await;

    let captured = Arc::new(Mutex::new(CapturedEvent::default()));
    let sink = event_sink(&captured);
    let (sender, receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let worker = tokio::spawn(run_observer(
        source,
        receiver,
        sink,
        cancellation.clone(),
        Arc::new(ObserverOperationGate::new()),
    ));
    let mut event_cursor = 0;

    wait_for_thread_page(&captured, &mut event_cursor, false, &[thread_id.as_str()]).await;
    sender
        .send(ObserverCommand::ChangeThreadLifecycle(
            ThreadLifecycleRequest {
                thread_id: thread_id.clone(),
                action: ThreadLifecycleAction::Archive,
            },
        ))
        .await
        .unwrap();
    wait_for_thread_page(&captured, &mut event_cursor, false, &[]).await;

    sender
        .send(ObserverCommand::SetThreadListScope(
            ThreadListScope::Archived,
        ))
        .await
        .unwrap();
    wait_for_thread_page(&captured, &mut event_cursor, true, &[thread_id.as_str()]).await;
    sender
        .send(ObserverCommand::ChangeThreadLifecycle(
            ThreadLifecycleRequest {
                thread_id: thread_id.clone(),
                action: ThreadLifecycleAction::Restore,
            },
        ))
        .await
        .unwrap();
    wait_for_thread_page(&captured, &mut event_cursor, true, &[]).await;

    sender.send(ObserverCommand::Stop).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .expect("the observer should stop after its stop command")
        .expect("the observer task should not panic");
}

#[tokio::test]
async fn publishes_all_paginated_threads_for_active_and_archived_scopes() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        thread_list_page_size: Some(1),
        ..FakeCodexAppServerOptions::default()
    });
    let source = fake_app_server.source();
    let cancellation = CodexHistoryCancellation::new();
    let working_directory = PathBuf::from("/workspace");
    let (writer, _) = CodexThreadWriter::start_on(
        &source,
        cancellation.clone(),
        &working_directory,
        ThreadStartOptions::default(),
    )
    .await
    .expect("the fake app-server should seed persisted history");
    writer.shutdown().await;
    for expected_id in ["thread-fork-1", "thread-fork-2", "thread-fork-3"] {
        let (writer, subscription) =
            CodexThreadWriter::fork_on(&source, cancellation.clone(), "thread-new", None)
                .await
                .expect("the seeded thread should fork");
        assert_eq!(subscription.thread.summary.id, expected_id);
        writer.shutdown().await;
    }
    let mut history = CodexHistorySession::connect(source.clone())
        .await
        .expect("the history session should connect");
    for thread_id in ["thread-fork-2", "thread-fork-3"] {
        history
            .archive_thread(thread_id)
            .await
            .expect("the seeded thread should be archived");
    }
    history.shutdown().await;

    let captured = Arc::new(Mutex::new(CapturedEvent::default()));
    let sink = event_sink(&captured);
    let (sender, receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let worker = tokio::spawn(run_observer(
        source,
        receiver,
        sink,
        cancellation,
        Arc::new(ObserverOperationGate::new()),
    ));
    let mut event_cursor = 0;

    wait_for_thread_page(
        &captured,
        &mut event_cursor,
        false,
        &["thread-new", "thread-fork-1"],
    )
    .await;
    sender
        .send(ObserverCommand::SetThreadListScope(
            ThreadListScope::Archived,
        ))
        .await
        .unwrap();
    wait_for_thread_page(
        &captured,
        &mut event_cursor,
        true,
        &["thread-fork-2", "thread-fork-3"],
    )
    .await;

    sender.send(ObserverCommand::Stop).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .expect("the observer should stop after its stop command")
        .expect("the observer task should not panic");
}

#[tokio::test]
async fn publishes_the_complete_model_catalog_when_the_observer_starts() {
    let fake_app_server = FakeCodexAppServer::default();
    let cancellation = CodexHistoryCancellation::new();
    let captured = Arc::new(Mutex::new(CapturedEvent::default()));
    let sink = event_sink(&captured);
    let (sender, receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let worker = tokio::spawn(run_observer(
        fake_app_server.source(),
        receiver,
        sink,
        cancellation,
        Arc::new(ObserverOperationGate::new()),
    ));
    let mut event_cursor = 0;

    let event = wait_for_event_kind(
        &captured,
        &mut event_cursor,
        wire::HistoryEventKind::ModelCatalogUpdated,
    )
    .await;
    assert_eq!(event.thread_id, None);
    let Some(wire::history_event::Body::ModelCatalog(catalog)) = event.body else {
        panic!("the model-catalog event should carry the complete catalog");
    };
    assert_eq!(catalog.models.len(), 2);
    assert_eq!(catalog.models[0].model_id, "balanced");
    assert_eq!(catalog.models[0].model, "gpt-balanced");
    assert_eq!(catalog.models[0].default_reasoning_effort, "medium");
    assert_eq!(
        catalog.models[0].supported_reasoning_efforts[0].reasoning_effort,
        "low"
    );
    assert_eq!(catalog.models[1].model_id, "fast");

    sender.send(ObserverCommand::Stop).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .expect("the observer should stop after its stop command")
        .expect("the observer task should not panic");
}

#[tokio::test]
async fn reports_and_recovers_from_a_model_catalog_error_without_blocking_history() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        model_list_failures: 1,
        ..FakeCodexAppServerOptions::default()
    });
    let cancellation = CodexHistoryCancellation::new();
    let captured = Arc::new(Mutex::new(CapturedEvent::default()));
    let sink = event_sink(&captured);
    let (sender, receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let worker = tokio::spawn(run_observer(
        fake_app_server.source(),
        receiver,
        sink,
        cancellation,
        Arc::new(ObserverOperationGate::new()),
    ));
    let mut event_cursor = 0;

    wait_for_event_kind(
        &captured,
        &mut event_cursor,
        wire::HistoryEventKind::ThreadsUpdated,
    )
    .await;
    let failed = wait_for_event_kind(
        &captured,
        &mut event_cursor,
        wire::HistoryEventKind::ModelCatalogError,
    )
    .await;
    assert_eq!(failed.thread_id, None);
    assert!(matches!(
        failed.body,
        Some(wire::history_event::Body::ErrorMessage(message))
            if message.contains("model catalog is temporarily unavailable")
    ));
    wait_for_event_kind(
        &captured,
        &mut event_cursor,
        wire::HistoryEventKind::ModelCatalogUpdated,
    )
    .await;

    sender.send(ObserverCommand::Stop).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .expect("the observer should stop after its stop command")
        .expect("the observer task should not panic");
}

#[tokio::test]
async fn forks_and_selects_a_thread_through_the_observer_actor() {
    let fake_app_server = FakeCodexAppServer::default();
    let source = fake_app_server.source();
    let cancellation = CodexHistoryCancellation::new();
    let (mut writer, _) = CodexThreadWriter::start_on(
        &source,
        cancellation.clone(),
        PathBuf::from("/workspace").as_path(),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the fake app-server should seed persisted history");
    for prompt in ["First direction", "Second direction", "Third direction"] {
        writer
            .begin_text_turn(prompt, TurnOptions::default())
            .await
            .expect("the seeded turn should start");
        loop {
            let event = writer
                .next_subscription_event()
                .await
                .expect("the seeded turn should stream to completion");
            if matches!(event, ThreadStreamEvent::TurnCompleted { .. }) {
                break;
            }
        }
    }
    writer.shutdown().await;

    let captured = Arc::new(Mutex::new(CapturedEvent::default()));
    let sink = event_sink(&captured);
    let (sender, receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let worker = tokio::spawn(run_observer(
        source,
        receiver,
        sink,
        cancellation.clone(),
        Arc::new(ObserverOperationGate::new()),
    ));
    let mut event_cursor = 0;

    wait_for_thread_page(&captured, &mut event_cursor, false, &["thread-new"]).await;
    sender
        .send(ObserverCommand::Watch("thread-new".to_owned()))
        .await
        .unwrap();
    wait_for_event_kind(
        &captured,
        &mut event_cursor,
        wire::HistoryEventKind::ConversationUpdated,
    )
    .await;
    sender
        .send(ObserverCommand::AcquireWrite("thread-new".to_owned()))
        .await
        .unwrap();
    let write_event = wait_for_event_kind(
        &captured,
        &mut event_cursor,
        wire::HistoryEventKind::ThreadWriteStateChanged,
    )
    .await;
    let Some(wire::history_event::Body::ThreadWriteState(write_state)) = write_event.body else {
        panic!("the write-state event should carry its status");
    };
    assert_eq!(write_state.status, wire::ThreadWriteStatus::Checking as i32);
    let writable_event = wait_for_event_kind(
        &captured,
        &mut event_cursor,
        wire::HistoryEventKind::ThreadWriteStateChanged,
    )
    .await;
    let Some(wire::history_event::Body::ThreadWriteState(write_state)) = writable_event.body else {
        panic!("the write-state event should carry its status");
    };
    assert_eq!(write_state.status, wire::ThreadWriteStatus::Writable as i32);

    sender
        .send(ObserverCommand::ForkThread(ThreadForkRequest {
            thread_id: "thread-new".to_owned(),
            last_turn_id: "live-turn-1".to_owned(),
        }))
        .await
        .unwrap();
    let forked = wait_for_event_kind(
        &captured,
        &mut event_cursor,
        wire::HistoryEventKind::ThreadForked,
    )
    .await;
    assert_eq!(forked.thread_id.as_deref(), Some("thread-fork-1"));
    let Some(wire::history_event::Body::Conversation(conversation)) = forked.body else {
        panic!("the fork event should carry the truncated conversation");
    };
    assert_eq!(conversation.timeline.len(), 2);
    assert!(
        conversation
            .timeline
            .iter()
            .all(|item| item.turn_id == "live-turn-1")
    );
    wait_for_thread_page(
        &captured,
        &mut event_cursor,
        false,
        &["thread-new", "thread-fork-1"],
    )
    .await;

    sender
        .send(ObserverCommand::StartTurn(TurnRequest {
            thread_id: "thread-fork-1".to_owned(),
            input: vec![TurnInput::Text("Continue the fork".to_owned())],
            options: TurnOptions::default(),
        }))
        .await
        .unwrap();
    let completed = wait_for_event_kind(
        &captured,
        &mut event_cursor,
        wire::HistoryEventKind::TurnCompleted,
    )
    .await;
    assert_eq!(completed.thread_id.as_deref(), Some("thread-fork-1"));

    sender.send(ObserverCommand::Stop).await.unwrap();
    tokio::time::timeout(Duration::from_secs(5), worker)
        .await
        .expect("the observer should stop after its stop command")
        .expect("the observer task should not panic");
}

async fn wait_for_event_kind(
    captured: &Mutex<CapturedEvent>,
    event_cursor: &mut usize,
    kind: wire::HistoryEventKind,
) -> wire::HistoryEvent {
    let event = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let sample = {
                let captured = captured.lock().unwrap();
                captured.events[*event_cursor..]
                    .iter()
                    .enumerate()
                    .find(|(_, event)| event.kind == kind as i32)
                    .map(|(offset, event)| (*event_cursor + offset + 1, event.clone()))
            };
            if let Some(sample) = sample {
                break sample;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the observer should emit the requested event");
    *event_cursor = event.0;
    event.1
}

async fn wait_for_thread_page(
    captured: &Mutex<CapturedEvent>,
    event_cursor: &mut usize,
    archived: bool,
    thread_ids: &[&str],
) {
    let sample = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let sample = {
                let captured = captured.lock().unwrap();
                captured.events[*event_cursor..]
                    .iter()
                    .enumerate()
                    .find_map(|(offset, event)| {
                        let Some(wire::history_event::Body::ThreadPage(page)) = event.body.as_ref()
                        else {
                            return None;
                        };
                        Some((
                            *event_cursor + offset + 1,
                            event.archived,
                            page.threads
                                .iter()
                                .map(|thread| thread.thread_id.clone())
                                .collect::<Vec<_>>(),
                        ))
                    })
            };
            if let Some(sample) = sample {
                break sample;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the observer should emit the requested thread page");

    *event_cursor = sample.0;
    assert_eq!(sample.1, Some(archived));
    assert_eq!(
        sample.2,
        thread_ids
            .iter()
            .map(|thread_id| (*thread_id).to_owned())
            .collect::<Vec<_>>()
    );
}
