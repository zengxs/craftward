// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;
use std::time::Duration;

use ward_codex::{
    ActivityKind, ActivityStatus, CodexError, CodexHistoryCancellation, CodexHistorySession,
    CodexThreadWriter, InteractionAnswer, InteractionDecision, InteractionResponse,
    InteractionResponseBody, PendingInteraction, PendingInteractionKind, ThreadItem,
    ThreadListOptions, ThreadPagePoll, ThreadPoll, ThreadRuntimeStatus, ThreadStartOptions,
    ThreadStreamEvent, TurnOptions, TurnStatus, UserInput,
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
