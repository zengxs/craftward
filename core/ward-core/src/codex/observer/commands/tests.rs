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

fn steer_request(thread_id: &str, turn_id: &str, prompt: &str) -> TurnSteerRequest {
    TurnSteerRequest {
        thread_id: thread_id.to_owned(),
        expected_turn_id: turn_id.to_owned(),
        prompt: prompt.to_owned(),
    }
}

fn thread_start_request(working_directory: &str) -> ThreadStartRequest {
    ThreadStartRequest {
        working_directory: working_directory.into(),
    }
}

fn thread_rename_request(thread_id: &str, name: &str) -> ThreadRenameRequest {
    ThreadRenameRequest {
        thread_id: thread_id.to_owned(),
        name: name.to_owned(),
    }
}

fn thread_fork_request(thread_id: &str) -> ThreadForkRequest {
    ThreadForkRequest {
        thread_id: thread_id.to_owned(),
        last_turn_id: "turn-1".to_owned(),
    }
}

#[test]
fn reserves_only_the_first_thread_fork() {
    let (sender, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    sender
        .try_send(ObserverCommand::ForkThread(thread_fork_request("thread-2")))
        .unwrap();

    assert_eq!(
        drain_commands(
            ObserverCommand::ForkThread(thread_fork_request("thread-1")),
            &mut receiver,
        ),
        DrainedCommands::Update(CommandUpdate {
            thread_fork: Some(Box::new(thread_fork_request("thread-1"))),
            ..CommandUpdate::default()
        })
    );
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
            thread_list_scope: None,
            refresh: true,
            write_access: Some(WriteAccessRequest::Acquire("thread-2".to_owned())),
            thread_rename: None,
            thread_fork: None,
            thread_lifecycle: vec![],
            thread_start: None,
            turn: Some(Box::new(turn_request("thread-2", "Continue"))),
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
fn reserves_only_the_first_thread_start() {
    let (sender, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    sender
        .try_send(ObserverCommand::StartThread(thread_start_request(
            "/workspace/two",
        )))
        .unwrap();

    assert_eq!(
        drain_commands(
            ObserverCommand::StartThread(thread_start_request("/workspace/one")),
            &mut receiver,
        ),
        DrainedCommands::Update(CommandUpdate {
            thread_start: Some(Box::new(thread_start_request("/workspace/one"))),
            ..CommandUpdate::default()
        })
    );
}

#[test]
fn recognizes_an_update_with_exactly_one_exclusive_operation() {
    let start = CommandUpdate {
        thread_start: Some(Box::new(thread_start_request("/workspace"))),
        ..CommandUpdate::default()
    };
    let turn = CommandUpdate {
        turn: Some(Box::new(turn_request("thread-1", "Continue"))),
        ..CommandUpdate::default()
    };
    let fork = CommandUpdate {
        thread_fork: Some(Box::new(thread_fork_request("thread-1"))),
        ..CommandUpdate::default()
    };
    let both = CommandUpdate {
        thread_start: Some(Box::new(thread_start_request("/workspace"))),
        turn: Some(Box::new(turn_request("thread-1", "Continue"))),
        ..CommandUpdate::default()
    };

    assert!(start.is_exclusive_operation_only());
    assert!(turn.is_exclusive_operation_only());
    assert!(fork.is_exclusive_operation_only());
    assert!(!both.is_exclusive_operation_only());
    assert!(!CommandUpdate::default().is_exclusive_operation_only());
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
fn keeps_only_the_latest_thread_rename() {
    let (sender, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    sender
        .try_send(ObserverCommand::RenameThread(thread_rename_request(
            "thread-1",
            "Focused work",
        )))
        .unwrap();

    assert_eq!(
        drain_commands(
            ObserverCommand::RenameThread(thread_rename_request("thread-1", "First name")),
            &mut receiver,
        ),
        DrainedCommands::Update(CommandUpdate {
            thread_rename: Some(Box::new(thread_rename_request("thread-1", "Focused work"))),
            ..CommandUpdate::default()
        })
    );
}

#[test]
fn preserves_thread_lifecycle_changes_in_arrival_order() {
    let (sender, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    sender
        .try_send(ObserverCommand::ChangeThreadLifecycle(
            ThreadLifecycleRequest {
                thread_id: "thread-1".to_owned(),
                action: ThreadLifecycleAction::Restore,
            },
        ))
        .unwrap();

    assert_eq!(
        drain_commands(
            ObserverCommand::ChangeThreadLifecycle(ThreadLifecycleRequest {
                thread_id: "thread-1".to_owned(),
                action: ThreadLifecycleAction::Archive,
            }),
            &mut receiver,
        ),
        DrainedCommands::Update(CommandUpdate {
            thread_lifecycle: vec![
                ThreadLifecycleRequest {
                    thread_id: "thread-1".to_owned(),
                    action: ThreadLifecycleAction::Archive,
                },
                ThreadLifecycleRequest {
                    thread_id: "thread-1".to_owned(),
                    action: ThreadLifecycleAction::Restore,
                },
            ],
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
    sender
        .try_send(ObserverCommand::SteerTurn(steer_request(
            "thread-1",
            "turn-2",
            "Use the existing seam",
        )))
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
                ThreadControlRequest::Steer(steer_request(
                    "thread-1",
                    "turn-2",
                    "Use the existing seam",
                )),
            ],
            ..CommandUpdate::default()
        })
    );
}

#[test]
fn merges_deferred_updates_without_replacing_the_reserved_turn() {
    let mut deferred = CommandUpdate {
        watched_thread: Some("thread-1".to_owned()),
        thread_list_scope: None,
        refresh: false,
        write_access: Some(WriteAccessRequest::Acquire("thread-1".to_owned())),
        thread_rename: None,
        thread_fork: None,
        thread_lifecycle: vec![],
        thread_start: Some(Box::new(thread_start_request("/workspace/one"))),
        turn: Some(Box::new(turn_request("thread-1", "First"))),
        controls: vec![],
    };

    deferred.merge(CommandUpdate {
        watched_thread: Some("thread-2".to_owned()),
        thread_list_scope: None,
        refresh: true,
        write_access: Some(WriteAccessRequest::Release("thread-1".to_owned())),
        thread_rename: None,
        thread_fork: None,
        thread_lifecycle: vec![],
        thread_start: Some(Box::new(thread_start_request("/workspace/two"))),
        turn: Some(Box::new(turn_request("thread-2", "Second"))),
        controls: vec![],
    });

    assert_eq!(
        deferred,
        CommandUpdate {
            watched_thread: Some("thread-2".to_owned()),
            thread_list_scope: None,
            refresh: true,
            write_access: Some(WriteAccessRequest::Release("thread-1".to_owned())),
            thread_rename: None,
            thread_fork: None,
            thread_lifecycle: vec![],
            thread_start: Some(Box::new(thread_start_request("/workspace/one"))),
            turn: Some(Box::new(turn_request("thread-1", "First"))),
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
        turn: Some(Box::new(turn_request("thread-1", "Continue"))),
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
