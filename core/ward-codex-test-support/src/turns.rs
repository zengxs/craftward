// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::catalog::fake_model;
use super::state::{FakeState, FakeTurn};
use super::{FakeCodexAppServerOptions, FakeTurnScenario};

const LIVE_TURN_ID: &str = "live-turn-1";
const LIVE_USER_ID: &str = "live-user-1";
const LIVE_STEER_USER_ID: &str = "live-steer-user-1";
const LIVE_AGENT_ID: &str = "live-agent-1";
const LIVE_COMMAND_ID: &str = "live-command-1";
const LIVE_TOOL_ID: &str = "live-tool-1";
const COMMAND_APPROVAL_REQUEST_ID: &str = "command-approval-1";
const USER_INPUT_REQUEST_ID: &str = "user-input-1";

pub(super) fn handle_client_response(
    response: &Value,
    state: &Arc<Mutex<FakeState>>,
) -> Vec<Value> {
    if response.get("id").and_then(Value::as_str) == Some(USER_INPUT_REQUEST_ID) {
        return handle_user_input_response(response, state);
    }
    if response.get("id").and_then(Value::as_str) != Some(COMMAND_APPROVAL_REQUEST_ID) {
        return vec![];
    }
    let (answer, command_status, command_output) =
        match response.pointer("/result/decision").and_then(Value::as_str) {
            Some("accept") => ("Command approved.", "completed", Some("/workspace\n")),
            Some("decline") => ("Command declined.", "declined", None),
            _ => return vec![],
        };

    let Some(active_turn) =
        complete_active_turn(state, FakeTurnScenario::RequestCommandApproval, answer)
    else {
        return vec![];
    };

    let command = command_item(active_turn.number, command_status, command_output);
    let mut messages = vec![];
    if let Some(output) = command_output {
        messages.push(json!({
            "method": "item/commandExecution/outputDelta",
            "params": {
                "threadId": active_turn.thread_id,
                "turnId": active_turn.turn_id,
                "itemId": active_turn.command_id,
                "delta": output
            }
        }));
    }
    messages.push(item_notification(
        "item/completed",
        &active_turn.thread_id,
        &active_turn.turn_id,
        command,
    ));
    messages.extend(turn_completion_messages(
        &active_turn.thread_id,
        active_turn.number,
        answer,
    ));
    messages
}

fn handle_user_input_response(response: &Value, state: &Arc<Mutex<FakeState>>) -> Vec<Value> {
    let Some(scope) = response
        .pointer("/result/answers/scope/answers/0")
        .and_then(Value::as_str)
    else {
        return vec![];
    };
    let Some(note) = response
        .pointer("/result/answers/note/answers/0")
        .and_then(Value::as_str)
    else {
        return vec![];
    };
    let answer = format!("Scope: {scope}; note: {note}.");

    let Some(active_turn) =
        complete_active_turn(state, FakeTurnScenario::RequestUserInput, &answer)
    else {
        return vec![];
    };

    turn_completion_messages(&active_turn.thread_id, active_turn.number, &answer)
}

struct ActiveTurn {
    thread_id: String,
    turn_id: String,
    command_id: String,
    number: usize,
}

fn complete_active_turn(
    state: &Arc<Mutex<FakeState>>,
    expected_scenario: FakeTurnScenario,
    answer: &str,
) -> Option<ActiveTurn> {
    let mut state = state.lock().unwrap();
    if state.options.turn_scenario != expected_scenario {
        return None;
    }
    state.threads.iter_mut().find_map(|thread| {
        let turn = thread.turns.iter_mut().rev().find(|turn| !turn.completed)?;
        turn.answer = answer.to_owned();
        turn.completed = true;
        Some(ActiveTurn {
            thread_id: thread.id.clone(),
            turn_id: live_turn_id(turn.number),
            command_id: live_command_id(turn.number),
            number: turn.number,
        })
    })
}

pub(super) fn turn_start_messages(
    id: Value,
    params: &Value,
    state: &Arc<Mutex<FakeState>>,
    connection_id: u64,
) -> Vec<Value> {
    let requested_thread_id = params
        .get("threadId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let input = params
        .get("input")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let prompt = input
        .iter()
        .find(|input| input.get("type").and_then(Value::as_str) == Some("text"))
        .and_then(|input| input.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let selected_model = params
        .get("model")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let selected_reasoning_effort = params
        .get("effort")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let (turn_scenario, turn_number) = {
        let mut state = state.lock().unwrap();
        let turn_scenario = state.options.turn_scenario;
        let Some(thread) = state.threads.iter_mut().find(|thread| {
            thread.id == requested_thread_id && thread.writer_connection_id == Some(connection_id)
        }) else {
            return vec![json!({
                "id": id,
                "error": {
                    "code": -32600,
                    "message": format!("thread not loaded: {requested_thread_id}")
                }
            })];
        };
        let effective_model = selected_model
            .as_deref()
            .unwrap_or(&thread.model)
            .to_owned();
        let Some(model_definition) = fake_model(&effective_model) else {
            return vec![json!({
                "id": id,
                "error": {
                    "code": -32602,
                    "message": format!("unsupported model: {effective_model}")
                }
            })];
        };
        let effective_reasoning_effort = selected_reasoning_effort
            .as_deref()
            .or_else(|| {
                model_definition
                    .supports_reasoning_effort(&thread.reasoning_effort)
                    .then_some(thread.reasoning_effort.as_str())
            })
            .unwrap_or(model_definition.default_reasoning_effort)
            .to_owned();
        if !model_definition.supports_reasoning_effort(&effective_reasoning_effort) {
            return vec![json!({
                "id": id,
                "error": {
                    "code": -32602,
                    "message": format!(
                        "unsupported reasoning effort for {effective_model}: {effective_reasoning_effort}"
                    )
                }
            })];
        }
        if let Some(model) = selected_model {
            thread.model = model;
        }
        thread.reasoning_effort = effective_reasoning_effort;
        let turn_number = thread.turns.len() + 1;
        thread.turns.push(FakeTurn {
            number: turn_number,
            prompt: prompt.clone(),
            input: input.clone(),
            guidance: vec![],
            answer: "Done.".to_owned(),
            completed: turn_scenario == FakeTurnScenario::Complete,
        });
        (turn_scenario, turn_number)
    };

    let turn_id = live_turn_id(turn_number);
    let user_id = live_user_id(turn_number);
    let user = json!({
        "id": user_id,
        "type": "userMessage",
        "content": input
    });
    let mut messages = vec![
        json!({
            "id": id,
            "result": { "turn": { "id": turn_id, "status": "inProgress", "items": [] } }
        }),
        item_notification("item/started", &requested_thread_id, &turn_id, user.clone()),
        item_notification("item/completed", &requested_thread_id, &turn_id, user),
    ];
    match turn_scenario {
        FakeTurnScenario::Complete => messages.extend(turn_completion_messages(
            &requested_thread_id,
            turn_number,
            "Done.",
        )),
        FakeTurnScenario::WaitForGuidance => {}
        FakeTurnScenario::RequestCommandApproval => {
            messages.extend([
                item_notification(
                    "item/started",
                    &requested_thread_id,
                    &turn_id,
                    command_item(turn_number, "inProgress", None),
                ),
                json!({
                    "id": COMMAND_APPROVAL_REQUEST_ID,
                    "method": "item/commandExecution/requestApproval",
                    "params": {
                        "threadId": requested_thread_id,
                        "turnId": turn_id,
                        "itemId": live_command_id(turn_number),
                        "command": "pwd",
                        "cwd": "/workspace",
                        "reason": "Verify the workspace",
                        "startedAtMs": 42
                    }
                }),
            ]);
        }
        FakeTurnScenario::RequestUserInput => {
            messages.push(json!({
                "id": USER_INPUT_REQUEST_ID,
                "method": "item/tool/requestUserInput",
                "params": {
                    "threadId": requested_thread_id,
                    "turnId": turn_id,
                    "itemId": live_tool_id(turn_number),
                    "isBlocking": true,
                    "questions": [
                        {
                            "id": "scope",
                            "header": "Scope",
                            "question": "Which scope should be used?",
                            "options": [
                                {
                                    "label": "Current",
                                    "description": "Only the current turn"
                                },
                                {
                                    "label": "All",
                                    "description": "The whole conversation"
                                }
                            ],
                            "isOther": true,
                            "isSecret": false
                        },
                        {
                            "id": "note",
                            "header": "Note",
                            "question": "What should Codex remember?",
                            "options": [],
                            "isOther": false,
                            "isSecret": false
                        }
                    ]
                }
            }));
        }
    }
    messages
}

fn command_item(turn_number: usize, status: &str, output: Option<&str>) -> Value {
    let mut command = json!({
        "id": live_command_id(turn_number),
        "type": "commandExecution",
        "command": "pwd",
        "commandActions": [],
        "cwd": "/workspace",
        "status": status
    });
    if let Some(output) = output {
        command["aggregatedOutput"] = json!(output);
    }
    command
}

pub(super) fn turn_steer_messages(
    id: Value,
    params: &Value,
    state: &Arc<Mutex<FakeState>>,
    connection_id: u64,
) -> Vec<Value> {
    let requested_thread_id = params.get("threadId").and_then(Value::as_str).unwrap_or("");
    let expected_turn_id = params
        .get("expectedTurnId")
        .and_then(Value::as_str)
        .unwrap_or("");
    let guidance = params
        .get("input")
        .and_then(Value::as_array)
        .and_then(|input| input.first())
        .and_then(|input| input.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let turn_number = {
        let mut state = state.lock().unwrap();
        let Some(turn) = state
            .threads
            .iter_mut()
            .find(|thread| {
                thread.id == requested_thread_id
                    && thread.writer_connection_id == Some(connection_id)
            })
            .and_then(|thread| thread.turns.iter_mut().rev().find(|turn| !turn.completed))
            .filter(|turn| expected_turn_id == live_turn_id(turn.number))
        else {
            return vec![json!({
                "id": id,
                "error": {
                    "code": -32600,
                    "message": format!("turn is not active: {expected_turn_id}")
                }
            })];
        };
        turn.guidance.push(guidance.clone());
        turn.answer = "Adjusted.".to_owned();
        turn.completed = true;
        turn.number
    };

    let turn_id = live_turn_id(turn_number);
    let user = json!({
        "id": live_steer_user_id(turn_number),
        "type": "userMessage",
        "content": [{ "type": "text", "text": guidance }]
    });
    let mut messages = vec![
        json!({ "id": id, "result": { "turnId": turn_id } }),
        item_notification("item/started", requested_thread_id, &turn_id, user.clone()),
        item_notification("item/completed", requested_thread_id, &turn_id, user),
    ];
    messages.extend(turn_completion_messages(
        requested_thread_id,
        turn_number,
        "Adjusted.",
    ));
    messages
}

fn turn_completion_messages(thread_id: &str, turn_number: usize, answer: &str) -> Vec<Value> {
    let turn_id = live_turn_id(turn_number);
    let agent_id = live_agent_id(turn_number);
    let agent_started = json!({
        "id": agent_id,
        "type": "agentMessage",
        "text": "",
        "phase": "final_answer"
    });
    let agent_completed = json!({
        "id": agent_id,
        "type": "agentMessage",
        "text": answer,
        "phase": "final_answer"
    });
    vec![
        item_notification("item/started", thread_id, &turn_id, agent_started),
        json!({
            "method": "item/agentMessage/delta",
            "params": {
                "threadId": thread_id,
                "turnId": turn_id,
                "itemId": agent_id,
                "delta": answer
            }
        }),
        item_notification("item/completed", thread_id, &turn_id, agent_completed),
        json!({
            "method": "turn/completed",
            "params": {
                "threadId": thread_id,
                "turn": { "id": turn_id, "status": "completed", "items": [] }
            }
        }),
    ]
}

fn item_notification(method: &str, thread_id: &str, turn_id: &str, item: Value) -> Value {
    json!({
        "method": method,
        "params": {
            "threadId": thread_id,
            "turnId": turn_id,
            "item": item
        }
    })
}

pub(super) fn turn_json(
    turn: &FakeTurn,
    options: FakeCodexAppServerOptions,
    persisted: bool,
) -> Value {
    let renumber = persisted && options.renumber_persisted_first_turn && turn.number == 1;
    let turn_id = if renumber {
        persisted_turn_id(turn, options)
    } else {
        live_turn_id(turn.number)
    };
    let user_id = if renumber {
        "persisted-user-1".to_owned()
    } else {
        live_user_id(turn.number)
    };
    let agent_id = if renumber {
        "persisted-agent-1".to_owned()
    } else {
        live_agent_id(turn.number)
    };
    let steer_user_id = if renumber {
        "persisted-steer-user-1".to_owned()
    } else {
        live_steer_user_id(turn.number)
    };
    let mut items = vec![json!({
        "id": user_id,
        "type": "userMessage",
        "content": turn.input
    })];
    items.extend(turn.guidance.iter().map(|guidance| {
        json!({
            "id": steer_user_id,
            "type": "userMessage",
            "content": [{ "type": "text", "text": guidance }]
        })
    }));
    if turn.completed {
        items.push(json!({
            "id": agent_id,
            "type": "agentMessage",
            "text": turn.answer,
            "phase": "final_answer"
        }));
    }
    json!({
        "id": turn_id,
        "status": if turn.completed { "completed" } else { "inProgress" },
        "items": items
    })
}

pub(super) fn persisted_turn_id(turn: &FakeTurn, options: FakeCodexAppServerOptions) -> String {
    if options.renumber_persisted_first_turn && turn.number == 1 {
        "persisted-turn-1".to_owned()
    } else {
        live_turn_id(turn.number)
    }
}

fn live_turn_id(turn_number: usize) -> String {
    live_id(turn_number, LIVE_TURN_ID, "live-turn")
}

fn live_user_id(turn_number: usize) -> String {
    live_id(turn_number, LIVE_USER_ID, "live-user")
}

fn live_steer_user_id(turn_number: usize) -> String {
    live_id(turn_number, LIVE_STEER_USER_ID, "live-steer-user")
}

fn live_agent_id(turn_number: usize) -> String {
    live_id(turn_number, LIVE_AGENT_ID, "live-agent")
}

fn live_command_id(turn_number: usize) -> String {
    live_id(turn_number, LIVE_COMMAND_ID, "live-command")
}

fn live_tool_id(turn_number: usize) -> String {
    live_id(turn_number, LIVE_TOOL_ID, "live-tool")
}

fn live_id(turn_number: usize, first_id: &str, prefix: &str) -> String {
    if turn_number == 1 {
        first_id.to_owned()
    } else {
        format!("{prefix}-{turn_number}")
    }
}
