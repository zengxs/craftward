// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;

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
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        initial_thread_read_failures: 1,
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
    captured.lock().unwrap().events.clear();

    state.select_thread().await;

    assert_ne!(
        state.poll_conversation(&thread_id, &sink).await,
        ConversationPollOutcome::Failed
    );
    assert!(
        captured
            .lock()
            .unwrap()
            .events
            .iter()
            .all(|event| event.kind != wire::HistoryEventKind::ConversationError as i32)
    );

    state.shutdown().await;
}

#[tokio::test]
async fn suppresses_repeated_identical_errors_for_each_target() {
    let captured = Mutex::new(CapturedEvent::default());
    let sink = event_sink(&captured);
    let mut state = ObserverState::new(
        PathBuf::from("/craftward-tests/missing-codex"),
        CodexHistoryCancellation::new(),
    );

    assert_eq!(
        state.poll_threads(None, &sink).await,
        ThreadPagePollOutcome::Failed
    );
    assert_eq!(
        state.poll_threads(None, &sink).await,
        ThreadPagePollOutcome::Failed
    );
    state.select_thread().await;
    assert_eq!(
        state.poll_conversation("thread-1", &sink).await,
        ConversationPollOutcome::Failed
    );
    assert_eq!(
        state.poll_conversation("thread-1", &sink).await,
        ConversationPollOutcome::Failed
    );

    let captured = captured.lock().unwrap();
    assert_eq!(captured.events.len(), 2);
    assert_eq!(
        captured.events.last().unwrap().kind,
        wire::HistoryEventKind::ConversationError as i32
    );
}
