// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use ward_codex::{
    Activity, ActivityKind, ActivityStatus, CodexError, CodexHistoryCancellation,
    InteractionDecision, InteractionId, InteractionResponse, InteractionResponseBody,
    PendingInteraction, PendingInteractionKind, ThreadActiveFlag, ThreadItem, ThreadStreamEvent,
    ThreadSubscription, Turn, TurnOptions, TurnStatus,
};
use ward_codex_test_support::{FakeCodexAppServer, FakeCodexAppServerOptions, FakeTurnScenario};

use super::super::test_support::{CapturedEvent, event_sink, thread};
use super::*;
use crate::codex::observer::COMMAND_QUEUE_CAPACITY;
use crate::codex::observer::commands::{
    CommandUpdate, ObserverCommand, ThreadForkRequest, ThreadLifecycleAction,
    ThreadLifecycleRequest, ThreadListScope, ThreadRenameRequest, ThreadStartRequest,
    TurnSteerRequest,
};
use crate::codex::wire;

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
    assert!(state.writer.is_none());
    assert_eq!(state.live.runtime(), LiveRuntimeState::Detached);
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
    assert_eq!(
        state.writer.as_ref().map(CodexThreadWriter::thread_id),
        Some("thread-fork-1")
    );
    assert_eq!(state.live.runtime(), LiveRuntimeState::Idle);
    assert_eq!(
        state
            .live
            .conversation()
            .map(|thread| thread.summary.id.as_str()),
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
        let Some(wire::history_event::Body::ThreadWriteState(write_state)) =
            captured.events[3].body.as_ref()
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
    assert_eq!(
        state.writer.as_ref().map(CodexThreadWriter::thread_id),
        Some("thread-new")
    );
    assert_eq!(state.live.runtime(), LiveRuntimeState::Idle);
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
        state.writer.as_ref().map(CodexThreadWriter::thread_id),
        Some("thread-new")
    );

    captured.lock().unwrap().events.clear();
    let started = state
        .writer
        .as_mut()
        .expect("the source writer should remain available")
        .begin_text_turn("Keep working", TurnOptions::default())
        .await
        .expect("the source turn should start");
    state.accept_writer_event(&source_thread_id, started, &sink);
    assert!(matches!(
        state.live.runtime(),
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
            prompt: "Continue the fork".to_owned(),
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
        assert!(conversation.forkable_turn_ids.is_empty());
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
        assert_eq!(conversation.forkable_turn_ids, ["persisted-turn-1"]);
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
        turn_scenario: FakeTurnScenario::WaitForGuidance,
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

#[tokio::test]
async fn publishes_and_clears_command_approval_before_the_turn_completes() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        turn_scenario: FakeTurnScenario::RequestCommandApproval,
        ..FakeCodexAppServerOptions::default()
    });
    let captured = Arc::new(Mutex::new(CapturedEvent::default()));
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
    let (sender, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let producer_capture = Arc::clone(&captured);
    let producer = tokio::spawn(async move {
        let interaction_id = loop {
            let interaction_id = {
                let captured = producer_capture.lock().unwrap();
                captured.events.iter().find_map(|event| {
                    let Some(wire::history_event::Body::PendingInteractions(page)) =
                        event.body.as_ref()
                    else {
                        return None;
                    };
                    page.interactions
                        .first()
                        .map(|interaction| interaction.interaction_id)
                })
            };
            if let Some(interaction_id) = interaction_id {
                break interaction_id;
            }
            tokio::task::yield_now().await;
        };
        sender
            .send(ObserverCommand::ResolveInteraction(InteractionResponse {
                interaction_id: InteractionId::new(interaction_id).unwrap(),
                body: InteractionResponseBody::Decision(InteractionDecision::Accept),
            }))
            .await
            .unwrap();
        std::future::pending::<()>().await;
    });

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        state.run_turn(
            TurnRequest {
                thread_id: thread_id.clone(),
                prompt: "Run pwd".to_owned(),
                options: TurnOptions::default(),
            },
            &sink,
            &mut receiver,
            vec![],
        ),
    )
    .await
    .expect("the approved turn should not remain blocked");
    producer.abort();

    assert!(matches!(
        result,
        OperationDrive::Completed {
            output: true,
            deferred: None,
        }
    ));
    {
        let captured = captured.lock().unwrap();
        let interaction_snapshots = captured
            .events
            .iter()
            .enumerate()
            .filter_map(|(index, event)| {
                let Some(wire::history_event::Body::PendingInteractions(page)) =
                    event.body.as_ref()
                else {
                    return None;
                };
                Some((index, page.interactions.len()))
            })
            .collect::<Vec<_>>();
        assert_eq!(interaction_snapshots.len(), 2);
        assert_eq!(interaction_snapshots[0].1, 1);
        assert_eq!(interaction_snapshots[1].1, 0);
        let completed_index = captured
            .events
            .iter()
            .position(|event| event.kind == wire::HistoryEventKind::TurnCompleted as i32)
            .expect("the approved turn should report completion");
        assert!(interaction_snapshots[0].0 < interaction_snapshots[1].0);
        assert!(interaction_snapshots[1].0 < completed_index);

        let conversation = latest_conversation(&captured);
        assert_eq!(conversation.timeline.len(), 3);
        assert!(
            conversation
                .timeline
                .iter()
                .all(|item| item.turn_id == "live-turn-1")
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
