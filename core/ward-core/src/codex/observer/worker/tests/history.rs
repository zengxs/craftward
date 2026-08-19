// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;

async fn complete_fake_turn(state: &mut ObserverState, thread_id: &str, sink: &HistoryEventSink) {
    let (_commands, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let OperationDrive::Completed {
        output: completed,
        deferred,
    } = state
        .run_turn(
            TurnRequest {
                thread_id: thread_id.to_owned(),
                prompt: "Seed the fork boundary".to_owned(),
                options: TurnOptions::default(),
            },
            sink,
            &mut receiver,
            vec![],
        )
        .await
    else {
        panic!("the fake turn should complete without stopping the observer");
    };
    assert!(completed);
    assert!(deferred.is_none());
}

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
    let snapshot = state.test_snapshot();
    assert_eq!(snapshot.writer_thread_id.as_deref(), Some("thread-new"));
    assert_eq!(snapshot.runtime, LiveRuntimeState::Idle);

    {
        let captured = captured.lock().unwrap();
        assert_eq!(captured.events.len(), 5);
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
            wire::HistoryEventKind::ThreadModelChanged as i32
        );
        let Some(wire::history_event::Body::ThreadModelState(model_state)) =
            captured.events[1].body.as_ref()
        else {
            panic!("the start event must report the thread model");
        };
        assert_eq!(model_state.model, "gpt-balanced");
        assert_eq!(model_state.reasoning_effort.as_deref(), Some("medium"));
        assert_eq!(
            captured.events[2].kind,
            wire::HistoryEventKind::PendingInteractionsUpdated as i32
        );
        assert_eq!(
            captured.events[3].kind,
            wire::HistoryEventKind::ThreadRuntimeStateChanged as i32
        );
        assert_eq!(
            captured.events[4].kind,
            wire::HistoryEventKind::ThreadWriteStateChanged as i32
        );
    }

    state.shutdown().await;
}

#[tokio::test]
async fn publishes_conversation_inference_changes_after_the_turn_is_accepted() {
    let fake_app_server = FakeCodexAppServer::default();
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
    captured.lock().unwrap().events.clear();
    let (_commands, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);

    let result = state
        .run_turn(
            TurnRequest {
                thread_id: thread_id.clone(),
                prompt: "Use the faster model".to_owned(),
                options: TurnOptions {
                    inference: ReasoningEffort::new("low")
                        .map(|effort| InferenceOverride::selection("gpt-fast", effort)),
                    ..TurnOptions::default()
                },
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
    let snapshot = state.test_snapshot();
    assert_eq!(snapshot.active_model.as_deref(), Some("gpt-fast"));
    assert_eq!(snapshot.active_reasoning_effort.as_deref(), Some("low"));
    {
        let captured = captured.lock().unwrap();
        let model_state = captured
            .events
            .iter()
            .find_map(|event| match event.body.as_ref() {
                Some(wire::history_event::Body::ThreadModelState(model_state)) => Some(model_state),
                _ => None,
            })
            .expect("the accepted turn should publish the selected model");
        assert_eq!(model_state.model, "gpt-fast");
        assert_eq!(model_state.reasoning_effort.as_deref(), Some("low"));
    }

    state.shutdown().await;
}

#[tokio::test]
async fn renames_a_persisted_thread_and_refreshes_its_public_snapshots() {
    let fake_app_server = FakeCodexAppServer::default();
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
    captured.lock().unwrap().events.clear();

    assert!(
        state
            .rename_thread(
                ThreadRenameRequest {
                    thread_id: thread_id.clone(),
                    name: "Focused work".to_owned(),
                },
                &sink,
            )
            .await
    );
    assert!(state.poll_threads(&sink).await);
    assert!(state.poll_conversation(&thread_id, &sink).await);

    {
        let captured = captured.lock().unwrap();
        let thread_page = captured
            .events
            .iter()
            .find_map(|event| match event.body.as_ref() {
                Some(wire::history_event::Body::ThreadPage(page)) => Some(page),
                _ => None,
            })
            .expect("renaming should refresh the public thread page");
        assert_eq!(thread_page.threads.len(), 1);
        assert_eq!(thread_page.threads[0].name.as_deref(), Some("Focused work"));
        assert_eq!(latest_conversation(&captured).title, "Focused work");
    }

    state.shutdown().await;
}

#[tokio::test]
async fn forks_the_loaded_idle_thread_through_the_selected_turn() {
    let fake_app_server = FakeCodexAppServer::default();
    let captured = Mutex::new(CapturedEvent::default());
    let sink = event_sink(&captured);
    let mut state = ObserverState::new(fake_app_server.source(), CodexHistoryCancellation::new());
    let source_thread_id = state
        .start_thread(
            ThreadStartRequest {
                working_directory: PathBuf::from("/workspace"),
            },
            &sink,
        )
        .await
        .expect("the fake app-server should start a thread");
    complete_fake_turn(&mut state, &source_thread_id, &sink).await;
    state.release_write(&source_thread_id, &sink).await;
    let snapshot = state.test_snapshot();
    assert!(snapshot.writer_thread_id.is_none());
    assert_eq!(snapshot.runtime, LiveRuntimeState::Detached);
    captured.lock().unwrap().events.clear();

    let forked_thread_id = state
        .fork_thread(
            ThreadForkRequest {
                thread_id: source_thread_id,
                last_turn_id: "live-turn-1".to_owned(),
            },
            &sink,
        )
        .await;

    assert_eq!(forked_thread_id.as_deref(), Some("thread-fork-1"));
    let snapshot = state.test_snapshot();
    assert_eq!(snapshot.writer_thread_id.as_deref(), Some("thread-fork-1"));
    assert_eq!(snapshot.runtime, LiveRuntimeState::Idle);
    assert_eq!(
        snapshot.conversation_thread_id.as_deref(),
        Some("thread-fork-1")
    );
    assert!(state.poll_threads(&sink).await);

    {
        let captured = captured.lock().unwrap();
        assert_eq!(
            captured.events[0].kind,
            wire::HistoryEventKind::ThreadForked as i32
        );
        assert_eq!(
            captured.events[0].thread_id.as_deref(),
            Some("thread-fork-1")
        );
        let Some(wire::history_event::Body::Conversation(conversation)) =
            captured.events[0].body.as_ref()
        else {
            panic!("the fork event should contain the copied conversation");
        };
        assert_eq!(conversation.timeline.len(), 2);
        assert!(
            conversation
                .timeline
                .iter()
                .all(|item| item.turn_id == "live-turn-1")
        );
        let Some(wire::history_event::Body::ThreadModelState(model_state)) =
            captured.events[1].body.as_ref()
        else {
            panic!("the fork should publish its inherited model");
        };
        assert_eq!(model_state.model, "gpt-balanced");
        assert_eq!(model_state.reasoning_effort.as_deref(), Some("medium"));
        let Some(wire::history_event::Body::ThreadWriteState(write_state)) =
            captured.events[4].body.as_ref()
        else {
            panic!("the fork should publish its adopted writer state");
        };
        assert_eq!(write_state.status, wire::ThreadWriteStatus::Writable as i32);
        let page = captured
            .events
            .iter()
            .find_map(|event| match event.body.as_ref() {
                Some(wire::history_event::Body::ThreadPage(page)) => Some(page),
                _ => None,
            })
            .expect("forking should refresh the active thread page");
        let mut ids = page
            .threads
            .iter()
            .map(|thread| thread.thread_id.as_str())
            .collect::<Vec<_>>();
        ids.sort_unstable();
        assert_eq!(ids, ["thread-fork-1", "thread-new"]);
    }

    state.shutdown().await;
}

#[tokio::test]
async fn preserves_the_source_writer_when_a_fork_response_is_lost() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        lose_first_fork_response: true,
        ..FakeCodexAppServerOptions::default()
    });
    let captured = Mutex::new(CapturedEvent::default());
    let sink = event_sink(&captured);
    let mut state = ObserverState::new(fake_app_server.source(), CodexHistoryCancellation::new());
    let source_thread_id = state
        .start_thread(
            ThreadStartRequest {
                working_directory: PathBuf::from("/workspace"),
            },
            &sink,
        )
        .await
        .expect("the fake app-server should start a thread");
    complete_fake_turn(&mut state, &source_thread_id, &sink).await;
    captured.lock().unwrap().events.clear();

    let forked_thread_id = state
        .fork_thread(
            ThreadForkRequest {
                thread_id: source_thread_id,
                last_turn_id: "live-turn-1".to_owned(),
            },
            &sink,
        )
        .await;

    assert_eq!(forked_thread_id, None);
    let snapshot = state.test_snapshot();
    assert_eq!(snapshot.writer_thread_id.as_deref(), Some("thread-new"));
    assert_eq!(snapshot.runtime, LiveRuntimeState::Idle);
    assert!(state.poll_threads(&sink).await);

    {
        let captured = captured.lock().unwrap();
        assert_eq!(
            captured.events[0].kind,
            wire::HistoryEventKind::ThreadForkError as i32
        );
        let page = captured
            .events
            .iter()
            .find_map(|event| match event.body.as_ref() {
                Some(wire::history_event::Body::ThreadPage(page)) => Some(page),
                _ => None,
            })
            .expect("the uncertain fork should still refresh the thread page");
        assert_eq!(page.threads.len(), 2);
    }

    state.shutdown().await;
}

#[tokio::test]
async fn refreshes_the_thread_page_after_a_fork_writer_conflict() {
    let fake_app_server = FakeCodexAppServer::default();
    let source = fake_app_server.source();
    let cancellation = CodexHistoryCancellation::new();
    let (mut external_writer, _) = CodexThreadWriter::start_on(
        &source,
        cancellation.clone(),
        PathBuf::from("/workspace").as_path(),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the external writer should seed persisted history");
    external_writer
        .begin_text_turn("Seed the fork boundary", TurnOptions::default())
        .await
        .expect("the seeded turn should start");
    loop {
        if matches!(
            external_writer
                .next_subscription_event()
                .await
                .expect("the seeded turn should complete"),
            ThreadStreamEvent::TurnCompleted { .. }
        ) {
            break;
        }
    }

    let captured = Mutex::new(CapturedEvent::default());
    let sink = event_sink(&captured);
    let mut state = ObserverState::new(source, cancellation);
    assert!(state.poll_threads(&sink).await);
    assert!(state.poll_conversation("thread-new", &sink).await);
    captured.lock().unwrap().events.clear();

    assert_eq!(
        state
            .fork_thread(
                ThreadForkRequest {
                    thread_id: "thread-new".to_owned(),
                    last_turn_id: "live-turn-1".to_owned(),
                },
                &sink,
            )
            .await,
        None
    );
    assert!(state.poll_threads(&sink).await);

    {
        let captured = captured.lock().unwrap();
        assert!(
            captured
                .events
                .iter()
                .any(|event| { event.kind == wire::HistoryEventKind::ThreadForkError as i32 })
        );
        assert!(
            captured.events.iter().any(|event| {
                matches!(event.body, Some(wire::history_event::Body::ThreadPage(_)))
            })
        );
    }

    external_writer.shutdown().await;
    state.shutdown().await;
}

#[tokio::test]
async fn rejects_forking_archived_unloaded_or_nonidle_history() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        turn_scenario: FakeTurnScenario::WaitForGuidance,
        ..FakeCodexAppServerOptions::default()
    });
    let captured = Mutex::new(CapturedEvent::default());
    let sink = event_sink(&captured);
    let mut state = ObserverState::new(fake_app_server.source(), CodexHistoryCancellation::new());
    let source_thread_id = state
        .start_thread(
            ThreadStartRequest {
                working_directory: PathBuf::from("/workspace"),
            },
            &sink,
        )
        .await
        .expect("the fake app-server should start a thread");
    captured.lock().unwrap().events.clear();

    state.set_thread_list_scope(ThreadListScope::Archived);
    assert_eq!(
        state
            .fork_thread(
                ThreadForkRequest {
                    thread_id: source_thread_id.clone(),
                    last_turn_id: "live-turn-1".to_owned(),
                },
                &sink,
            )
            .await,
        None
    );
    assert_eq!(
        captured.lock().unwrap().events.last().unwrap().kind,
        wire::HistoryEventKind::ThreadForkError as i32
    );

    state.set_thread_list_scope(ThreadListScope::Active);
    captured.lock().unwrap().events.clear();
    assert_eq!(
        state
            .fork_thread(
                ThreadForkRequest {
                    thread_id: "thread-missing".to_owned(),
                    last_turn_id: "live-turn-1".to_owned(),
                },
                &sink,
            )
            .await,
        None
    );
    assert_eq!(
        captured.lock().unwrap().events.last().unwrap().kind,
        wire::HistoryEventKind::ThreadForkError as i32
    );
    assert_eq!(
        state.test_snapshot().writer_thread_id.as_deref(),
        Some("thread-new")
    );

    captured.lock().unwrap().events.clear();
    state.accept_writer_event(
        &source_thread_id,
        ThreadStreamEvent::TurnStarted {
            thread_id: source_thread_id.clone(),
            turn: Turn {
                id: "live-turn-2".to_owned(),
                status: TurnStatus::InProgress,
                items: vec![],
            },
        },
        &sink,
    );
    assert!(matches!(
        state.test_snapshot().runtime,
        LiveRuntimeState::Active { .. }
    ));

    assert_eq!(
        state
            .fork_thread(
                ThreadForkRequest {
                    thread_id: source_thread_id,
                    last_turn_id: "live-turn-1".to_owned(),
                },
                &sink,
            )
            .await,
        None
    );
    assert_eq!(
        captured.lock().unwrap().events.last().unwrap().kind,
        wire::HistoryEventKind::ThreadForkError as i32
    );

    state.shutdown().await;
}

#[tokio::test]
async fn archives_and_restores_a_thread_with_scope_tagged_authoritative_snapshots() {
    let fake_app_server = FakeCodexAppServer::default();
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
    captured.lock().unwrap().events.clear();

    assert!(state.poll_threads(&sink).await);
    assert_thread_page(&captured, false, &[thread_id.as_str()]);

    assert!(
        state
            .change_thread_lifecycle(
                ThreadLifecycleRequest {
                    thread_id: thread_id.clone(),
                    action: ThreadLifecycleAction::Archive,
                },
                &sink,
            )
            .await
    );
    assert!(state.poll_threads(&sink).await);
    assert_thread_page(&captured, false, &[]);

    state.set_thread_list_scope(ThreadListScope::Archived);
    assert!(state.poll_threads(&sink).await);
    assert_thread_page(&captured, true, &[thread_id.as_str()]);

    assert!(
        state
            .change_thread_lifecycle(
                ThreadLifecycleRequest {
                    thread_id: thread_id.clone(),
                    action: ThreadLifecycleAction::Restore,
                },
                &sink,
            )
            .await
    );
    assert!(state.poll_threads(&sink).await);
    assert_thread_page(&captured, true, &[]);

    state.set_thread_list_scope(ThreadListScope::Active);
    assert!(state.poll_threads(&sink).await);
    assert_thread_page(&captured, false, &[thread_id.as_str()]);

    captured.lock().unwrap().events.clear();
    assert!(
        !state
            .change_thread_lifecycle(
                ThreadLifecycleRequest {
                    thread_id,
                    action: ThreadLifecycleAction::Restore,
                },
                &sink,
            )
            .await
    );
    assert_eq!(
        captured.lock().unwrap().events.last().unwrap().kind,
        wire::HistoryEventKind::ThreadLifecycleError as i32
    );

    state.shutdown().await;
}
