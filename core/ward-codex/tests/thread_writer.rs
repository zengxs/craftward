// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;
use std::time::Duration;

use ward_codex::{
    ActivityKind, ActivityStatus, CodexError, CodexHistoryCancellation, CodexHistorySession,
    CodexThreadWriter, InteractionDecision, InteractionId, InteractionResponse,
    InteractionResponseBody, PendingInteraction, PendingInteractionKind, ThreadItem, ThreadPoll,
    ThreadRuntimeStatus, ThreadStartOptions, ThreadStreamEvent, TurnOptions, TurnStatus, UserInput,
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

struct CommandApprovalOutcome {
    command_status: ActivityStatus,
    streamed_answer: String,
    answer: String,
    turn_id: String,
}

async fn start_command_approval_turn() -> CodexThreadWriter {
    let fake_app_server = FakeCodexAppServer::new(FakeCodexAppServerOptions {
        turn_scenario: FakeTurnScenario::RequestCommandApproval,
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
        .begin_text_turn("Run pwd", TurnOptions::default())
        .await
        .expect("the turn should start");
    assert!(matches!(
        started,
        ThreadStreamEvent::TurnStarted { ref turn, .. } if turn.id == "live-turn-1"
    ));
    writer
}

async fn next_command_approval(writer: &mut CodexThreadWriter) -> PendingInteraction {
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
                panic!("the turn must wait for command approval");
            }
            _ => {}
        }
    }
}

async fn resolve_command_approval(
    writer: &mut CodexThreadWriter,
    interaction_id: InteractionId,
    decision: InteractionDecision,
) {
    let cleared = writer
        .resolve_interaction(InteractionResponse {
            interaction_id,
            body: InteractionResponseBody::Decision(decision),
        })
        .await
        .expect("the command approval decision should resolve");
    assert!(matches!(
        cleared,
        ThreadStreamEvent::PendingInteractionsUpdated { interactions, .. }
            if interactions.is_empty()
    ));
}

async fn finish_command_approval_turn(writer: &mut CodexThreadWriter) -> CommandApprovalOutcome {
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
            return CommandApprovalOutcome {
                command_status: command_status
                    .expect("the command should finish before the turn completes"),
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
    let mut writer = start_command_approval_turn().await;
    let interaction = next_command_approval(&mut writer).await;
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

    resolve_command_approval(&mut writer, interaction.id, InteractionDecision::Accept).await;
    let outcome = finish_command_approval_turn(&mut writer).await;
    assert_eq!(outcome.command_status, ActivityStatus::Completed);
    assert_eq!(outcome.streamed_answer, "Command approved.");
    assert_eq!(outcome.answer, "Command approved.");
    assert_eq!(outcome.turn_id, "live-turn-1");
    writer.shutdown().await;
}

#[tokio::test]
async fn declines_a_command_approval_and_completes_the_same_turn_through_the_public_writer_seam() {
    let mut writer = start_command_approval_turn().await;
    let interaction = next_command_approval(&mut writer).await;
    resolve_command_approval(&mut writer, interaction.id, InteractionDecision::Decline).await;
    let outcome = finish_command_approval_turn(&mut writer).await;
    assert_eq!(outcome.command_status, ActivityStatus::Declined);
    assert_eq!(outcome.streamed_answer, "Command declined.");
    assert_eq!(outcome.answer, "Command declined.");
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
