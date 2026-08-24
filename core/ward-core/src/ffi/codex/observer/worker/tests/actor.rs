// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;

struct ObserverHarness {
    app_server: FakeCodexAppServer,
    cancellation: CodexHistoryCancellation,
    captured: Arc<Mutex<CapturedEvent>>,
    commands: mpsc::Sender<ObserverCommand>,
    polling_enabled: watch::Sender<bool>,
    worker: tokio::task::JoinHandle<()>,
    event_cursor: usize,
}

impl ObserverHarness {
    fn start(app_server: FakeCodexAppServer, cancellation: CodexHistoryCancellation) -> Self {
        let captured = Arc::new(Mutex::new(CapturedEvent::default()));
        let sink = event_sink(&captured);
        let (commands, receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
        let (polling_enabled, polling_enabled_updates) = watch::channel(true);
        let worker = tokio::spawn(run_observer(
            app_server.source(),
            receiver,
            polling_enabled_updates,
            sink,
            cancellation.clone(),
            Arc::new(ObserverOperationGate::new()),
        ));
        Self {
            app_server,
            cancellation,
            captured,
            commands,
            polling_enabled,
            worker,
            event_cursor: 0,
        }
    }

    fn empty(options: FakeCodexAppServerOptions) -> Self {
        Self::start(
            FakeCodexAppServer::new(options),
            CodexHistoryCancellation::new(),
        )
    }

    async fn with_idle_thread(options: FakeCodexAppServerOptions) -> (Self, String) {
        let app_server = FakeCodexAppServer::new(options);
        let cancellation = CodexHistoryCancellation::new();
        let (writer, _) = CodexThreadWriter::start_on(
            &app_server.source(),
            cancellation.clone(),
            PathBuf::from("/workspace").as_path(),
            ThreadStartOptions::default(),
        )
        .await
        .expect("the fake app-server should seed persisted history");
        let thread_id = writer.thread_id().to_owned();
        writer.shutdown().await;
        (Self::start(app_server, cancellation), thread_id)
    }

    fn source(&self) -> ward_codex::CodexAppServerSource {
        self.app_server.source()
    }

    fn cancellation(&self) -> CodexHistoryCancellation {
        self.cancellation.clone()
    }

    fn app_server(&self) -> &FakeCodexAppServer {
        &self.app_server
    }

    async fn send(&self, command: ObserverCommand) {
        self.commands.send(command).await.unwrap();
    }

    fn set_polling_enabled(&self, enabled: bool) {
        self.polling_enabled.send(enabled).unwrap();
    }

    async fn wait_for_thread_page(&mut self, archived: bool, thread_ids: &[&str]) {
        wait_for_thread_page(&self.captured, &mut self.event_cursor, archived, thread_ids).await;
    }

    async fn wait_for_event(&mut self, kind: wire::HistoryEventKind) -> wire::HistoryEvent {
        wait_for_event_kind(&self.captured, &mut self.event_cursor, kind).await
    }

    async fn stop(self) {
        self.commands.send(ObserverCommand::Stop).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), self.worker)
            .await
            .expect("the observer should stop after its stop command")
            .expect("the observer task should not panic");
    }
}

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
    let (_polling_enabled, polling_enabled_updates) = watch::channel(true);
    let worker = tokio::spawn(run_observer(
        source,
        receiver,
        polling_enabled_updates,
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
    let (_polling_enabled, polling_enabled_updates) = watch::channel(true);
    let worker = tokio::spawn(run_observer(
        source,
        receiver,
        polling_enabled_updates,
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
async fn pauses_periodic_history_polling_until_it_is_enabled_again() {
    let mut observer = ObserverHarness::empty(FakeCodexAppServerOptions::default());

    observer.wait_for_thread_page(false, &[]).await;
    observer.set_polling_enabled(false);
    observer
        .send(ObserverCommand::SetThreadListScope(
            ThreadListScope::Archived,
        ))
        .await;
    observer.wait_for_thread_page(true, &[]).await;

    let requests_while_hidden = observer.app_server().thread_list_requests().len();
    tokio::time::sleep(Duration::from_millis(2_100)).await;
    assert_eq!(
        observer.app_server().thread_list_requests().len(),
        requests_while_hidden,
        "a hidden history page should not run the periodic thread-list poll"
    );

    observer.set_polling_enabled(true);
    tokio::time::timeout(Duration::from_secs(1), async {
        loop {
            if observer.app_server().thread_list_requests().len() > requests_while_hidden {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("showing the history page should refresh its thread list immediately");

    observer.stop().await;
}

#[tokio::test]
async fn does_not_reread_an_idle_watched_conversation() {
    let (mut observer, thread_id) =
        ObserverHarness::with_idle_thread(FakeCodexAppServerOptions::default()).await;

    observer.wait_for_thread_page(false, &[&thread_id]).await;
    observer.send(ObserverCommand::Watch(thread_id)).await;
    observer
        .wait_for_event(wire::HistoryEventKind::ConversationUpdated)
        .await;
    let reads_after_load = observer.app_server().thread_read_request_count();

    tokio::time::sleep(Duration::from_millis(1_100)).await;
    let reads_after_idle = observer.app_server().thread_read_request_count();

    observer.stop().await;
    assert_eq!(
        reads_after_idle, reads_after_load,
        "an idle observer should not repeatedly fetch the complete conversation"
    );
}

#[tokio::test]
async fn does_not_reread_the_watched_conversation_when_another_thread_changes() {
    let (mut observer, thread_id) =
        ObserverHarness::with_idle_thread(FakeCodexAppServerOptions::default()).await;

    observer.wait_for_thread_page(false, &[&thread_id]).await;
    observer
        .send(ObserverCommand::Watch(thread_id.clone()))
        .await;
    observer
        .wait_for_event(wire::HistoryEventKind::ConversationUpdated)
        .await;
    let reads_before_unrelated_change = observer.app_server().thread_read_request_count();

    let (fork_writer, fork_subscription) = CodexThreadWriter::fork_on(
        &observer.source(),
        observer.cancellation(),
        &thread_id,
        None,
    )
    .await
    .expect("another client should create an unrelated thread");
    let fork_id = fork_subscription.thread.summary.id;
    fork_writer.shutdown().await;

    observer
        .wait_for_thread_page(false, &[&thread_id, &fork_id])
        .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(
        observer.app_server().thread_read_request_count(),
        reads_before_unrelated_change,
        "an unrelated thread-list change should not reread the selected conversation"
    );

    observer.stop().await;
}

#[tokio::test]
async fn reloads_a_watched_conversation_after_an_external_turn() {
    let (mut observer, thread_id) =
        ObserverHarness::with_idle_thread(FakeCodexAppServerOptions::default()).await;

    observer.wait_for_thread_page(false, &[&thread_id]).await;
    observer
        .send(ObserverCommand::Watch(thread_id.clone()))
        .await;
    observer
        .wait_for_event(wire::HistoryEventKind::ConversationUpdated)
        .await;

    let (mut external_writer, _) =
        CodexThreadWriter::acquire_on(&observer.source(), observer.cancellation(), &thread_id)
            .await
            .expect("another client should acquire the idle thread");
    external_writer
        .begin_text_turn("External update", TurnOptions::default())
        .await
        .expect("the external turn should start");
    loop {
        if matches!(
            external_writer
                .next_subscription_event()
                .await
                .expect("the external turn should stream to completion"),
            ThreadStreamEvent::TurnCompleted { .. }
        ) {
            break;
        }
    }
    external_writer.shutdown().await;

    let updated = observer
        .wait_for_event(wire::HistoryEventKind::ConversationUpdated)
        .await;
    let Some(wire::history_event::Body::Conversation(conversation)) = updated.body else {
        panic!("the external update should carry the reloaded conversation");
    };
    assert_eq!(conversation.timeline.len(), 2);
    let reads_after_update = observer.app_server().thread_read_request_count();

    tokio::time::sleep(Duration::from_millis(1_100)).await;
    let reads_after_idle = observer.app_server().thread_read_request_count();

    observer.stop().await;
    assert_eq!(
        reads_after_idle, reads_after_update,
        "the observer should return to idle after loading the external turn"
    );
}

#[tokio::test]
async fn polls_an_external_active_turn_until_it_becomes_idle() {
    let (mut observer, thread_id) = ObserverHarness::with_idle_thread(FakeCodexAppServerOptions {
        turn_scenario: FakeTurnScenario::WaitForGuidance,
        ..FakeCodexAppServerOptions::default()
    })
    .await;

    observer.wait_for_thread_page(false, &[&thread_id]).await;
    observer
        .send(ObserverCommand::Watch(thread_id.clone()))
        .await;
    observer
        .wait_for_event(wire::HistoryEventKind::ConversationUpdated)
        .await;

    let (mut external_writer, _) =
        CodexThreadWriter::acquire_on(&observer.source(), observer.cancellation(), &thread_id)
            .await
            .expect("another client should acquire the idle thread");
    let started = external_writer
        .begin_text_turn("External active turn", TurnOptions::default())
        .await
        .expect("the external turn should start");
    let ThreadStreamEvent::TurnStarted { turn, .. } = started else {
        panic!("the external writer should receive the started turn");
    };

    observer
        .wait_for_event(wire::HistoryEventKind::ConversationUpdated)
        .await;
    let reads_after_active_load = observer.app_server().thread_read_request_count();
    let turn_pages_after_active_load = observer.app_server().thread_turns_list_requests().len();
    tokio::time::sleep(Duration::from_millis(1_100)).await;
    assert_eq!(
        observer.app_server().thread_read_request_count(),
        reads_after_active_load,
        "active follow-up polling should not reload the complete conversation"
    );
    let turn_requests = observer.app_server().thread_turns_list_requests();
    assert!(
        turn_requests.len() >= turn_pages_after_active_load + 2,
        "the observer should keep polling the active turn through paginated turn reads"
    );
    let incremental_requests = &turn_requests[turn_pages_after_active_load..];
    assert_eq!(incremental_requests[0].thread_id, thread_id);
    assert_eq!(incremental_requests[0].cursor, None);
    assert_eq!(incremental_requests[0].limit, Some(1));
    assert_eq!(
        incremental_requests[0].sort_direction.as_deref(),
        Some("desc")
    );
    assert_eq!(incremental_requests[0].items_view.as_deref(), Some("full"));
    assert!(incremental_requests[1].cursor.is_some());
    assert_eq!(
        incremental_requests[1].sort_direction.as_deref(),
        Some("asc")
    );

    external_writer
        .steer_text_turn(&turn.id, "Finish now")
        .await
        .expect("guidance should complete the external turn");
    loop {
        if matches!(
            external_writer
                .next_subscription_event()
                .await
                .expect("the external turn should stream to completion"),
            ThreadStreamEvent::TurnCompleted { .. }
        ) {
            break;
        }
    }
    external_writer.shutdown().await;
    observer
        .wait_for_event(wire::HistoryEventKind::ConversationUpdated)
        .await;
    let reads_after_completion = observer.app_server().thread_read_request_count();
    let turn_pages_after_completion = observer.app_server().thread_turns_list_requests().len();

    tokio::time::sleep(Duration::from_millis(1_100)).await;
    let reads_after_idle = observer.app_server().thread_read_request_count();
    let turn_pages_after_idle = observer.app_server().thread_turns_list_requests().len();

    observer.stop().await;
    assert_eq!(
        reads_after_idle, reads_after_completion,
        "the observer should stop full reads after the external turn completes"
    );
    assert_eq!(
        turn_pages_after_idle, turn_pages_after_completion,
        "the observer should stop incremental reads after the external turn completes"
    );
}

#[tokio::test]
async fn publishes_the_complete_model_catalog_when_the_observer_starts() {
    let fake_app_server = FakeCodexAppServer::default();
    let cancellation = CodexHistoryCancellation::new();
    let captured = Arc::new(Mutex::new(CapturedEvent::default()));
    let sink = event_sink(&captured);
    let (sender, receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let (_polling_enabled, polling_enabled_updates) = watch::channel(true);
    let worker = tokio::spawn(run_observer(
        fake_app_server.source(),
        receiver,
        polling_enabled_updates,
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
    let (_polling_enabled, polling_enabled_updates) = watch::channel(true);
    let worker = tokio::spawn(run_observer(
        fake_app_server.source(),
        receiver,
        polling_enabled_updates,
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
    let (_polling_enabled, polling_enabled_updates) = watch::channel(true);
    let worker = tokio::spawn(run_observer(
        source,
        receiver,
        polling_enabled_updates,
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
