// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

//! In-memory Codex app-server support for integration tests.
//!
//! The fake speaks the same newline-delimited JSON protocol as a real
//! app-server. It intentionally implements product-level behavior instead of
//! exposing a generic sequence-of-responses scripting language.

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::io::{
    AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf, duplex, split,
};
use tokio::task::JoinHandle;
use ward_codex::{
    CodexAppServerConnector, CodexAppServerSource, CodexAppServerTransport, CodexError,
};

const STREAM_CAPACITY: usize = 64 * 1024;
const THREAD_ID: &str = "thread-new";
const LIVE_TURN_ID: &str = "live-turn-1";
const LIVE_USER_ID: &str = "live-user-1";
const LIVE_STEER_USER_ID: &str = "live-steer-user-1";
const LIVE_AGENT_ID: &str = "live-agent-1";
const LIVE_COMMAND_ID: &str = "live-command-1";
const LIVE_TOOL_ID: &str = "live-tool-1";
const COMMAND_APPROVAL_REQUEST_ID: &str = "command-approval-1";
const USER_INPUT_REQUEST_ID: &str = "user-input-1";

/// Mutually exclusive turn behaviors supported by the fake app-server.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FakeTurnScenario {
    /// Complete the turn without waiting for client input.
    #[default]
    Complete,
    /// Keep the turn active until the client supplies guidance.
    WaitForGuidance,
    /// Request approval for a command before completing the turn.
    RequestCommandApproval,
    /// Request structured user input before completing the turn.
    RequestUserInput,
}

/// Observable behaviors supported by the fake app-server.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakeCodexAppServerOptions {
    /// Whether the app-server confirms ephemeral thread starts as ephemeral.
    pub confirm_ephemeral_thread_starts: bool,
    /// Number of initial reads of a newly started thread that report the
    /// app-server's transient `thread not loaded` error.
    pub initial_thread_read_failures: usize,
    /// Whether persisted history assigns different identifiers to the first
    /// live turn and its messages.
    pub renumber_persisted_first_turn: bool,
    /// Behavior exercised by the first turn.
    pub turn_scenario: FakeTurnScenario,
}

impl Default for FakeCodexAppServerOptions {
    fn default() -> Self {
        Self {
            confirm_ephemeral_thread_starts: true,
            initial_thread_read_failures: 0,
            renumber_persisted_first_turn: false,
            turn_scenario: FakeTurnScenario::default(),
        }
    }
}

/// A stateful in-memory Codex app-server shared by independent connections.
pub struct FakeCodexAppServer {
    source: CodexAppServerSource,
}

impl FakeCodexAppServer {
    #[must_use]
    pub fn new(options: FakeCodexAppServerOptions) -> Self {
        let state = Arc::new(Mutex::new(FakeState::new(options)));
        Self {
            source: CodexAppServerSource::with_connector(FakeConnector { state }),
        }
    }

    #[must_use]
    pub fn source(&self) -> CodexAppServerSource {
        self.source.clone()
    }
}

impl Default for FakeCodexAppServer {
    fn default() -> Self {
        Self::new(FakeCodexAppServerOptions::default())
    }
}

struct FakeConnector {
    state: Arc<Mutex<FakeState>>,
}

impl CodexAppServerConnector for FakeConnector {
    fn connect(&self) -> Result<CodexAppServerTransport, CodexError> {
        let (client, server) = duplex(STREAM_CAPACITY);
        let (client_reader, client_writer) = split(client);
        let state = Arc::clone(&self.state);
        let task = tokio::spawn(async move {
            serve_connection(server, state).await;
        });
        Ok(CodexAppServerTransport::new(
            client_reader,
            client_writer,
            move || stop_task(task),
        ))
    }
}

async fn stop_task(task: JoinHandle<()>) {
    task.abort();
    let _ = task.await;
}

struct FakeState {
    options: FakeCodexAppServerOptions,
    thread: Option<FakeThread>,
    remaining_thread_read_failures: usize,
}

impl FakeState {
    fn new(options: FakeCodexAppServerOptions) -> Self {
        Self {
            options,
            thread: None,
            remaining_thread_read_failures: 0,
        }
    }
}

struct FakeThread {
    cwd: String,
    ephemeral: bool,
    turn: Option<FakeTurn>,
}

struct FakeTurn {
    prompt: String,
    guidance: Vec<String>,
    answer: String,
    completed: bool,
}

async fn serve_connection(stream: DuplexStream, state: Arc<Mutex<FakeState>>) {
    let (reader, mut writer) = split(stream);
    let mut lines = BufReader::new(reader).lines();
    if initialize(&mut lines, &mut writer).await.is_err() {
        return;
    }

    while let Ok(Some(line)) = lines.next_line().await {
        let Ok(request) = serde_json::from_str::<Value>(&line) else {
            return;
        };
        let messages = handle_client_message(request, &state);
        for message in messages {
            if write_message(&mut writer, &message).await.is_err() {
                return;
            }
        }
    }
}

async fn initialize(
    lines: &mut tokio::io::Lines<BufReader<ReadHalf<DuplexStream>>>,
    writer: &mut WriteHalf<DuplexStream>,
) -> std::io::Result<()> {
    let Some(line) = lines.next_line().await? else {
        return Ok(());
    };
    let request = serde_json::from_str::<Value>(&line).map_err(std::io::Error::other)?;
    if request.get("method").and_then(Value::as_str) != Some("initialize") {
        return Ok(());
    }
    write_message(
        writer,
        &json!({
            "id": request.get("id").cloned().unwrap_or(Value::Null),
            "result": {
                "codexHome": "/codex-home",
                "platformFamily": "test",
                "platformOs": "test",
                "userAgent": "ward-codex-test-support"
            }
        }),
    )
    .await?;
    let _ = lines.next_line().await?;
    Ok(())
}

async fn write_message(
    writer: &mut WriteHalf<DuplexStream>,
    message: &Value,
) -> std::io::Result<()> {
    let bytes = serde_json::to_vec(message).map_err(std::io::Error::other)?;
    writer.write_all(&bytes).await?;
    writer.write_all(b"\n").await?;
    writer.flush().await
}

fn handle_client_message(message: Value, state: &Arc<Mutex<FakeState>>) -> Vec<Value> {
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return handle_client_response(&message, state);
    };
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
    match method {
        "thread/list" => vec![thread_list_response(id, state)],
        "thread/read" => vec![thread_read_response(id, &params, state)],
        "thread/start" => vec![thread_start_response(id, &params, state)],
        "thread/resume" => vec![thread_resume_response(id, &params, state)],
        "turn/start" => turn_start_messages(id, &params, state),
        "turn/steer" => turn_steer_messages(id, &params, state),
        "turn/interrupt" => vec![json!({ "id": id, "result": {} })],
        _ => vec![json!({
            "id": id,
            "error": { "code": -32601, "message": format!("unsupported fake method: {method}") }
        })],
    }
}

fn handle_client_response(response: &Value, state: &Arc<Mutex<FakeState>>) -> Vec<Value> {
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

    if !complete_active_turn(state, FakeTurnScenario::RequestCommandApproval, answer) {
        return vec![];
    }

    let command = command_item(command_status, command_output);
    let mut messages = vec![];
    if let Some(output) = command_output {
        messages.push(json!({
            "method": "item/commandExecution/outputDelta",
            "params": {
                "threadId": THREAD_ID,
                "turnId": LIVE_TURN_ID,
                "itemId": LIVE_COMMAND_ID,
                "delta": output
            }
        }));
    }
    messages.push(item_notification("item/completed", LIVE_TURN_ID, command));
    messages.extend(turn_completion_messages(answer));
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

    if !complete_active_turn(state, FakeTurnScenario::RequestUserInput, &answer) {
        return vec![];
    }

    turn_completion_messages(&answer)
}

fn complete_active_turn(
    state: &Arc<Mutex<FakeState>>,
    expected_scenario: FakeTurnScenario,
    answer: &str,
) -> bool {
    let mut state = state.lock().unwrap();
    if state.options.turn_scenario != expected_scenario {
        return false;
    }
    let Some(turn) = state
        .thread
        .as_mut()
        .and_then(|thread| thread.turn.as_mut())
        .filter(|turn| !turn.completed)
    else {
        return false;
    };
    turn.answer = answer.to_owned();
    turn.completed = true;
    true
}

fn thread_list_response(id: Value, state: &Arc<Mutex<FakeState>>) -> Value {
    let state = state.lock().unwrap();
    let data = state
        .thread
        .as_ref()
        .map(|thread| vec![thread_json(thread, state.options, true)])
        .unwrap_or_default();
    json!({ "id": id, "result": { "data": data, "nextCursor": null } })
}

fn thread_read_response(id: Value, params: &Value, state: &Arc<Mutex<FakeState>>) -> Value {
    let requested_thread_id = params.get("threadId").and_then(Value::as_str).unwrap_or("");
    let mut state = state.lock().unwrap();
    if requested_thread_id != THREAD_ID || state.thread.is_none() {
        return json!({
            "id": id,
            "error": { "code": -32600, "message": format!("thread not loaded: {requested_thread_id}") }
        });
    }
    if state.remaining_thread_read_failures > 0 {
        state.remaining_thread_read_failures -= 1;
        return json!({
            "id": id,
            "error": { "code": -32600, "message": format!("thread not loaded: {THREAD_ID}") }
        });
    }
    let thread = thread_json(
        state.thread.as_ref().expect("the thread was checked above"),
        state.options,
        true,
    );
    json!({ "id": id, "result": { "thread": thread } })
}

fn thread_start_response(id: Value, params: &Value, state: &Arc<Mutex<FakeState>>) -> Value {
    let cwd = params
        .get("cwd")
        .and_then(Value::as_str)
        .unwrap_or("/workspace")
        .to_owned();
    let requested_ephemeral = params
        .get("ephemeral")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut state = state.lock().unwrap();
    let ephemeral = requested_ephemeral && state.options.confirm_ephemeral_thread_starts;
    state.thread = Some(FakeThread {
        cwd,
        ephemeral,
        turn: None,
    });
    state.remaining_thread_read_failures = state.options.initial_thread_read_failures;
    let thread = thread_json(
        state
            .thread
            .as_ref()
            .expect("the thread was inserted above"),
        state.options,
        false,
    );
    json!({
        "id": id,
        "result": { "model": "gpt-5.6-sol", "thread": thread }
    })
}

fn thread_resume_response(id: Value, params: &Value, state: &Arc<Mutex<FakeState>>) -> Value {
    let requested_thread_id = params.get("threadId").and_then(Value::as_str).unwrap_or("");
    let state = state.lock().unwrap();
    let Some(thread) = state
        .thread
        .as_ref()
        .filter(|_| requested_thread_id == THREAD_ID)
    else {
        return json!({
            "id": id,
            "error": { "code": -32600, "message": format!("thread not loaded: {requested_thread_id}") }
        });
    };
    json!({
        "id": id,
        "result": {
            "model": "gpt-5.6-sol",
            "thread": thread_json(thread, state.options, true)
        }
    })
}

fn turn_start_messages(id: Value, params: &Value, state: &Arc<Mutex<FakeState>>) -> Vec<Value> {
    let prompt = params
        .get("input")
        .and_then(Value::as_array)
        .and_then(|input| input.first())
        .and_then(|input| input.get("text"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let turn_scenario = {
        let mut state = state.lock().unwrap();
        let turn_scenario = state.options.turn_scenario;
        if let Some(thread) = state.thread.as_mut() {
            thread.turn = Some(FakeTurn {
                prompt: prompt.clone(),
                guidance: vec![],
                answer: "Done.".to_owned(),
                completed: turn_scenario == FakeTurnScenario::Complete,
            });
        }
        turn_scenario
    };

    let user = json!({
        "id": LIVE_USER_ID,
        "type": "userMessage",
        "content": [{ "type": "text", "text": prompt }]
    });
    let mut messages = vec![
        json!({
            "id": id,
            "result": { "turn": { "id": LIVE_TURN_ID, "status": "inProgress", "items": [] } }
        }),
        item_notification("item/started", LIVE_TURN_ID, user.clone()),
        item_notification("item/completed", LIVE_TURN_ID, user),
    ];
    match turn_scenario {
        FakeTurnScenario::Complete => messages.extend(turn_completion_messages("Done.")),
        FakeTurnScenario::WaitForGuidance => {}
        FakeTurnScenario::RequestCommandApproval => {
            messages.extend([
                item_notification(
                    "item/started",
                    LIVE_TURN_ID,
                    command_item("inProgress", None),
                ),
                json!({
                    "id": COMMAND_APPROVAL_REQUEST_ID,
                    "method": "item/commandExecution/requestApproval",
                    "params": {
                        "threadId": THREAD_ID,
                        "turnId": LIVE_TURN_ID,
                        "itemId": LIVE_COMMAND_ID,
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
                    "threadId": THREAD_ID,
                    "turnId": LIVE_TURN_ID,
                    "itemId": LIVE_TOOL_ID,
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

fn command_item(status: &str, output: Option<&str>) -> Value {
    let mut command = json!({
        "id": LIVE_COMMAND_ID,
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

fn turn_steer_messages(id: Value, params: &Value, state: &Arc<Mutex<FakeState>>) -> Vec<Value> {
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
    {
        let mut state = state.lock().unwrap();
        let Some(turn) = state
            .thread
            .as_mut()
            .filter(|_| requested_thread_id == THREAD_ID)
            .and_then(|thread| thread.turn.as_mut())
            .filter(|turn| expected_turn_id == LIVE_TURN_ID && !turn.completed)
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
    }

    let user = json!({
        "id": LIVE_STEER_USER_ID,
        "type": "userMessage",
        "content": [{ "type": "text", "text": guidance }]
    });
    let mut messages = vec![
        json!({ "id": id, "result": { "turnId": LIVE_TURN_ID } }),
        item_notification("item/started", LIVE_TURN_ID, user.clone()),
        item_notification("item/completed", LIVE_TURN_ID, user),
    ];
    messages.extend(turn_completion_messages("Adjusted."));
    messages
}

fn turn_completion_messages(answer: &str) -> Vec<Value> {
    let agent_started = json!({
        "id": LIVE_AGENT_ID,
        "type": "agentMessage",
        "text": "",
        "phase": "final_answer"
    });
    let agent_completed = json!({
        "id": LIVE_AGENT_ID,
        "type": "agentMessage",
        "text": answer,
        "phase": "final_answer"
    });
    vec![
        item_notification("item/started", LIVE_TURN_ID, agent_started),
        json!({
            "method": "item/agentMessage/delta",
            "params": {
                "threadId": THREAD_ID,
                "turnId": LIVE_TURN_ID,
                "itemId": LIVE_AGENT_ID,
                "delta": answer
            }
        }),
        item_notification("item/completed", LIVE_TURN_ID, agent_completed),
        json!({
            "method": "turn/completed",
            "params": {
                "threadId": THREAD_ID,
                "turn": { "id": LIVE_TURN_ID, "status": "completed", "items": [] }
            }
        }),
    ]
}

fn item_notification(method: &str, turn_id: &str, item: Value) -> Value {
    json!({
        "method": method,
        "params": {
            "threadId": THREAD_ID,
            "turnId": turn_id,
            "item": item
        }
    })
}

fn thread_json(thread: &FakeThread, options: FakeCodexAppServerOptions, persisted: bool) -> Value {
    let turns = thread
        .turn
        .as_ref()
        .map(|turn| vec![turn_json(turn, options, persisted)])
        .unwrap_or_default();
    let status = if thread.turn.as_ref().is_some_and(|turn| !turn.completed) {
        json!({ "type": "active", "activeFlags": [] })
    } else {
        json!({ "type": "idle" })
    };
    json!({
        "id": THREAD_ID,
        "name": null,
        "preview": thread.turn.as_ref().map(|turn| turn.prompt.as_str()).unwrap_or(""),
        "cwd": thread.cwd,
        "createdAt": 10,
        "updatedAt": if thread.turn.is_some() { 20 } else { 10 },
        "ephemeral": thread.ephemeral,
        "status": status,
        "turns": turns
    })
}

fn turn_json(turn: &FakeTurn, options: FakeCodexAppServerOptions, persisted: bool) -> Value {
    let renumber = persisted && options.renumber_persisted_first_turn;
    let turn_id = if renumber {
        "persisted-turn-1"
    } else {
        LIVE_TURN_ID
    };
    let user_id = if renumber {
        "persisted-user-1"
    } else {
        LIVE_USER_ID
    };
    let agent_id = if renumber {
        "persisted-agent-1"
    } else {
        LIVE_AGENT_ID
    };
    let steer_user_id = if renumber {
        "persisted-steer-user-1"
    } else {
        LIVE_STEER_USER_ID
    };
    let mut items = vec![json!({
        "id": user_id,
        "type": "userMessage",
        "content": [{ "type": "text", "text": turn.prompt }]
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
