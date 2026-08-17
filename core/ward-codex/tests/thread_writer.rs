// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use ward_codex::{
    CodexError, CodexHistoryCancellation, CodexHistorySession, CodexThreadWriter, ThreadItem,
    ThreadPoll, ThreadRuntimeStatus, ThreadStartOptions, ThreadStreamEvent, TurnOptions,
    TurnStatus, UserInput,
};
use ward_codex_test_support::{FakeCodexAppServer, FakeCodexAppServerOptions};

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
    assert_eq!(subscription.thread.summary.id, "thread-new");
    assert_eq!(subscription.thread.summary.cwd, Path::new("/workspace"));
    assert_eq!(subscription.runtime_status, ThreadRuntimeStatus::Idle);

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
async fn steers_the_expected_active_turn_through_the_public_writer_seam() {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        renumber_persisted_first_turn: true,
        wait_for_turn_steer: true,
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
