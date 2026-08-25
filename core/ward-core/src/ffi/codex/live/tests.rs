// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use ward_codex::{Activity, ActivityKind, AgentMessagePhase, ThreadSummary, UserInput};

use super::*;

fn thread() -> Thread {
    Thread {
        summary: ThreadSummary {
            id: "thread-1".to_owned(),
            name: Some("Example".to_owned()),
            preview: "Preview".to_owned(),
            cwd: PathBuf::from("/workspace"),
            created_at_unix_seconds: 10,
            updated_at_unix_seconds: 20,
        },
        turns: vec![Turn {
            id: "turn-1".to_owned(),
            status: TurnStatus::Completed,
            timing: Default::default(),
            items: vec![ThreadItem::AgentMessage {
                id: "agent-1".to_owned(),
                text: "Done".to_owned(),
                phase: Some(AgentMessagePhase::FinalAnswer),
            }],
        }],
    }
}

fn attached_projection() -> LiveThreadProjection {
    let mut projection = LiveThreadProjection::default();
    projection.attach(ThreadSubscription {
        thread: thread(),
        runtime_status: ThreadRuntimeStatus::Idle,
    });
    projection
}

#[test]
fn preserves_reasoning_lifecycle_timing_when_the_final_item_replaces_the_start() {
    let mut items = vec![ThreadItem::Activity(Activity {
        id: "reasoning-1".to_owned(),
        kind: ActivityKind::Reasoning,
        status: ActivityStatus::InProgress,
        started_at_unix_milliseconds: Some(1_723_456_789_000),
        completed_at_unix_milliseconds: None,
        summary: "Planning".to_owned(),
        detail: None,
        context: None,
        command_actions: vec![],
    })];

    upsert_item(
        &mut items,
        ThreadItem::Activity(Activity {
            id: "reasoning-1".to_owned(),
            kind: ActivityKind::Reasoning,
            status: ActivityStatus::Completed,
            started_at_unix_milliseconds: None,
            completed_at_unix_milliseconds: Some(1_723_456_793_250),
            summary: "Planning UI state".to_owned(),
            detail: None,
            context: None,
            command_actions: vec![],
        }),
    );

    let ThreadItem::Activity(activity) = &items[0] else {
        panic!("the item should remain an activity");
    };
    assert_eq!(
        activity.started_at_unix_milliseconds,
        Some(1_723_456_789_000)
    );
    assert_eq!(
        activity.completed_at_unix_milliseconds,
        Some(1_723_456_793_250)
    );
    assert_eq!(activity.summary, "Planning UI state");
}

#[test]
fn distinguishes_starting_from_a_confirmed_active_turn() {
    let mut projection = attached_projection();

    assert!(projection.begin_turn());
    assert_eq!(projection.runtime(), LiveRuntimeState::Starting);

    let effect = projection.apply_event(
        &ThreadStreamEvent::TurnStarted {
            thread_id: "thread-1".to_owned(),
            turn: Turn {
                id: "turn-2".to_owned(),
                status: TurnStatus::InProgress,
                timing: Default::default(),
                items: vec![],
            },
        },
        "thread-1",
    );

    assert_eq!(effect.started_turn_id.as_deref(), Some("turn-2"));
    assert_eq!(
        projection.runtime(),
        LiveRuntimeState::Active {
            turn_id: Some("turn-2".to_owned()),
            active_flags: vec![],
        }
    );

    let status_effect = projection.apply_event(
        &ThreadStreamEvent::ThreadStatusChanged {
            thread_id: "thread-1".to_owned(),
            status: ThreadRuntimeStatus::Active {
                active_flags: vec![ThreadActiveFlag::WaitingOnApproval],
            },
        },
        "thread-1",
    );
    assert!(status_effect.runtime_changed);
    assert_eq!(
        projection.runtime(),
        LiveRuntimeState::Active {
            turn_id: Some("turn-2".to_owned()),
            active_flags: vec![ThreadActiveFlag::WaitingOnApproval],
        }
    );
}

#[test]
fn folds_incremental_messages_and_activity_content() {
    let mut projection = attached_projection();
    let turn_id = "turn-2".to_owned();
    projection.apply_event(
        &ThreadStreamEvent::TurnStarted {
            thread_id: "thread-1".to_owned(),
            turn: Turn {
                id: turn_id.clone(),
                status: TurnStatus::InProgress,
                timing: Default::default(),
                items: vec![],
            },
        },
        "thread-1",
    );
    projection.apply_event(
        &ThreadStreamEvent::ItemStarted {
            thread_id: "thread-1".to_owned(),
            turn_id: turn_id.clone(),
            item: ThreadItem::Activity(Activity {
                id: "reasoning-1".to_owned(),
                kind: ActivityKind::Reasoning,
                status: ActivityStatus::InProgress,
                started_at_unix_milliseconds: None,
                completed_at_unix_milliseconds: None,
                summary: String::new(),
                detail: None,
                context: None,
                command_actions: vec![],
            }),
        },
        "thread-1",
    );
    projection.apply_event(
        &ThreadStreamEvent::ActivityUpdated {
            thread_id: "thread-1".to_owned(),
            turn_id: turn_id.clone(),
            item_id: "reasoning-1".to_owned(),
            update: ActivityUpdate::AppendSummary("Inspect files".to_owned()),
        },
        "thread-1",
    );
    projection.apply_event(
        &ThreadStreamEvent::ActivityUpdated {
            thread_id: "thread-1".to_owned(),
            turn_id: turn_id.clone(),
            item_id: "reasoning-1".to_owned(),
            update: ActivityUpdate::StartSummarySection,
        },
        "thread-1",
    );
    projection.apply_event(
        &ThreadStreamEvent::ActivityUpdated {
            thread_id: "thread-1".to_owned(),
            turn_id: turn_id.clone(),
            item_id: "reasoning-1".to_owned(),
            update: ActivityUpdate::AppendSummary("Update UI".to_owned()),
        },
        "thread-1",
    );
    projection.apply_event(
        &ThreadStreamEvent::ItemStarted {
            thread_id: "thread-1".to_owned(),
            turn_id: turn_id.clone(),
            item: ThreadItem::Activity(Activity {
                id: "tool-1".to_owned(),
                kind: ActivityKind::ToolCall,
                status: ActivityStatus::InProgress,
                started_at_unix_milliseconds: None,
                completed_at_unix_milliseconds: None,
                summary: "server / search".to_owned(),
                detail: Some("{}".to_owned()),
                context: None,
                command_actions: vec![],
            }),
        },
        "thread-1",
    );
    projection.apply_event(
        &ThreadStreamEvent::ActivityUpdated {
            thread_id: "thread-1".to_owned(),
            turn_id: turn_id.clone(),
            item_id: "tool-1".to_owned(),
            update: ActivityUpdate::AppendDetail("Fetching results\n".to_owned()),
        },
        "thread-1",
    );
    projection.apply_event(
        &ThreadStreamEvent::AgentMessageDelta {
            thread_id: "thread-1".to_owned(),
            turn_id,
            item_id: "agent-2".to_owned(),
            delta: "Working".to_owned(),
        },
        "thread-1",
    );

    let live_turn = projection.conversation().unwrap().turns.last().unwrap();
    let ThreadItem::Activity(reasoning) = &live_turn.items[0] else {
        panic!("the first item should be live reasoning");
    };
    assert_eq!(reasoning.summary, "Inspect files\nUpdate UI");
    let ThreadItem::Activity(tool) = &live_turn.items[1] else {
        panic!("the second item should be the live tool call");
    };
    assert_eq!(tool.detail.as_deref(), Some("{}\nFetching results\n"));
    assert!(matches!(
        &live_turn.items[2],
        ThreadItem::AgentMessage { text, .. } if text == "Working"
    ));
}

#[test]
fn retains_the_initial_active_subscription_until_persistence_catches_up() {
    let mut subscribed = thread();
    subscribed.turns.push(Turn {
        id: "turn-2".to_owned(),
        status: TurnStatus::InProgress,
        timing: Default::default(),
        items: vec![ThreadItem::AgentMessage {
            id: "agent-2".to_owned(),
            text: "Live text".to_owned(),
            phase: Some(AgentMessagePhase::Commentary),
        }],
    });
    let mut projection = LiveThreadProjection::default();
    projection.attach(ThreadSubscription {
        thread: subscribed,
        runtime_status: ThreadRuntimeStatus::Active {
            active_flags: vec![],
        },
    });

    let mut stale = thread();
    stale.turns.push(Turn {
        id: "turn-2".to_owned(),
        status: TurnStatus::InProgress,
        timing: Default::default(),
        items: vec![],
    });
    assert!(!projection.accept_snapshot(stale));

    assert_eq!(
        projection.runtime(),
        LiveRuntimeState::Active {
            turn_id: Some("turn-2".to_owned()),
            active_flags: vec![],
        }
    );
    assert_eq!(projection.conversation().unwrap().turns[1].items.len(), 1);
}

#[test]
fn retains_a_partial_live_turn_without_hiding_new_persisted_turns() {
    let mut projection = attached_projection();
    projection.apply_event(
        &ThreadStreamEvent::TurnStarted {
            thread_id: "thread-1".to_owned(),
            turn: Turn {
                id: "turn-2".to_owned(),
                status: TurnStatus::InProgress,
                timing: Default::default(),
                items: vec![ThreadItem::UserMessage {
                    id: "user-2".to_owned(),
                    content: vec![UserInput::Text("Continue".to_owned())],
                }],
            },
        },
        "thread-1",
    );

    let mut persisted = thread();
    persisted.turns.push(Turn {
        id: "turn-2".to_owned(),
        status: TurnStatus::InProgress,
        timing: Default::default(),
        items: vec![],
    });
    persisted.turns.push(Turn {
        id: "turn-3".to_owned(),
        status: TurnStatus::Completed,
        timing: Default::default(),
        items: vec![ThreadItem::AgentMessage {
            id: "final-3".to_owned(),
            text: "External answer".to_owned(),
            phase: Some(AgentMessagePhase::FinalAnswer),
        }],
    });

    assert!(projection.accept_snapshot(persisted));
    let merged = projection.conversation().unwrap();
    assert_eq!(
        merged
            .turns
            .iter()
            .map(|turn| turn.id.as_str())
            .collect::<Vec<_>>(),
        ["turn-1", "turn-2", "turn-3"]
    );
    assert_eq!(merged.turns[1].items.len(), 1);
    assert_eq!(merged.turns[2].items.len(), 1);
}

#[test]
fn accepts_an_authoritative_completed_snapshot_after_missing_live_completion() {
    let mut projection = attached_projection();
    projection.apply_event(
        &ThreadStreamEvent::TurnStarted {
            thread_id: "thread-1".to_owned(),
            turn: Turn {
                id: "turn-2".to_owned(),
                status: TurnStatus::InProgress,
                timing: Default::default(),
                items: vec![ThreadItem::AgentMessage {
                    id: "commentary-2".to_owned(),
                    text: "Working".to_owned(),
                    phase: Some(AgentMessagePhase::Commentary),
                }],
            },
        },
        "thread-1",
    );
    projection.apply_event(
        &ThreadStreamEvent::ItemStarted {
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-2".to_owned(),
            item: ThreadItem::Activity(Activity {
                id: "command-2".to_owned(),
                kind: ActivityKind::CommandExecution,
                status: ActivityStatus::InProgress,
                started_at_unix_milliseconds: Some(1_000),
                completed_at_unix_milliseconds: None,
                summary: "cargo test".to_owned(),
                detail: None,
                context: Some("/workspace".to_owned()),
                command_actions: vec![],
            }),
        },
        "thread-1",
    );

    let mut persisted = thread();
    persisted.turns.push(Turn {
        id: "turn-2".to_owned(),
        status: TurnStatus::Completed,
        timing: Default::default(),
        items: vec![
            ThreadItem::Activity(Activity {
                id: "command-2".to_owned(),
                kind: ActivityKind::CommandExecution,
                status: ActivityStatus::Completed,
                started_at_unix_milliseconds: None,
                completed_at_unix_milliseconds: Some(2_000),
                summary: "cargo test".to_owned(),
                detail: Some("passed".to_owned()),
                context: Some("/workspace".to_owned()),
                command_actions: vec![],
            }),
            ThreadItem::AgentMessage {
                id: "final-2".to_owned(),
                text: "Done".to_owned(),
                phase: Some(AgentMessagePhase::FinalAnswer),
            },
        ],
    });

    assert!(projection.accept_snapshot(persisted));
    let completed = projection.conversation().unwrap().turns.last().unwrap();
    assert_eq!(completed.status, TurnStatus::Completed);
    assert!(completed.items.iter().any(|item| {
        matches!(
            item,
            ThreadItem::AgentMessage { id, text, .. }
                if id == "commentary-2" && text == "Working"
        )
    }));
    assert!(completed.items.iter().any(|item| {
        matches!(
            item,
            ThreadItem::Activity(Activity {
                id,
                status: ActivityStatus::Completed,
                started_at_unix_milliseconds: Some(1_000),
                completed_at_unix_milliseconds: Some(2_000),
                detail: Some(detail),
                ..
            }) if id == "command-2" && detail == "passed"
        )
    }));
    assert!(completed.items.iter().any(|item| {
        matches!(
            item,
            ThreadItem::AgentMessage { id, text, .. }
                if id == "final-2" && text == "Done"
        )
    }));
}

#[test]
fn new_thread_fork_boundary_waits_for_the_authoritative_first_turn() {
    let mut initial = thread();
    initial.turns.clear();
    let mut projection = LiveThreadProjection::default();
    projection.attach(ThreadSubscription {
        thread: initial,
        runtime_status: ThreadRuntimeStatus::Idle,
    });
    let items = vec![ThreadItem::UserMessage {
        id: "user-1".to_owned(),
        content: vec![UserInput::Text("Hello".to_owned())],
    }];
    projection.apply_event(
        &ThreadStreamEvent::TurnCompleted {
            thread_id: "thread-1".to_owned(),
            turn: Turn {
                id: "live-turn".to_owned(),
                status: TurnStatus::Completed,
                timing: Default::default(),
                items: items.clone(),
            },
        },
        "thread-1",
    );
    assert!(projection.forkable_turn_ids().is_empty());

    let mut persisted = thread();
    persisted.turns = vec![Turn {
        id: "persisted-turn".to_owned(),
        status: TurnStatus::Completed,
        timing: Default::default(),
        items,
    }];
    assert!(projection.accept_snapshot(persisted));

    let turns = &projection.conversation().unwrap().turns;
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].id, "persisted-turn");
    assert_eq!(projection.forkable_turn_ids(), ["persisted-turn"]);
}

#[test]
fn authoritative_snapshot_publishes_an_unchanged_live_turn_as_forkable() {
    let mut initial = thread();
    initial.turns.clear();
    let mut projection = LiveThreadProjection::default();
    projection.attach(ThreadSubscription {
        thread: initial,
        runtime_status: ThreadRuntimeStatus::Idle,
    });
    let completed = Turn {
        id: "turn-1".to_owned(),
        status: TurnStatus::Completed,
        timing: Default::default(),
        items: vec![ThreadItem::UserMessage {
            id: "user-1".to_owned(),
            content: vec![UserInput::Text("Hello".to_owned())],
        }],
    };
    projection.apply_event(
        &ThreadStreamEvent::TurnCompleted {
            thread_id: "thread-1".to_owned(),
            turn: completed.clone(),
        },
        "thread-1",
    );

    assert!(projection.forkable_turn_ids().is_empty());
    assert!(projection.accept_snapshot(Thread {
        summary: projection.conversation().unwrap().summary.clone(),
        turns: vec![completed],
    }));
    assert_eq!(projection.forkable_turn_ids(), ["turn-1"]);
}

#[test]
fn new_thread_snapshot_does_not_duplicate_equivalent_items_when_all_ids_differ() {
    let mut initial = thread();
    initial.turns.clear();
    let mut projection = LiveThreadProjection::default();
    projection.attach(ThreadSubscription {
        thread: initial,
        runtime_status: ThreadRuntimeStatus::Idle,
    });
    projection.apply_event(
        &ThreadStreamEvent::TurnStarted {
            thread_id: "thread-1".to_owned(),
            turn: Turn {
                id: "turn-1".to_owned(),
                status: TurnStatus::InProgress,
                timing: Default::default(),
                items: vec![],
            },
        },
        "thread-1",
    );
    projection.apply_event(
        &ThreadStreamEvent::ItemCompleted {
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            item: ThreadItem::UserMessage {
                id: "live-user".to_owned(),
                content: vec![UserInput::Text("Hello".to_owned())],
            },
        },
        "thread-1",
    );
    projection.apply_event(
        &ThreadStreamEvent::ItemCompleted {
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-1".to_owned(),
            item: ThreadItem::AgentMessage {
                id: "live-agent".to_owned(),
                text: "Hi".to_owned(),
                phase: Some(AgentMessagePhase::FinalAnswer),
            },
        },
        "thread-1",
    );
    projection.apply_event(
        &ThreadStreamEvent::TurnCompleted {
            thread_id: "thread-1".to_owned(),
            turn: Turn {
                id: "turn-1".to_owned(),
                status: TurnStatus::Completed,
                timing: Default::default(),
                items: vec![],
            },
        },
        "thread-1",
    );

    let mut persisted = thread();
    persisted.turns = vec![Turn {
        id: "persisted-turn".to_owned(),
        status: TurnStatus::Completed,
        timing: Default::default(),
        items: vec![
            ThreadItem::UserMessage {
                id: "item-1".to_owned(),
                content: vec![UserInput::Text("Hello".to_owned())],
            },
            ThreadItem::AgentMessage {
                id: "item-2".to_owned(),
                text: "Hi".to_owned(),
                phase: Some(AgentMessagePhase::FinalAnswer),
            },
        ],
    }];
    assert!(projection.accept_snapshot(persisted));

    let turns = &projection.conversation().unwrap().turns;
    assert_eq!(turns.len(), 1);
    assert_eq!(turns[0].id, "persisted-turn");
    assert_eq!(turns[0].items.len(), 2);
    assert_eq!(thread_item_id(&turns[0].items[0]), "item-1");
    assert_eq!(thread_item_id(&turns[0].items[1]), "item-2");
}

#[test]
fn renamed_message_matching_preserves_repeated_messages() {
    let snapshot = Turn {
        id: "turn-1".to_owned(),
        status: TurnStatus::Completed,
        timing: Default::default(),
        items: vec![ThreadItem::AgentMessage {
            id: "item-1".to_owned(),
            text: "Same".to_owned(),
            phase: Some(AgentMessagePhase::Commentary),
        }],
    };
    let retained = Turn {
        id: "turn-1".to_owned(),
        status: TurnStatus::Completed,
        timing: Default::default(),
        items: vec![
            ThreadItem::AgentMessage {
                id: "live-1".to_owned(),
                text: "Same".to_owned(),
                phase: Some(AgentMessagePhase::Commentary),
            },
            ThreadItem::AgentMessage {
                id: "live-2".to_owned(),
                text: "Same".to_owned(),
                phase: Some(AgentMessagePhase::Commentary),
            },
        ],
    };

    let merged = merge_snapshot_turn(snapshot, &retained);

    assert_eq!(merged.items.len(), 2);
    assert_eq!(thread_item_id(&merged.items[0]), "item-1");
    assert_eq!(thread_item_id(&merged.items[1]), "live-2");
}

#[test]
fn snapshot_merge_preserves_live_turn_timing_when_history_omits_it() {
    let snapshot = Turn {
        id: "turn-1".to_owned(),
        status: TurnStatus::Completed,
        timing: Default::default(),
        items: vec![],
    };
    let retained = Turn {
        id: "turn-1".to_owned(),
        status: TurnStatus::Completed,
        timing: TurnTiming::new(Some(10), Some(13), Some(2_750)),
        items: vec![],
    };

    let merged = merge_snapshot_turn(snapshot, &retained);

    assert_eq!(
        merged.timing,
        TurnTiming::new(Some(10), Some(13), Some(2_750))
    );
}

#[test]
fn renamed_message_matching_preserves_snapshot_only_prefix_order() {
    let snapshot = Turn {
        id: "turn-1".to_owned(),
        status: TurnStatus::Completed,
        timing: Default::default(),
        items: vec![
            ThreadItem::Other {
                id: "persisted-only".to_owned(),
                kind: "futureItem".to_owned(),
            },
            ThreadItem::UserMessage {
                id: "item-1".to_owned(),
                content: vec![UserInput::Text("Hello".to_owned())],
            },
        ],
    };
    let retained = Turn {
        id: "turn-1".to_owned(),
        status: TurnStatus::Completed,
        timing: Default::default(),
        items: vec![
            ThreadItem::UserMessage {
                id: "live-user".to_owned(),
                content: vec![UserInput::Text("Hello".to_owned())],
            },
            ThreadItem::AgentMessage {
                id: "live-agent".to_owned(),
                text: "Hi".to_owned(),
                phase: Some(AgentMessagePhase::FinalAnswer),
            },
        ],
    };

    let merged = merge_snapshot_turn(snapshot, &retained);
    let item_ids = merged.items.iter().map(thread_item_id).collect::<Vec<_>>();

    assert_eq!(item_ids, ["persisted-only", "item-1", "live-agent"]);
}

#[test]
fn new_thread_snapshot_keeps_distinct_turns_without_shared_items() {
    let mut initial = thread();
    initial.turns.clear();
    let mut projection = LiveThreadProjection::default();
    projection.attach(ThreadSubscription {
        thread: initial,
        runtime_status: ThreadRuntimeStatus::Idle,
    });
    projection.apply_event(
        &ThreadStreamEvent::TurnCompleted {
            thread_id: "thread-1".to_owned(),
            turn: Turn {
                id: "live-turn".to_owned(),
                status: TurnStatus::Completed,
                timing: Default::default(),
                items: vec![ThreadItem::UserMessage {
                    id: "live-user".to_owned(),
                    content: vec![UserInput::Text("First".to_owned())],
                }],
            },
        },
        "thread-1",
    );

    let mut persisted = thread();
    persisted.turns = vec![Turn {
        id: "persisted-turn".to_owned(),
        status: TurnStatus::Completed,
        timing: Default::default(),
        items: vec![ThreadItem::UserMessage {
            id: "persisted-user".to_owned(),
            content: vec![UserInput::Text("Second".to_owned())],
        }],
    }];
    assert!(projection.accept_snapshot(persisted));

    assert_eq!(projection.conversation().unwrap().turns.len(), 2);
}

#[test]
fn semantic_turn_matching_does_not_cross_later_turns() {
    let mut projection = attached_projection();
    projection.apply_event(
        &ThreadStreamEvent::TurnCompleted {
            thread_id: "thread-1".to_owned(),
            turn: Turn {
                id: "live-turn".to_owned(),
                status: TurnStatus::Completed,
                timing: Default::default(),
                items: vec![ThreadItem::UserMessage {
                    id: "live-user".to_owned(),
                    content: vec![UserInput::Text("Same".to_owned())],
                }],
            },
        },
        "thread-1",
    );

    let mut persisted = thread();
    persisted.turns.push(Turn {
        id: "persisted-turn".to_owned(),
        status: TurnStatus::Completed,
        timing: Default::default(),
        items: vec![ThreadItem::UserMessage {
            id: "persisted-user".to_owned(),
            content: vec![UserInput::Text("Same".to_owned())],
        }],
    });
    assert!(projection.accept_snapshot(persisted));

    let turn_ids = projection
        .conversation()
        .unwrap()
        .turns
        .iter()
        .map(|turn| turn.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(turn_ids, ["turn-1", "live-turn", "persisted-turn"]);
}

#[test]
fn marks_only_the_active_turn_failed_when_the_stream_disconnects() {
    let mut projection = attached_projection();
    projection.apply_event(
        &ThreadStreamEvent::TurnStarted {
            thread_id: "thread-1".to_owned(),
            turn: Turn {
                id: "turn-2".to_owned(),
                status: TurnStatus::InProgress,
                timing: Default::default(),
                items: vec![ThreadItem::Activity(Activity {
                    id: "command-2".to_owned(),
                    kind: ActivityKind::CommandExecution,
                    status: ActivityStatus::InProgress,
                    started_at_unix_milliseconds: None,
                    completed_at_unix_milliseconds: None,
                    summary: "cargo test".to_owned(),
                    detail: None,
                    context: Some("/workspace".to_owned()),
                    command_actions: vec![],
                })],
            },
        },
        "thread-1",
    );

    let effect = projection.fail_stream();

    assert!(effect.conversation_changed);
    assert!(effect.runtime_changed);
    assert_eq!(projection.runtime(), LiveRuntimeState::Detached);
    let turns = &projection.conversation().unwrap().turns;
    assert_eq!(turns[0].status, TurnStatus::Completed);
    assert_eq!(turns[1].status, TurnStatus::Failed);
    let ThreadItem::Activity(activity) = &turns[1].items[0] else {
        panic!("the failed turn should contain an activity");
    };
    assert_eq!(activity.status, ActivityStatus::Failed);
}
