// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;
use std::time::Duration;

use ward_codex::{
    ActivityKind, ActivityStatus, CodexError, CodexHistoryCancellation, CodexHistorySession,
    CodexThreadWriter, InferenceOverride, InteractionAnswer, InteractionDecision,
    InteractionResponse, InteractionResponseBody, PendingInteraction, PendingInteractionKind,
    ReasoningEffort, ThreadItem, ThreadListOptions, ThreadPagePoll, ThreadPoll,
    ThreadRuntimeStatus, ThreadStartOptions, ThreadStreamEvent, TurnInput, TurnOptions, TurnStatus,
    UserInput,
};
use ward_codex_test_support::{FakeCodexAppServer, FakeCodexAppServerOptions, FakeTurnScenario};

#[tokio::test]
async fn starts_a_thread_through_the_public_writer_seam() {
    let fake_app_server = FakeCodexAppServer::default();
    let source = fake_app_server.source();

    let (writer, subscription) = CodexThreadWriter::start_on(
        &source,
        CodexHistoryCancellation::new(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the public writer seam should start a thread");

    assert_eq!(writer.thread_id(), "thread-new");
    assert_eq!(writer.active_model(), Some("gpt-balanced"));
    assert_eq!(writer.active_reasoning_effort(), Some("medium"));
    assert_eq!(subscription.thread.summary.id, "thread-new");
    assert_eq!(subscription.thread.summary.cwd, Path::new("/workspace"));
    assert_eq!(subscription.runtime_status, ThreadRuntimeStatus::Idle);

    writer.shutdown().await;
}

#[tokio::test]
async fn starts_a_turn_with_typed_attachments_through_the_public_writer_seam() {
    let fake_app_server = FakeCodexAppServer::default();
    let source = fake_app_server.source();
    let (mut writer, _) = CodexThreadWriter::start_on(
        &source,
        CodexHistoryCancellation::new(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the public writer seam should start a thread");
    let input = vec![
        TurnInput::Text("Compare these screenshots".to_owned()),
        TurnInput::LocalImage {
            path: "/workspace/before.png".into(),
        },
        TurnInput::LocalAudio {
            path: "/workspace/note.wav".into(),
        },
        TurnInput::Mention {
            name: "requirements.pdf".to_owned(),
            path: "/workspace/requirements.pdf".into(),
        },
    ];

    writer
        .begin_turn(&input, TurnOptions::default())
        .await
        .expect("the attachment turn should start");
    let ThreadStreamEvent::ItemStarted {
        item: ThreadItem::UserMessage { content, .. },
        ..
    } = next_fake_turn_event(&mut writer).await
    else {
        panic!("the attachment turn should stream its user input first");
    };
    assert_eq!(
        content,
        vec![
            UserInput::Text("Compare these screenshots".to_owned()),
            UserInput::LocalImage {
                path: "/workspace/before.png".into(),
            },
            UserInput::LocalAudio {
                path: "/workspace/note.wav".into(),
            },
            UserInput::Mention {
                name: "requirements.pdf".to_owned(),
                path: "/workspace/requirements.pdf".into(),
            },
        ]
    );

    let outcome = finish_fake_turn(&mut writer).await;
    assert_eq!(outcome.answer, "Done.");
    writer.shutdown().await;
}

#[tokio::test]
async fn continues_without_exposing_empty_input_as_a_regular_turn() {
    let fake_app_server = FakeCodexAppServer::default();
    let source = fake_app_server.source();
    let (mut writer, _) = CodexThreadWriter::start_on(
        &source,
        CodexHistoryCancellation::new(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the public writer seam should start a thread");

    assert!(matches!(
        writer.begin_turn(&[], TurnOptions::default()).await,
        Err(CodexError::InvalidTurnInput { .. })
    ));
    writer
        .continue_turn(TurnOptions::default())
        .await
        .expect("the explicit continuation should start a turn");
    let outcome = finish_fake_turn(&mut writer).await;
    assert_eq!(outcome.answer, "Done.");

    writer.shutdown().await;
}

#[tokio::test]
async fn changes_and_restores_conversation_inference_options_through_the_public_writer_seam() {
    let fake_app_server = FakeCodexAppServer::default();
    let source = fake_app_server.source();
    let cancellation = CodexHistoryCancellation::new();
    let (mut writer, _) = CodexThreadWriter::start_on(
        &source,
        cancellation.clone(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the public writer seam should start a thread");

    let rejected = writer
        .begin_text_turn(
            "Try an unavailable model",
            TurnOptions {
                inference: Some(InferenceOverride::model("gpt-missing")),
                ..TurnOptions::default()
            },
        )
        .await;
    assert!(matches!(
        rejected,
        Err(CodexError::UnsupportedTurnControls { .. })
    ));
    assert_eq!(writer.active_model(), Some("gpt-balanced"));
    assert_eq!(writer.active_reasoning_effort(), Some("medium"));

    let rejected = writer
        .begin_text_turn(
            "Try an unavailable reasoning effort",
            TurnOptions {
                inference: ReasoningEffort::new("ultra").map(InferenceOverride::reasoning_effort),
                ..TurnOptions::default()
            },
        )
        .await;
    assert!(matches!(
        rejected,
        Err(CodexError::Server {
            method: "turn/start",
            ..
        })
    ));
    assert_eq!(writer.active_reasoning_effort(), Some("medium"));

    writer
        .begin_text_turn(
            "Use deeper reasoning",
            TurnOptions {
                inference: ReasoningEffort::new("high").map(InferenceOverride::reasoning_effort),
                ..TurnOptions::default()
            },
        )
        .await
        .expect("the reasoning-effort-changing turn should start");
    finish_fake_turn(&mut writer).await;
    assert_eq!(writer.active_reasoning_effort(), Some("high"));

    writer
        .begin_text_turn(
            "Use the faster model",
            TurnOptions {
                inference: ReasoningEffort::new("low")
                    .map(|effort| InferenceOverride::selection("gpt-fast", effort)),
                ..TurnOptions::default()
            },
        )
        .await
        .expect("the model-changing turn should start");
    finish_fake_turn(&mut writer).await;
    assert_eq!(writer.active_model(), Some("gpt-fast"));
    assert_eq!(writer.active_reasoning_effort(), Some("low"));
    writer.shutdown().await;

    let (writer, _) = CodexThreadWriter::acquire_on(&source, cancellation, "thread-new")
        .await
        .expect("the thread should resume with its selected model");
    assert_eq!(writer.active_model(), Some("gpt-fast"));
    assert_eq!(writer.active_reasoning_effort(), Some("low"));
    writer.shutdown().await;
}

#[tokio::test]
async fn defaults_the_effort_when_a_model_only_override_rejects_the_active_effort() {
    let fake_app_server = FakeCodexAppServer::default();
    let source = fake_app_server.source();
    let cancellation = CodexHistoryCancellation::new();
    let (mut writer, _) = CodexThreadWriter::start_on(
        &source,
        cancellation.clone(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the public writer seam should start a thread");

    writer
        .begin_text_turn(
            "Use deeper reasoning",
            TurnOptions {
                inference: ReasoningEffort::new("high").map(InferenceOverride::reasoning_effort),
                ..TurnOptions::default()
            },
        )
        .await
        .expect("the reasoning-effort-changing turn should start");
    finish_fake_turn(&mut writer).await;
    assert_eq!(writer.active_reasoning_effort(), Some("high"));

    writer
        .begin_text_turn(
            "Use the faster model",
            TurnOptions {
                inference: Some(InferenceOverride::model("gpt-fast")),
                ..TurnOptions::default()
            },
        )
        .await
        .expect("the model-changing turn should start");
    assert_eq!(writer.active_model(), Some("gpt-fast"));
    assert_eq!(writer.active_reasoning_effort(), Some("low"));
    finish_fake_turn(&mut writer).await;

    writer.shutdown().await;

    let (writer, _) = CodexThreadWriter::acquire_on(&source, cancellation, "thread-new")
        .await
        .expect("the thread should resume with the server's effective inference options");
    assert_eq!(writer.active_model(), Some("gpt-fast"));
    assert_eq!(writer.active_reasoning_effort(), Some("low"));
    writer.shutdown().await;
}

#[tokio::test]
async fn starts_an_ephemeral_thread_when_the_app_server_confirms_it() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        confirm_ephemeral_thread_starts: true,
        ..FakeCodexAppServerOptions::default()
    });
    let source = fake_app_server.source();

    let (writer, subscription) = CodexThreadWriter::start_on(
        &source,
        CodexHistoryCancellation::new(),
        Path::new("/workspace"),
        ThreadStartOptions { ephemeral: true },
    )
    .await
    .expect("the app-server should confirm the ephemeral thread");

    assert_eq!(writer.thread_id(), "thread-new");
    assert_eq!(subscription.thread.summary.id, "thread-new");

    writer.shutdown().await;
}

#[tokio::test]
async fn rejects_an_ephemeral_thread_without_app_server_confirmation() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        confirm_ephemeral_thread_starts: false,
        ..FakeCodexAppServerOptions::default()
    });
    let source = fake_app_server.source();

    let result = CodexThreadWriter::start_on(
        &source,
        CodexHistoryCancellation::new(),
        Path::new("/workspace"),
        ThreadStartOptions { ephemeral: true },
    )
    .await;

    match result {
        Err(CodexError::UnexpectedMessage {
            method: "thread/start",
            description,
        }) => assert_eq!(
            description,
            "the app-server did not confirm an ephemeral thread"
        ),
        Ok((writer, _)) => {
            writer.shutdown().await;
            panic!("the unconfirmed ephemeral thread should be rejected");
        }
        Err(error) => panic!("the thread start returned an unexpected error: {error}"),
    }
}

#[tokio::test]
async fn renames_a_persisted_thread_through_the_public_history_session() {
    let fake_app_server = FakeCodexAppServer::default();
    let source = fake_app_server.source();
    let (writer, _) = CodexThreadWriter::start_on(
        &source,
        CodexHistoryCancellation::new(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the thread should start before it is renamed");
    writer.shutdown().await;

    let mut history = CodexHistorySession::connect(source)
        .await
        .expect("the public history session should connect");
    let ThreadPagePoll::Baseline(page) = history
        .poll_thread_page(&ThreadListOptions::default())
        .await
        .expect("the initial thread page should load")
    else {
        panic!("the first thread-page poll should establish a baseline");
    };
    assert_eq!(page.threads.len(), 1);
    assert_eq!(page.threads[0].name, None);

    history
        .rename_thread("thread-new", "Focused work")
        .await
        .expect("the persisted thread should be renamed");

    let ThreadPagePoll::Changed(page) = history
        .poll_thread_page(&ThreadListOptions::default())
        .await
        .expect("the renamed thread page should load")
    else {
        panic!("renaming should change the tracked thread page");
    };
    assert_eq!(page.threads.len(), 1);
    assert_eq!(page.threads[0].name.as_deref(), Some("Focused work"));

    history.shutdown().await;
}

#[tokio::test]
async fn archives_and_restores_a_persisted_thread_through_the_public_history_session() {
    let fake_app_server = FakeCodexAppServer::default();
    let source = fake_app_server.source();
    let (writer, _) = CodexThreadWriter::start_on(
        &source,
        CodexHistoryCancellation::new(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the thread should start before it is archived");
    writer.shutdown().await;

    let active_threads = ThreadListOptions {
        archived: Some(false),
        ..ThreadListOptions::default()
    };
    let archived_threads = ThreadListOptions {
        archived: Some(true),
        ..ThreadListOptions::default()
    };
    let mut history = CodexHistorySession::connect(source)
        .await
        .expect("the public history session should connect");
    history
        .rename_thread("thread-new", "Focused work")
        .await
        .expect("the test thread should have a stable name");

    let ThreadPagePoll::Baseline(page) = history
        .poll_thread_page(&active_threads)
        .await
        .expect("the active thread page should load")
    else {
        panic!("the first active page should establish a baseline");
    };
    assert_eq!(page.threads.len(), 1);

    history
        .archive_thread("thread-new")
        .await
        .expect("the persisted thread should be archived");
    let ThreadPagePoll::Changed(page) = history
        .poll_thread_page(&active_threads)
        .await
        .expect("the active page should refresh after archiving")
    else {
        panic!("archiving should change the active thread page");
    };
    assert!(page.threads.is_empty());

    history.reset_thread_page_baseline();
    let ThreadPagePoll::Baseline(page) = history
        .poll_thread_page(&archived_threads)
        .await
        .expect("the archived thread page should load")
    else {
        panic!("switching to archived threads should establish a baseline");
    };
    assert_eq!(page.threads.len(), 1);
    assert_eq!(page.threads[0].name.as_deref(), Some("Focused work"));

    history
        .unarchive_thread("thread-new")
        .await
        .expect("the archived thread should be restored");
    let ThreadPagePoll::Changed(page) = history
        .poll_thread_page(&archived_threads)
        .await
        .expect("the archived page should refresh after restoring")
    else {
        panic!("restoring should change the archived thread page");
    };
    assert!(page.threads.is_empty());

    history.reset_thread_page_baseline();
    let ThreadPagePoll::Baseline(page) = history
        .poll_thread_page(&active_threads)
        .await
        .expect("the restored active thread page should load")
    else {
        panic!("switching back to active threads should establish a baseline");
    };
    assert_eq!(page.threads.len(), 1);
    assert_eq!(page.threads[0].name.as_deref(), Some("Focused work"));

    history.shutdown().await;
}

#[tokio::test]
async fn polls_all_thread_pages_as_one_deduplicated_history_snapshot() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        thread_list_page_size: Some(2),
        overlap_thread_list_pages: true,
        ..FakeCodexAppServerOptions::default()
    });
    let source = fake_app_server.source();
    let (writer, _) = CodexThreadWriter::start_on(
        &source,
        CodexHistoryCancellation::new(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the source thread should start");
    writer.shutdown().await;
    for expected_id in ["thread-fork-1", "thread-fork-2"] {
        let (writer, subscription) = CodexThreadWriter::fork_on(
            &source,
            CodexHistoryCancellation::new(),
            "thread-new",
            None,
        )
        .await
        .expect("the source thread should fork");
        assert_eq!(subscription.thread.summary.id, expected_id);
        writer.shutdown().await;
    }

    let mut history = CodexHistorySession::connect(source)
        .await
        .expect("the public history session should connect");
    let ThreadPagePoll::Baseline(page) = history
        .poll_all_threads(&ThreadListOptions {
            archived: Some(false),
            ..ThreadListOptions::default()
        })
        .await
        .expect("all active thread pages should load")
    else {
        panic!("the first complete history poll should establish a baseline");
    };

    assert_eq!(
        page.threads
            .iter()
            .map(|thread| thread.id.as_str())
            .collect::<Vec<_>>(),
        ["thread-new", "thread-fork-1", "thread-fork-2"]
    );
    assert_eq!(page.next_cursor, None);
    history.shutdown().await;
}

#[tokio::test]
async fn rejects_a_repeated_thread_list_cursor_while_polling_complete_history() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        thread_list_page_size: Some(1),
        repeat_thread_list_cursor: true,
        ..FakeCodexAppServerOptions::default()
    });
    let source = fake_app_server.source();
    let (writer, _) = CodexThreadWriter::start_on(
        &source,
        CodexHistoryCancellation::new(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the source thread should start");
    writer.shutdown().await;

    let mut history = CodexHistorySession::connect(source)
        .await
        .expect("the public history session should connect");
    let error = history
        .poll_all_threads(&ThreadListOptions::default())
        .await
        .expect_err("a repeated continuation cursor must be rejected");

    assert!(matches!(
        error,
        ward_codex::CodexError::UnexpectedMessage {
            method: "thread/list",
            description,
        } if description == "the app-server repeated a thread-list pagination cursor"
    ));
    history.shutdown().await;
}

#[tokio::test]
async fn retries_complete_history_from_the_first_page_after_a_lost_connection() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        thread_list_page_size: Some(1),
        lose_first_thread_list_continuation_response: true,
        ..FakeCodexAppServerOptions::default()
    });
    let source = fake_app_server.source();
    let (writer, _) = CodexThreadWriter::start_on(
        &source,
        CodexHistoryCancellation::new(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the source thread should start");
    writer.shutdown().await;
    let (writer, subscription) =
        CodexThreadWriter::fork_on(&source, CodexHistoryCancellation::new(), "thread-new", None)
            .await
            .expect("the source thread should fork");
    assert_eq!(subscription.thread.summary.id, "thread-fork-1");
    writer.shutdown().await;

    let mut history = CodexHistorySession::connect(source)
        .await
        .expect("the public history session should connect");
    let original_options = ThreadListOptions {
        cursor: Some("thread-list-offset-0".to_owned()),
        limit: Some(1),
        archived: Some(false),
    };
    let ThreadPagePoll::Baseline(page) = history
        .poll_all_threads(&original_options)
        .await
        .expect("the complete history should reconnect and retry")
    else {
        panic!("the retried complete history should establish a baseline");
    };

    assert_eq!(
        page.threads
            .iter()
            .map(|thread| thread.id.as_str())
            .collect::<Vec<_>>(),
        ["thread-new", "thread-fork-1"]
    );
    assert_eq!(page.next_cursor, None);
    history.shutdown().await;

    let requests = fake_app_server.thread_list_requests();
    let [
        first_page,
        lost_continuation,
        retried_first_page,
        retried_continuation,
    ] = requests.as_slice()
    else {
        panic!("the complete history retry should issue two requests per connection");
    };
    assert_eq!(first_page.connection_id, lost_continuation.connection_id);
    assert_ne!(first_page.connection_id, retried_first_page.connection_id);
    assert_eq!(
        retried_first_page.connection_id,
        retried_continuation.connection_id
    );
    assert_eq!(
        [
            first_page.cursor.as_deref(),
            lost_continuation.cursor.as_deref(),
            retried_first_page.cursor.as_deref(),
            retried_continuation.cursor.as_deref(),
        ],
        [
            Some("thread-list-offset-0"),
            Some("thread-list-offset-1"),
            Some("thread-list-offset-0"),
            Some("thread-list-offset-1"),
        ]
    );
    for request in requests {
        assert_eq!(request.limit, original_options.limit);
        assert_eq!(request.archived, original_options.archived);
    }
}

#[tokio::test]
async fn forks_through_a_completed_turn_and_continues_the_branch() {
    let fake_app_server = FakeCodexAppServer::default();
    let source = fake_app_server.source();
    let (mut original_writer, _) = CodexThreadWriter::start_on(
        &source,
        CodexHistoryCancellation::new(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the original thread should start");
    for prompt in [
        "Explore the original approach",
        "Refine the original approach",
        "Finish the original approach",
    ] {
        original_writer
            .begin_text_turn(prompt, TurnOptions::default())
            .await
            .expect("the original turn should start");
        let original_outcome = finish_fake_turn(&mut original_writer).await;
        assert_eq!(original_outcome.answer, "Done.");
    }
    original_writer.shutdown().await;

    let mut history = CodexHistorySession::connect(source.clone())
        .await
        .expect("the public history session should connect");
    history
        .rename_thread("thread-new", "Original approach")
        .await
        .expect("the original thread should have a stable name");
    let (mut fork_writer, fork_subscription) = CodexThreadWriter::fork_on(
        &source,
        CodexHistoryCancellation::new(),
        "thread-new",
        Some("live-turn-1"),
    )
    .await
    .expect("the thread should fork through the selected completed turn");
    let fork = fork_subscription.thread;
    assert_eq!(fork.summary.id, "thread-fork-1");
    assert_eq!(fork.summary.cwd, Path::new("/workspace"));
    assert_eq!(fork.turns.len(), 1);
    assert_eq!(fork.turns[0].status, TurnStatus::Completed);
    let [
        ThreadItem::UserMessage { content, .. },
        ThreadItem::AgentMessage { text, .. },
    ] = fork.turns[0].items.as_slice()
    else {
        panic!("the fork should preserve the source turn's complete message history");
    };
    assert_eq!(
        content.as_slice(),
        [UserInput::Text("Explore the original approach".to_owned())]
    );
    assert_eq!(text, "Done.");

    let ThreadPagePoll::Baseline(page) = history
        .poll_thread_page(&ThreadListOptions {
            archived: Some(false),
            ..ThreadListOptions::default()
        })
        .await
        .expect("the active thread page should include both branches")
    else {
        panic!("the first active page should establish a baseline");
    };
    let mut thread_ids = page
        .threads
        .iter()
        .map(|thread| thread.id.as_str())
        .collect::<Vec<_>>();
    thread_ids.sort_unstable();
    assert_eq!(thread_ids, ["thread-fork-1", "thread-new"]);

    assert_eq!(fork_writer.thread_id(), "thread-fork-1");
    fork_writer
        .begin_text_turn("Try a different direction", TurnOptions::default())
        .await
        .expect("the fork should accept a new turn");
    let fork_outcome = finish_fake_turn(&mut fork_writer).await;
    assert_eq!(fork_outcome.answer, "Done.");
    fork_writer.shutdown().await;

    let ThreadPoll::Baseline(original) = history
        .poll_thread("thread-new")
        .await
        .expect("the original thread should remain readable")
    else {
        panic!("reading the original thread should establish a baseline");
    };
    assert_eq!(original.summary.name.as_deref(), Some("Original approach"));
    assert_eq!(original.turns.len(), 3);
    let ThreadPoll::Baseline(fork) = history
        .poll_thread("thread-fork-1")
        .await
        .expect("the continued fork should remain readable")
    else {
        panic!("switching to the fork should establish a baseline");
    };
    assert_eq!(fork.turns.len(), 2);

    history.shutdown().await;
}

#[tokio::test]
async fn does_not_retry_a_fork_after_its_applied_response_is_lost() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        lose_first_fork_response: true,
        ..FakeCodexAppServerOptions::default()
    });
    let source = fake_app_server.source();
    let (writer, _) = CodexThreadWriter::start_on(
        &source,
        CodexHistoryCancellation::new(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the source thread should start");
    writer.shutdown().await;

    match CodexThreadWriter::fork_on(&source, CodexHistoryCancellation::new(), "thread-new", None)
        .await
    {
        Err(_) => {}
        Ok((writer, _)) => {
            writer.shutdown().await;
            panic!("a lost fork response must not be retried");
        }
    }

    let mut history = CodexHistorySession::connect(source)
        .await
        .expect("the history session should reconnect independently");
    let ThreadPagePoll::Baseline(page) = history
        .poll_thread_page(&ThreadListOptions::default())
        .await
        .expect("the applied fork should be visible")
    else {
        panic!("the first thread page should establish a baseline");
    };
    assert_eq!(page.threads.len(), 2);
    history.shutdown().await;
}

struct FakeTurnOutcome {
    command_status: Option<ActivityStatus>,
    streamed_answer: String,
    answer: String,
    turn_id: String,
}

async fn start_fake_turn(turn_scenario: FakeTurnScenario, prompt: &str) -> CodexThreadWriter {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        turn_scenario,
        ..FakeCodexAppServerOptions::default()
    });
    let source = fake_app_server.source();
    let (mut writer, _) = CodexThreadWriter::start_on(
        &source,
        CodexHistoryCancellation::new(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the public writer seam should start a thread");
    let started = writer
        .begin_text_turn(prompt, TurnOptions::default())
        .await
        .expect("the turn should start");
    assert!(matches!(
        started,
        ThreadStreamEvent::TurnStarted { ref turn, .. } if turn.id == "live-turn-1"
    ));
    writer
}

async fn next_pending_interaction(writer: &mut CodexThreadWriter) -> PendingInteraction {
    loop {
        match next_fake_turn_event(writer).await {
            ThreadStreamEvent::PendingInteractionsUpdated {
                thread_id,
                interactions,
            } => {
                assert_eq!(thread_id, "thread-new");
                let [interaction] = interactions.as_slice() else {
                    panic!("the turn should have exactly one pending interaction");
                };
                return interaction.clone();
            }
            ThreadStreamEvent::TurnCompleted { .. } => {
                panic!("the turn must wait for the pending interaction");
            }
            _ => {}
        }
    }
}

async fn resolve_pending_interaction(
    writer: &mut CodexThreadWriter,
    response: InteractionResponse,
) {
    let cleared = writer
        .resolve_interaction(response)
        .await
        .expect("the pending interaction response should resolve");
    assert!(matches!(
        cleared,
        ThreadStreamEvent::PendingInteractionsUpdated { interactions, .. }
            if interactions.is_empty()
    ));
}

async fn finish_fake_turn(writer: &mut CodexThreadWriter) -> FakeTurnOutcome {
    let mut command_status = None;
    let mut streamed_answer = String::new();
    let mut answer = None;
    loop {
        let event = next_fake_turn_event(writer).await;
        match &event {
            ThreadStreamEvent::AgentMessageDelta { delta, .. } => {
                streamed_answer.push_str(delta);
            }
            ThreadStreamEvent::ItemCompleted {
                item: ThreadItem::Activity(activity),
                ..
            } if activity.kind == ActivityKind::CommandExecution => {
                command_status = Some(activity.status.clone());
            }
            ThreadStreamEvent::ItemCompleted {
                item: ThreadItem::AgentMessage { text, .. },
                ..
            } => answer = Some(text.clone()),
            _ => {}
        }
        if let ThreadStreamEvent::TurnCompleted { turn, .. } = event {
            return FakeTurnOutcome {
                command_status,
                streamed_answer,
                answer: answer.expect("the agent should answer before the turn completes"),
                turn_id: turn.id,
            };
        }
    }
}

async fn next_fake_turn_event(writer: &mut CodexThreadWriter) -> ThreadStreamEvent {
    tokio::time::timeout(Duration::from_secs(5), writer.next_subscription_event())
        .await
        .expect("the fake turn should not remain blocked")
        .expect("the fake turn should keep streaming")
}

#[tokio::test]
async fn accepts_a_command_approval_and_completes_the_same_turn_through_the_public_writer_seam() {
    let mut writer = start_fake_turn(FakeTurnScenario::RequestCommandApproval, "Run pwd").await;
    let interaction = next_pending_interaction(&mut writer).await;
    assert_eq!(interaction.thread_id, "thread-new");
    assert_eq!(interaction.turn_id.as_deref(), Some("live-turn-1"));
    assert_eq!(interaction.item_id.as_deref(), Some("live-command-1"));
    assert_eq!(interaction.kind, PendingInteractionKind::CommandApproval);
    assert_eq!(interaction.command.as_deref(), Some("pwd"));
    assert_eq!(
        interaction.working_directory.as_deref(),
        Some(Path::new("/workspace"))
    );
    assert_eq!(interaction.reason.as_deref(), Some("Verify the workspace"));

    resolve_pending_interaction(
        &mut writer,
        InteractionResponse {
            interaction_id: interaction.id,
            body: InteractionResponseBody::Decision(InteractionDecision::Accept),
        },
    )
    .await;
    let outcome = finish_fake_turn(&mut writer).await;
    assert_eq!(outcome.command_status, Some(ActivityStatus::Completed));
    assert_eq!(outcome.streamed_answer, "Command approved.");
    assert_eq!(outcome.answer, "Command approved.");
    assert_eq!(outcome.turn_id, "live-turn-1");
    writer.shutdown().await;
}

#[tokio::test]
async fn declines_a_command_approval_and_completes_the_same_turn_through_the_public_writer_seam() {
    let mut writer = start_fake_turn(FakeTurnScenario::RequestCommandApproval, "Run pwd").await;
    let interaction = next_pending_interaction(&mut writer).await;
    resolve_pending_interaction(
        &mut writer,
        InteractionResponse {
            interaction_id: interaction.id,
            body: InteractionResponseBody::Decision(InteractionDecision::Decline),
        },
    )
    .await;
    let outcome = finish_fake_turn(&mut writer).await;
    assert_eq!(outcome.command_status, Some(ActivityStatus::Declined));
    assert_eq!(outcome.streamed_answer, "Command declined.");
    assert_eq!(outcome.answer, "Command declined.");
    assert_eq!(outcome.turn_id, "live-turn-1");
    writer.shutdown().await;
}

#[tokio::test]
async fn answers_structured_user_input_and_completes_the_same_turn_through_the_public_writer_seam()
{
    let mut writer = start_fake_turn(FakeTurnScenario::RequestUserInput, "Plan the change").await;
    let interaction = next_pending_interaction(&mut writer).await;
    assert_eq!(interaction.thread_id, "thread-new");
    assert_eq!(interaction.turn_id.as_deref(), Some("live-turn-1"));
    assert_eq!(interaction.item_id.as_deref(), Some("live-tool-1"));
    assert_eq!(interaction.kind, PendingInteractionKind::UserInput);
    assert!(interaction.user_input_is_blocking);
    assert!(interaction.available_decisions.is_empty());
    let [scope, note] = interaction.questions.as_slice() else {
        panic!("the interaction should contain the two advertised questions");
    };
    assert_eq!(scope.id, "scope");
    assert_eq!(scope.header, "Scope");
    assert_eq!(scope.prompt, "Which scope should be used?");
    assert_eq!(scope.options.len(), 2);
    assert_eq!(scope.options[0].label, "Current");
    assert_eq!(scope.options[0].description, "Only the current turn");
    assert_eq!(scope.options[1].label, "All");
    assert_eq!(scope.options[1].description, "The whole conversation");
    assert!(scope.allows_other);
    assert!(!scope.secret);
    assert_eq!(note.id, "note");
    assert_eq!(note.header, "Note");
    assert_eq!(note.prompt, "What should Codex remember?");
    assert!(note.options.is_empty());
    assert!(!note.allows_other);
    assert!(!note.secret);

    resolve_pending_interaction(
        &mut writer,
        InteractionResponse {
            interaction_id: interaction.id,
            body: InteractionResponseBody::UserInput(vec![
                InteractionAnswer {
                    question_id: "note".to_owned(),
                    answers: vec!["Keep changes focused".to_owned()],
                },
                InteractionAnswer {
                    question_id: "scope".to_owned(),
                    answers: vec!["Current".to_owned()],
                },
            ]),
        },
    )
    .await;
    let outcome = finish_fake_turn(&mut writer).await;

    assert_eq!(
        outcome.streamed_answer,
        "Scope: Current; note: Keep changes focused."
    );
    assert_eq!(
        outcome.answer,
        "Scope: Current; note: Keep changes focused."
    );
    assert_eq!(outcome.turn_id, "live-turn-1");
    writer.shutdown().await;
}

#[tokio::test]
async fn steers_the_expected_active_turn_through_the_public_writer_seam() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        renumber_persisted_first_turn: true,
        turn_scenario: FakeTurnScenario::WaitForGuidance,
        ..FakeCodexAppServerOptions::default()
    });
    let source = fake_app_server.source();
    let (mut writer, _) = CodexThreadWriter::start_on(
        &source,
        CodexHistoryCancellation::new(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the public writer seam should start a thread");
    let started = writer
        .begin_text_turn("Implement the change", TurnOptions::default())
        .await
        .expect("the turn should start");
    let ThreadStreamEvent::TurnStarted { turn, .. } = started else {
        panic!("the writer should return the started turn");
    };

    writer
        .steer_text_turn(&turn.id, "Use the existing test seam")
        .await
        .expect("the active turn should accept guidance");

    let mut guidance_seen = false;
    loop {
        let event = writer
            .next_subscription_event()
            .await
            .expect("the guided turn should keep streaming");
        if let ThreadStreamEvent::ItemCompleted {
            item: ThreadItem::UserMessage { content, .. },
            ..
        } = &event
            && content == &[UserInput::Text("Use the existing test seam".to_owned())]
        {
            guidance_seen = true;
        }
        if matches!(event, ThreadStreamEvent::TurnCompleted { .. }) {
            break;
        }
    }

    assert!(guidance_seen);
    writer.shutdown().await;

    let mut history = CodexHistorySession::connect(source)
        .await
        .expect("the persisted history session should connect");
    let ThreadPoll::Baseline(thread) = history
        .poll_thread("thread-new")
        .await
        .expect("the guided thread should be persisted")
    else {
        panic!("the first persisted thread read should establish a baseline");
    };
    assert_eq!(thread.turns.len(), 1);
    assert_eq!(thread.turns[0].id, "persisted-turn-1");
    assert_eq!(thread.turns[0].status, TurnStatus::Completed);
    let messages = thread.turns[0]
        .items
        .iter()
        .filter_map(|item| match item {
            ThreadItem::UserMessage { content, .. } => {
                content.iter().find_map(|input| match input {
                    UserInput::Text(text) => Some(text.as_str()),
                    _ => None,
                })
            }
            ThreadItem::AgentMessage { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        messages,
        [
            "Implement the change",
            "Use the existing test seam",
            "Adjusted."
        ]
    );
    history.shutdown().await;
}

#[tokio::test]
async fn refreshes_an_active_turn_without_reloading_the_complete_thread() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        turn_scenario: FakeTurnScenario::WaitForGuidance,
        ..FakeCodexAppServerOptions::default()
    });
    let source = fake_app_server.source();
    let cancellation = CodexHistoryCancellation::new();
    let (mut writer, _) = CodexThreadWriter::start_on(
        &source,
        cancellation.clone(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the fake writer should start a thread");
    let started = writer
        .begin_text_turn("Wait for guidance", TurnOptions::default())
        .await
        .expect("the fake turn should start");
    let ThreadStreamEvent::TurnStarted { turn, .. } = started else {
        panic!("the fake writer should report the active turn");
    };
    let mut history = CodexHistorySession::connect_with_cancellation(source, cancellation)
        .await
        .expect("the persisted history session should connect");
    let ThreadPoll::Baseline(thread) = history
        .poll_thread("thread-new")
        .await
        .expect("the complete read should establish a baseline")
    else {
        panic!("the first complete read should establish a baseline");
    };
    assert_eq!(thread.turns[0].status, TurnStatus::InProgress);

    assert_eq!(
        history
            .poll_active_thread("thread-new")
            .await
            .expect("the active anchor should load"),
        ThreadPoll::Unchanged
    );
    writer
        .steer_text_turn(&turn.id, "Finish now")
        .await
        .expect("guidance should complete the fake turn");
    let ThreadPoll::Changed(completed) = history
        .poll_active_thread("thread-new")
        .await
        .expect("the anchored turn should refresh")
    else {
        panic!("the terminal turn should change the merged snapshot");
    };

    assert_eq!(completed.turns[0].status, TurnStatus::Completed);
    assert_eq!(fake_app_server.thread_read_request_count(), 1);
    let requests = fake_app_server.thread_turns_list_requests();
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0].cursor, None);
    assert_eq!(requests[0].sort_direction.as_deref(), Some("desc"));
    assert!(requests[1].cursor.is_some());
    assert_eq!(requests[1].sort_direction.as_deref(), Some("asc"));

    history.shutdown().await;
    writer.shutdown().await;
}

#[tokio::test]
async fn falls_back_once_when_turn_pagination_is_unsupported() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        support_thread_turns_list: false,
        turn_scenario: FakeTurnScenario::WaitForGuidance,
        ..FakeCodexAppServerOptions::default()
    });
    let source = fake_app_server.source();
    let cancellation = CodexHistoryCancellation::new();
    let (mut writer, _) = CodexThreadWriter::start_on(
        &source,
        cancellation.clone(),
        Path::new("/workspace"),
        ThreadStartOptions::default(),
    )
    .await
    .expect("the fake writer should start a thread");
    writer
        .begin_text_turn("Remain active", TurnOptions::default())
        .await
        .expect("the fake turn should start");
    let mut history = CodexHistorySession::connect_with_cancellation(source, cancellation)
        .await
        .expect("the persisted history session should connect");
    history
        .poll_thread("thread-new")
        .await
        .expect("the complete read should establish a baseline");

    assert_eq!(
        history
            .poll_active_thread("thread-new")
            .await
            .expect("the missing method should fall back to a complete read"),
        ThreadPoll::Unchanged
    );
    assert_eq!(
        history
            .poll_active_thread("thread-new")
            .await
            .expect("later polls should use the remembered fallback"),
        ThreadPoll::Unchanged
    );

    assert_eq!(fake_app_server.thread_turns_list_requests().len(), 1);
    assert_eq!(fake_app_server.thread_read_request_count(), 3);
    history.shutdown().await;
    writer.shutdown().await;
}
