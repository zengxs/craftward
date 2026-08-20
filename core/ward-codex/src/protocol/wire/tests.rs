// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use super::{
    InitializeParams, ThreadForkParams, ThreadReadResponse, ThreadResumeResponse,
    ThreadStartParams, ThreadStartResponse, TurnStartParams, TurnSteerParams, turn_stream_event,
};
use crate::{
    Activity, ActivityKind, ActivityStatus, ActivityUpdate, AgentMessagePhase, CommandAction,
    CommandActionKind, InferenceOverride, ReasoningEffort, ThreadActiveFlag, ThreadInferenceState,
    ThreadItem, ThreadRuntimeStatus, ThreadStartOptions, ThreadStreamEvent, TurnInput, TurnMode,
    TurnOptions, TurnPermissionPreset, UserInput,
};

fn inference_state(model: Option<&str>, reasoning_effort: Option<&str>) -> ThreadInferenceState {
    ThreadInferenceState::new(
        model.map(str::to_owned),
        reasoning_effort.map(str::to_owned),
    )
}

#[test]
fn serializes_full_and_turn_bounded_forks_without_overrides() {
    assert_eq!(
        serde_json::to_value(ThreadForkParams {
            thread_id: "thread-1",
            last_turn_id: None,
        })
        .unwrap(),
        serde_json::json!({ "threadId": "thread-1" })
    );
    assert_eq!(
        serde_json::to_value(ThreadForkParams {
            thread_id: "thread-1",
            last_turn_id: Some("turn-2"),
        })
        .unwrap(),
        serde_json::json!({ "threadId": "thread-1", "lastTurnId": "turn-2" })
    );
}

#[test]
fn opts_in_to_the_experimental_app_server_surface() {
    assert_eq!(
        serde_json::to_value(InitializeParams::craftward()).unwrap(),
        serde_json::json!({
            "clientInfo": {
                "name": "craftward",
                "title": "Craftward",
                "version": env!("CARGO_PKG_VERSION")
            },
            "capabilities": { "experimentalApi": true }
        })
    );
}

#[test]
fn maps_conversation_items_without_exposing_the_full_wire_schema() {
    let response: ThreadReadResponse = serde_json::from_value(serde_json::json!({
        "thread": {
            "id": "thread-1",
            "name": "Example",
            "preview": "First message",
            "cwd": "/workspace",
            "createdAt": 10,
            "updatedAt": 20,
            "status": { "type": "idle" },
            "turns": [{
                "id": "turn-1",
                "status": "completed",
                "items": [{
                    "id": "item-1",
                    "type": "userMessage",
                    "content": [
                        { "type": "text", "text": "Hello", "text_elements": [] },
                        { "type": "futureInput", "value": 1 }
                    ]
                }, {
                    "id": "item-2",
                    "type": "agentMessage",
                    "text": "Hi",
                    "phase": "final_answer"
                }, {
                    "id": "item-3",
                    "type": "reasoning",
                    "summary": ["Inspect the relevant files."],
                    "content": []
                }, {
                    "id": "item-4",
                    "type": "commandExecution",
                    "command": "sed -n 1,80p src/main.rs",
                    "commandActions": [{
                        "type": "read",
                        "command": "sed -n 1,80p src/main.rs",
                        "name": "src/main.rs",
                        "path": "/workspace/src/main.rs"
                    }],
                    "cwd": "/workspace",
                    "status": "completed",
                    "aggregatedOutput": "fn main() {}"
                }, {
                    "id": "item-5",
                    "type": "webSearch",
                    "query": "Codex app-server protocol"
                }, {
                    "id": "item-6",
                    "type": "futureItem",
                    "value": 2
                }]
            }]
        }
    }))
    .expect("the wire response should decode");

    let thread = response
        .thread
        .into_thread()
        .expect("the thread should map");

    assert_eq!(thread.summary.name.as_deref(), Some("Example"));
    assert_eq!(thread.turns.len(), 1);
    assert_eq!(
        thread.turns[0].items,
        vec![
            ThreadItem::UserMessage {
                id: "item-1".to_owned(),
                content: vec![
                    UserInput::Text("Hello".to_owned()),
                    UserInput::Other {
                        kind: "futureInput".to_owned()
                    }
                ]
            },
            ThreadItem::AgentMessage {
                id: "item-2".to_owned(),
                text: "Hi".to_owned(),
                phase: Some(AgentMessagePhase::FinalAnswer)
            },
            ThreadItem::Activity(Activity {
                id: "item-3".to_owned(),
                kind: ActivityKind::Reasoning,
                status: ActivityStatus::Unspecified,
                started_at_unix_milliseconds: None,
                completed_at_unix_milliseconds: None,
                summary: "Inspect the relevant files.".to_owned(),
                detail: None,
                context: None,
                command_actions: vec![],
            }),
            ThreadItem::Activity(Activity {
                id: "item-4".to_owned(),
                kind: ActivityKind::CommandExecution,
                status: ActivityStatus::Completed,
                started_at_unix_milliseconds: None,
                completed_at_unix_milliseconds: None,
                summary: "sed -n 1,80p src/main.rs".to_owned(),
                detail: Some("fn main() {}".to_owned()),
                context: Some("/workspace".to_owned()),
                command_actions: vec![CommandAction {
                    kind: CommandActionKind::Read,
                    command: "sed -n 1,80p src/main.rs".to_owned(),
                    name: Some("src/main.rs".to_owned()),
                    path: Some(PathBuf::from("/workspace/src/main.rs")),
                    query: None,
                }],
            }),
            ThreadItem::Activity(Activity {
                id: "item-5".to_owned(),
                kind: ActivityKind::WebSearch,
                status: ActivityStatus::Unspecified,
                started_at_unix_milliseconds: None,
                completed_at_unix_milliseconds: None,
                summary: "Codex app-server protocol".to_owned(),
                detail: None,
                context: None,
                command_actions: vec![],
            }),
            ThreadItem::Other {
                id: "item-6".to_owned(),
                kind: "futureItem".to_owned()
            }
        ]
    );
}

#[test]
fn maps_live_item_and_delta_notifications() {
    let started = turn_stream_event(
        "item/started",
        serde_json::json!({
            "threadId": "thread-1",
            "turnId": "turn-2",
            "startedAtMs": 10,
            "item": {
                "id": "command-1",
                "type": "commandExecution",
                "command": "sed -n 1,20p src/main.rs",
                "commandActions": [{
                    "type": "read",
                    "command": "sed -n 1,20p src/main.rs",
                    "path": "/workspace/src/main.rs"
                }],
                "cwd": "/workspace",
                "status": "inProgress"
            }
        }),
    )
    .unwrap()
    .expect("the item notification should be displayable");
    let ThreadStreamEvent::ItemStarted {
        thread_id,
        turn_id,
        item: ThreadItem::Activity(activity),
    } = started
    else {
        panic!("the notification should start one activity");
    };
    assert_eq!(thread_id, "thread-1");
    assert_eq!(turn_id, "turn-2");
    assert_eq!(activity.status, ActivityStatus::InProgress);
    assert_eq!(activity.command_actions[0].kind, CommandActionKind::Read);

    assert_eq!(
        turn_stream_event(
            "item/commandExecution/outputDelta",
            serde_json::json!({
                "threadId": "thread-1",
                "turnId": "turn-2",
                "itemId": "command-1",
                "delta": "fn main() {}"
            }),
        )
        .unwrap(),
        Some(ThreadStreamEvent::ActivityOutputDelta {
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-2".to_owned(),
            item_id: "command-1".to_owned(),
            delta: "fn main() {}".to_owned(),
        })
    );
}

#[test]
fn maps_live_reasoning_summary_as_a_displayable_activity() {
    let started = turn_stream_event(
        "item/started",
        serde_json::json!({
            "threadId": "thread-1",
            "turnId": "turn-2",
            "startedAtMs": 1_723_456_789_000_i64,
            "item": {
                "id": "reasoning-1",
                "type": "reasoning",
                "summary": [],
                "content": ["raw reasoning must not become display text"]
            }
        }),
    )
    .unwrap()
    .expect("the reasoning lifecycle should be displayable");
    let ThreadStreamEvent::ItemStarted {
        thread_id,
        turn_id,
        item: ThreadItem::Activity(activity),
    } = started
    else {
        panic!("the notification should start one reasoning activity");
    };
    assert_eq!(thread_id, "thread-1");
    assert_eq!(turn_id, "turn-2");
    assert_eq!(activity.status, ActivityStatus::InProgress);
    assert_eq!(
        activity.started_at_unix_milliseconds,
        Some(1_723_456_789_000)
    );
    assert_eq!(activity.completed_at_unix_milliseconds, None);
    assert!(activity.summary.is_empty());

    let completed = turn_stream_event(
        "item/completed",
        serde_json::json!({
            "threadId": "thread-1",
            "turnId": "turn-2",
            "completedAtMs": 1_723_456_793_250_i64,
            "item": {
                "id": "reasoning-1",
                "type": "reasoning",
                "summary": ["Planning UI state"],
                "content": ["raw reasoning must not become display text"]
            }
        }),
    )
    .unwrap()
    .expect("the reasoning completion should be displayable");
    let ThreadStreamEvent::ItemCompleted {
        item: ThreadItem::Activity(activity),
        ..
    } = completed
    else {
        panic!("the notification should complete one reasoning activity");
    };
    assert_eq!(activity.status, ActivityStatus::Completed);
    assert_eq!(activity.started_at_unix_milliseconds, None);
    assert_eq!(
        activity.completed_at_unix_milliseconds,
        Some(1_723_456_793_250)
    );
    assert_eq!(activity.summary, "Planning UI state");
}

#[test]
fn maps_live_reasoning_summary_deltas() {
    assert_eq!(
        turn_stream_event(
            "item/reasoning/summaryTextDelta",
            serde_json::json!({
                "threadId": "thread-1",
                "turnId": "turn-2",
                "itemId": "reasoning-1",
                "summaryIndex": 0,
                "delta": "Planning UI state"
            }),
        )
        .unwrap(),
        Some(ThreadStreamEvent::ActivityUpdated {
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-2".to_owned(),
            item_id: "reasoning-1".to_owned(),
            update: ActivityUpdate::AppendSummary("Planning UI state".to_owned()),
        })
    );

    assert_eq!(
        turn_stream_event(
            "item/reasoning/summaryPartAdded",
            serde_json::json!({
                "threadId": "thread-1",
                "turnId": "turn-2",
                "itemId": "reasoning-1",
                "summaryIndex": 1
            }),
        )
        .unwrap(),
        Some(ThreadStreamEvent::ActivityUpdated {
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-2".to_owned(),
            item_id: "reasoning-1".to_owned(),
            update: ActivityUpdate::StartSummarySection,
        })
    );

    assert_eq!(
        turn_stream_event(
            "item/reasoning/textDelta",
            serde_json::json!({
                "threadId": "thread-1",
                "turnId": "turn-2",
                "itemId": "reasoning-1",
                "contentIndex": 0,
                "delta": "private raw reasoning"
            }),
        )
        .unwrap(),
        None
    );
}

#[test]
fn maps_the_subscription_runtime_status_from_the_resume_response() {
    let response: ThreadResumeResponse = serde_json::from_value(serde_json::json!({
        "model": "gpt-5.6-sol",
        "reasoningEffort": "high",
        "thread": {
            "id": "thread-1",
            "name": null,
            "preview": "Working",
            "cwd": "/workspace",
            "createdAt": 10,
            "updatedAt": 20,
            "status": {
                "type": "active",
                "activeFlags": [
                    "waitingOnApproval",
                    "waitingOnUserInput",
                    "futureFlag"
                ]
            },
            "turns": []
        }
    }))
    .expect("the resume response should decode");

    let (subscription, inference) = response.into_parts().expect("the subscription should map");

    assert_eq!(inference.model(), Some("gpt-5.6-sol"));
    assert_eq!(inference.reasoning_effort(), Some("high"));
    assert_eq!(subscription.thread.summary.id, "thread-1");
    assert_eq!(
        subscription.runtime_status,
        ThreadRuntimeStatus::Active {
            active_flags: vec![
                ThreadActiveFlag::WaitingOnApproval,
                ThreadActiveFlag::WaitingOnUserInput,
                ThreadActiveFlag::Unknown("futureFlag".to_owned()),
            ]
        }
    );
}

#[test]
fn serializes_a_thread_start_for_one_working_directory() {
    assert_eq!(
        serde_json::to_value(ThreadStartParams::new(
            Path::new("/workspace"),
            ThreadStartOptions::default(),
        ))
        .unwrap(),
        serde_json::json!({ "cwd": "/workspace" })
    );
}

#[test]
fn maps_a_started_thread_to_the_normalized_subscription() {
    let response: ThreadStartResponse = serde_json::from_value(serde_json::json!({
        "approvalPolicy": "on-request",
        "approvalsReviewer": "user",
        "cwd": "/workspace",
        "model": "gpt-5.6-sol",
        "reasoningEffort": "medium",
        "modelProvider": "openai",
        "sandbox": { "type": "workspaceWrite" },
        "thread": {
            "id": "thread-new",
            "name": null,
            "preview": "",
            "cwd": "/workspace",
            "createdAt": 10,
            "updatedAt": 10,
            "ephemeral": false,
            "status": { "type": "idle" },
            "turns": []
        }
    }))
    .expect("the thread start response should decode");

    let (subscription, inference, ephemeral) = response
        .into_parts()
        .expect("the started thread should map");

    assert_eq!(inference.model(), Some("gpt-5.6-sol"));
    assert_eq!(inference.reasoning_effort(), Some("medium"));
    assert_eq!(ephemeral, Some(false));
    assert_eq!(subscription.thread.summary.id, "thread-new");
    assert_eq!(subscription.thread.summary.cwd, PathBuf::from("/workspace"));
    assert_eq!(subscription.runtime_status, ThreadRuntimeStatus::Idle);
}

#[test]
fn maps_runtime_status_notifications_from_the_subscribed_connection() {
    assert_eq!(
        turn_stream_event(
            "thread/status/changed",
            serde_json::json!({
                "threadId": "thread-1",
                "status": {
                    "type": "active",
                    "activeFlags": ["waitingOnApproval"]
                }
            }),
        )
        .unwrap(),
        Some(ThreadStreamEvent::ThreadStatusChanged {
            thread_id: "thread-1".to_owned(),
            status: ThreadRuntimeStatus::Active {
                active_flags: vec![ThreadActiveFlag::WaitingOnApproval],
            },
        })
    );
}

#[test]
fn maps_incremental_activity_notifications() {
    assert_eq!(
        turn_stream_event(
            "item/plan/delta",
            serde_json::json!({
                "threadId": "thread-1",
                "turnId": "turn-2",
                "itemId": "plan-1",
                "delta": "Inspect files"
            }),
        )
        .unwrap(),
        Some(ThreadStreamEvent::ActivityUpdated {
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-2".to_owned(),
            item_id: "plan-1".to_owned(),
            update: ActivityUpdate::AppendSummary("Inspect files".to_owned()),
        })
    );

    assert_eq!(
        turn_stream_event(
            "item/fileChange/patchUpdated",
            serde_json::json!({
                "threadId": "thread-1",
                "turnId": "turn-2",
                "itemId": "change-1",
                "changes": [{
                    "path": "/workspace/src/main.rs",
                    "kind": { "type": "update" },
                    "diff": "+fn main() {}"
                }]
            }),
        )
        .unwrap(),
        Some(ThreadStreamEvent::ActivityUpdated {
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-2".to_owned(),
            item_id: "change-1".to_owned(),
            update: ActivityUpdate::ReplaceContent {
                summary: "/workspace/src/main.rs".to_owned(),
                detail: Some("/workspace/src/main.rs\n+fn main() {}".to_owned()),
            },
        })
    );

    assert_eq!(
        turn_stream_event(
            "item/mcpToolCall/progress",
            serde_json::json!({
                "threadId": "thread-1",
                "turnId": "turn-2",
                "itemId": "tool-1",
                "message": "Fetching results"
            }),
        )
        .unwrap(),
        Some(ThreadStreamEvent::ActivityUpdated {
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-2".to_owned(),
            item_id: "tool-1".to_owned(),
            update: ActivityUpdate::AppendDetail("Fetching results\n".to_owned()),
        })
    );
}

#[test]
fn infers_lifecycle_status_for_activities_without_a_wire_status() {
    let plan = |method| {
        turn_stream_event(
            method,
            serde_json::json!({
                "threadId": "thread-1",
                "turnId": "turn-2",
                "item": {
                    "id": "plan-1",
                    "type": "plan",
                    "text": "Inspect files"
                }
            }),
        )
        .unwrap()
        .expect("the plan should map")
    };

    let ThreadStreamEvent::ItemStarted {
        item: ThreadItem::Activity(started),
        ..
    } = plan("item/started")
    else {
        panic!("the started plan should remain an activity");
    };
    let ThreadStreamEvent::ItemCompleted {
        item: ThreadItem::Activity(completed),
        ..
    } = plan("item/completed")
    else {
        panic!("the completed plan should remain an activity");
    };

    assert_eq!(started.status, ActivityStatus::InProgress);
    assert_eq!(completed.status, ActivityStatus::Completed);
}

#[test]
fn infers_context_compaction_lifecycle_status() {
    let compaction = |method| {
        turn_stream_event(
            method,
            serde_json::json!({
                "threadId": "thread-1",
                "turnId": "turn-2",
                "item": {
                    "id": "compaction-1",
                    "type": "contextCompaction"
                }
            }),
        )
        .unwrap()
        .expect("the context compaction should map")
    };

    let ThreadStreamEvent::ItemStarted {
        item: ThreadItem::Activity(started),
        ..
    } = compaction("item/started")
    else {
        panic!("the started context compaction should remain an activity");
    };
    let ThreadStreamEvent::ItemCompleted {
        item: ThreadItem::Activity(completed),
        ..
    } = compaction("item/completed")
    else {
        panic!("the completed context compaction should remain an activity");
    };

    assert_eq!(started.status, ActivityStatus::InProgress);
    assert_eq!(completed.status, ActivityStatus::Completed);
}

#[test]
fn serializes_a_text_turn_start_request() {
    assert_eq!(
        serde_json::to_value(
            TurnStartParams::text(
                "thread-1",
                "Continue",
                &inference_state(Some("gpt-5.6-sol"), Some("medium")),
                &TurnOptions::default(),
            )
            .unwrap(),
        )
        .unwrap(),
        serde_json::json!({
            "threadId": "thread-1",
            "input": [{ "type": "text", "text": "Continue" }],
            "collaborationMode": {
                "mode": "default",
                "settings": {
                    "developer_instructions": null,
                    "model": "gpt-5.6-sol",
                    "reasoning_effort": "medium"
                }
            }
        })
    );
}

#[test]
fn serializes_typed_turn_input_in_order() {
    assert_eq!(
        serde_json::to_value(
            TurnStartParams::new(
                "thread-1",
                &[
                    TurnInput::Text("Compare these screenshots".to_owned()),
                    TurnInput::LocalImage {
                        path: PathBuf::from("/workspace/before.png"),
                    },
                    TurnInput::LocalAudio {
                        path: PathBuf::from("/workspace/note.wav"),
                    },
                    TurnInput::Mention {
                        name: "requirements.pdf".to_owned(),
                        path: PathBuf::from("/workspace/requirements.pdf"),
                    },
                ],
                &inference_state(Some("gpt-5.6-sol"), Some("medium")),
                &TurnOptions::default(),
            )
            .unwrap(),
        )
        .unwrap(),
        serde_json::json!({
            "threadId": "thread-1",
            "input": [
                { "type": "text", "text": "Compare these screenshots" },
                { "type": "localImage", "path": "/workspace/before.png" },
                { "type": "localAudio", "path": "/workspace/note.wav" },
                {
                    "type": "mention",
                    "name": "requirements.pdf",
                    "path": "/workspace/requirements.pdf"
                }
            ],
            "collaborationMode": {
                "mode": "default",
                "settings": {
                    "developer_instructions": null,
                    "model": "gpt-5.6-sol",
                    "reasoning_effort": "medium"
                }
            }
        })
    );
}

#[test]
fn rejects_empty_turn_input_before_sending_a_request() {
    let active_inference = inference_state(Some("gpt-5.6-sol"), Some("medium"));

    assert!(matches!(
        TurnStartParams::new("thread-1", &[], &active_inference, &TurnOptions::default(),),
        Err(crate::CodexError::InvalidTurnInput { .. })
    ));
    assert!(matches!(
        TurnStartParams::new(
            "thread-1",
            &[TurnInput::LocalImage {
                path: PathBuf::new(),
            }],
            &active_inference,
            &TurnOptions::default(),
        ),
        Err(crate::CodexError::InvalidTurnInput { .. })
    ));
    assert!(matches!(
        TurnStartParams::new(
            "thread-1",
            &[TurnInput::LocalAudio {
                path: PathBuf::new(),
            }],
            &active_inference,
            &TurnOptions::default(),
        ),
        Err(crate::CodexError::InvalidTurnInput { .. })
    ));
    assert!(matches!(
        TurnStartParams::new(
            "thread-1",
            &[TurnInput::Mention {
                name: " ".to_owned(),
                path: PathBuf::from("/workspace/notes.md"),
            }],
            &active_inference,
            &TurnOptions::default(),
        ),
        Err(crate::CodexError::InvalidTurnInput { .. })
    ));
    assert!(matches!(
        TurnStartParams::new(
            "thread-1",
            &[TurnInput::Mention {
                name: "notes.md".to_owned(),
                path: PathBuf::new(),
            }],
            &active_inference,
            &TurnOptions::default(),
        ),
        Err(crate::CodexError::InvalidTurnInput { .. })
    ));
}

#[test]
fn serializes_a_conversation_model_override_for_this_and_subsequent_turns() {
    assert_eq!(
        serde_json::to_value(
            TurnStartParams::text(
                "thread-1",
                "Continue with the faster model",
                &inference_state(Some("gpt-balanced"), Some("medium")),
                &TurnOptions {
                    inference: Some(InferenceOverride::selection(
                        "gpt-fast",
                        ReasoningEffort::new("low").expect("the reasoning effort is valid"),
                    )),
                    ..TurnOptions::default()
                },
            )
            .unwrap(),
        )
        .unwrap(),
        serde_json::json!({
            "threadId": "thread-1",
            "input": [{ "type": "text", "text": "Continue with the faster model" }],
            "model": "gpt-fast",
            "effort": "low",
            "collaborationMode": {
                "mode": "default",
                "settings": {
                    "developer_instructions": null,
                    "model": "gpt-fast",
                    "reasoning_effort": "low"
                }
            }
        })
    );
}

#[test]
fn serializes_text_guidance_for_the_expected_active_turn() {
    assert_eq!(
        serde_json::to_value(TurnSteerParams::text(
            "thread-1",
            "turn-2",
            "Use the existing test seam",
        ))
        .unwrap(),
        serde_json::json!({
            "threadId": "thread-1",
            "expectedTurnId": "turn-2",
            "input": [{ "type": "text", "text": "Use the existing test seam" }]
        })
    );
}

#[test]
fn serializes_plan_mode_with_interactive_workspace_permissions() {
    assert_eq!(
        serde_json::to_value(
            TurnStartParams::text(
                "thread-1",
                "Plan this change",
                &inference_state(Some("gpt-5.6-sol"), Some("high")),
                &TurnOptions {
                    mode: TurnMode::Plan,
                    permission_preset: TurnPermissionPreset::RequestApproval,
                    ..TurnOptions::default()
                },
            )
            .unwrap(),
        )
        .unwrap(),
        serde_json::json!({
            "threadId": "thread-1",
            "input": [{ "type": "text", "text": "Plan this change" }],
            "collaborationMode": {
                "mode": "plan",
                "settings": {
                    "developer_instructions": null,
                    "model": "gpt-5.6-sol",
                    "reasoning_effort": "high"
                }
            },
            "approvalPolicy": "on-request",
            "approvalsReviewer": "user",
            "sandboxPolicy": {
                "type": "workspaceWrite",
                "networkAccess": false
            }
        })
    );
}

#[test]
fn serializes_read_only_permissions() {
    let options = TurnOptions {
        permission_preset: TurnPermissionPreset::ReadOnly,
        ..TurnOptions::default()
    };
    let active_inference = inference_state(Some("gpt-5.6-sol"), Some("medium"));
    let params =
        TurnStartParams::text("thread-1", "Inspect only", &active_inference, &options).unwrap();
    let value = serde_json::to_value(params).unwrap();

    assert_eq!(value["approvalPolicy"], "on-request");
    assert_eq!(value["approvalsReviewer"], "user");
    assert_eq!(
        value["sandboxPolicy"],
        serde_json::json!({ "type": "readOnly" })
    );
}

#[test]
fn rejects_plan_mode_when_an_older_resume_response_omits_the_model() {
    let options = TurnOptions {
        mode: TurnMode::Plan,
        ..TurnOptions::default()
    };
    let active_inference = ThreadInferenceState::default();
    let result = TurnStartParams::text("thread-1", "Plan this change", &active_inference, &options);

    assert!(matches!(
        result,
        Err(crate::CodexError::UnsupportedTurnControls { .. })
    ));
}

#[test]
fn maps_a_runtime_error_notification() {
    assert_eq!(
        turn_stream_event(
            "error",
            serde_json::json!({
                "threadId": "thread-1",
                "turnId": "turn-2",
                "willRetry": false,
                "error": { "message": "Connection failed" }
            }),
        )
        .unwrap(),
        Some(ThreadStreamEvent::RuntimeError {
            thread_id: "thread-1".to_owned(),
            turn_id: "turn-2".to_owned(),
            message: "Connection failed".to_owned(),
            will_retry: false,
        })
    );
}
