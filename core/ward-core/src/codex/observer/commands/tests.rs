// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use tokio::sync::mpsc;
use ward_codex::{
    InteractionDecision, InteractionId, InteractionResponse, InteractionResponseBody, TurnOptions,
};

use super::*;
use crate::codex::observer::COMMAND_QUEUE_CAPACITY;

fn turn_request(thread_id: &str, prompt: &str) -> TurnRequest {
    TurnRequest {
        thread_id: thread_id.to_owned(),
        prompt: prompt.to_owned(),
        options: TurnOptions::default(),
    }
}

#[test]
fn coalesces_commands_and_prioritizes_stop() {
    let (sender, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    sender
        .try_send(ObserverCommand::Watch("thread-2".to_owned()))
        .unwrap();
    sender.try_send(ObserverCommand::Refresh).unwrap();
    sender
        .try_send(ObserverCommand::AcquireWrite("thread-2".to_owned()))
        .unwrap();
    sender
        .try_send(ObserverCommand::StartTurn(turn_request(
            "thread-2", "Continue",
        )))
        .unwrap();
    assert_eq!(
        drain_commands(ObserverCommand::Watch("thread-1".to_owned()), &mut receiver),
        DrainedCommands::Update(CommandUpdate {
            watched_thread: Some("thread-2".to_owned()),
            refresh: true,
            write_access: Some(WriteAccessRequest::Acquire("thread-2".to_owned())),
            turn: Some(turn_request("thread-2", "Continue")),
            controls: vec![],
        })
    );

    sender.try_send(ObserverCommand::Stop).unwrap();
    assert_eq!(
        drain_commands(ObserverCommand::Refresh, &mut receiver),
        DrainedCommands::Stop
    );
}

#[test]
fn keeps_only_the_latest_write_access_intent() {
    let (sender, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    sender
        .try_send(ObserverCommand::ReleaseWrite("thread-1".to_owned()))
        .unwrap();

    assert_eq!(
        drain_commands(
            ObserverCommand::AcquireWrite("thread-1".to_owned()),
            &mut receiver,
        ),
        DrainedCommands::Update(CommandUpdate {
            write_access: Some(WriteAccessRequest::Release("thread-1".to_owned())),
            ..CommandUpdate::default()
        })
    );
}

#[test]
fn preserves_immediate_turn_controls_in_arrival_order() {
    let (sender, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    sender
        .try_send(ObserverCommand::ResolveInteraction(InteractionResponse {
            interaction_id: InteractionId::new(8).unwrap(),
            body: InteractionResponseBody::Decision(InteractionDecision::Decline),
        }))
        .unwrap();

    assert_eq!(
        drain_commands(
            ObserverCommand::InterruptTurn("thread-1".to_owned()),
            &mut receiver,
        ),
        DrainedCommands::Update(CommandUpdate {
            controls: vec![
                ThreadControlRequest::Interrupt("thread-1".to_owned()),
                ThreadControlRequest::ResolveInteraction(InteractionResponse {
                    interaction_id: InteractionId::new(8).unwrap(),
                    body: InteractionResponseBody::Decision(InteractionDecision::Decline),
                }),
            ],
            ..CommandUpdate::default()
        })
    );
}

#[test]
fn merges_deferred_updates_without_replacing_the_reserved_turn() {
    let mut deferred = CommandUpdate {
        watched_thread: Some("thread-1".to_owned()),
        refresh: false,
        write_access: Some(WriteAccessRequest::Acquire("thread-1".to_owned())),
        turn: Some(turn_request("thread-1", "First")),
        controls: vec![],
    };

    deferred.merge(CommandUpdate {
        watched_thread: Some("thread-2".to_owned()),
        refresh: true,
        write_access: Some(WriteAccessRequest::Release("thread-1".to_owned())),
        turn: Some(turn_request("thread-2", "Second")),
        controls: vec![],
    });

    assert_eq!(
        deferred,
        CommandUpdate {
            watched_thread: Some("thread-2".to_owned()),
            refresh: true,
            write_access: Some(WriteAccessRequest::Release("thread-1".to_owned())),
            turn: Some(turn_request("thread-1", "First")),
            controls: vec![],
        }
    );
}

#[test]
fn processes_control_commands_before_a_reserved_turn() {
    let (sender, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    sender
        .try_send(ObserverCommand::Watch("thread-2".to_owned()))
        .unwrap();
    sender.try_send(ObserverCommand::Refresh).unwrap();
    let mut update = CommandUpdate {
        turn: Some(turn_request("thread-1", "Continue")),
        ..CommandUpdate::default()
    };

    let first = receiver.try_recv().unwrap();
    assert!(merge_command(&mut update, first));
    while let Ok(command) = receiver.try_recv() {
        assert!(merge_command(&mut update, command));
    }

    assert_eq!(update.watched_thread.as_deref(), Some("thread-2"));
    assert!(update.refresh);
    assert_eq!(update.turn.unwrap().thread_id, "thread-1");
}
