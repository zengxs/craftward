// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use ward_codex::{
    CodexError, CodexHistoryCancellation, CodexThreadWriter, ThreadRuntimeStatus,
    ThreadStartOptions,
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
