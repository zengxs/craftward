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
const SECOND_MODEL_PAGE_CURSOR: &str = "models-page-2";

#[derive(Clone, Copy, Eq, PartialEq)]
enum FakeModelPage {
    First,
    Second,
}

struct FakeReasoningEffort {
    value: &'static str,
    description: &'static str,
}

struct FakeModelDefinition {
    id: &'static str,
    model: &'static str,
    display_name: &'static str,
    description: &'static str,
    hidden: bool,
    is_default: bool,
    default_reasoning_effort: &'static str,
    supported_reasoning_efforts: &'static [FakeReasoningEffort],
    page: FakeModelPage,
}

impl FakeModelDefinition {
    fn supports_reasoning_effort(&self, effort: &str) -> bool {
        self.supported_reasoning_efforts
            .iter()
            .any(|option| option.value == effort)
    }

    fn to_json(&self) -> Value {
        let supported_reasoning_efforts = self
            .supported_reasoning_efforts
            .iter()
            .map(|option| {
                json!({
                    "reasoningEffort": option.value,
                    "description": option.description,
                })
            })
            .collect::<Vec<_>>();
        json!({
            "id": self.id,
            "model": self.model,
            "displayName": self.display_name,
            "description": self.description,
            "hidden": self.hidden,
            "isDefault": self.is_default,
            "defaultReasoningEffort": self.default_reasoning_effort,
            "supportedReasoningEfforts": supported_reasoning_efforts,
        })
    }
}

const BALANCED_REASONING_EFFORTS: &[FakeReasoningEffort] = &[
    FakeReasoningEffort {
        value: "low",
        description: "Faster responses",
    },
    FakeReasoningEffort {
        value: "medium",
        description: "Balanced reasoning",
    },
    FakeReasoningEffort {
        value: "high",
        description: "Deeper reasoning",
    },
];
const FAST_REASONING_EFFORTS: &[FakeReasoningEffort] = &[
    FakeReasoningEffort {
        value: "low",
        description: "Faster responses",
    },
    FakeReasoningEffort {
        value: "medium",
        description: "Balanced reasoning",
    },
];
const INTERNAL_REASONING_EFFORTS: &[FakeReasoningEffort] = &[FakeReasoningEffort {
    value: "medium",
    description: "Balanced reasoning",
}];
const FAKE_MODELS: &[FakeModelDefinition] = &[
    FakeModelDefinition {
        id: "balanced",
        model: "gpt-balanced",
        display_name: "Balanced",
        description: "Balances capability and speed.",
        hidden: false,
        is_default: true,
        default_reasoning_effort: "medium",
        supported_reasoning_efforts: BALANCED_REASONING_EFFORTS,
        page: FakeModelPage::First,
    },
    FakeModelDefinition {
        id: "internal",
        model: "gpt-internal",
        display_name: "Internal",
        description: "Hidden test model.",
        hidden: true,
        is_default: false,
        default_reasoning_effort: "medium",
        supported_reasoning_efforts: INTERNAL_REASONING_EFFORTS,
        page: FakeModelPage::First,
    },
    FakeModelDefinition {
        id: "fast",
        model: "gpt-fast",
        display_name: "Fast",
        description: "Optimized for quick iteration.",
        hidden: false,
        is_default: false,
        default_reasoning_effort: "low",
        supported_reasoning_efforts: FAST_REASONING_EFFORTS,
        page: FakeModelPage::Second,
    },
];

fn fake_model(model: &str) -> Option<&'static FakeModelDefinition> {
    FAKE_MODELS
        .iter()
        .find(|definition| definition.model == model)
}

fn default_fake_model() -> &'static FakeModelDefinition {
    FAKE_MODELS
        .iter()
        .find(|model| model.is_default)
        .expect("the fake model catalog must declare a default")
}

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
    /// Whether the first fork is applied before its connection closes without
    /// returning the mutation response.
    pub lose_first_fork_response: bool,
    /// Number of initial model-list requests that return a temporary error.
    pub model_list_failures: usize,
    /// Behavior exercised by each started turn.
    pub turn_scenario: FakeTurnScenario,
}

impl Default for FakeCodexAppServerOptions {
    fn default() -> Self {
        Self {
            confirm_ephemeral_thread_starts: true,
            initial_thread_read_failures: 0,
            renumber_persisted_first_turn: false,
            lose_first_fork_response: false,
            model_list_failures: 0,
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
        let connection_id = {
            let mut state = state.lock().unwrap();
            let connection_id = state.next_connection_id;
            state.next_connection_id += 1;
            connection_id
        };
        let task = tokio::spawn(async move {
            serve_connection(server, state, connection_id).await;
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
    threads: Vec<FakeThread>,
    next_fork_number: usize,
    next_connection_id: u64,
}

impl FakeState {
    fn new(options: FakeCodexAppServerOptions) -> Self {
        Self {
            options,
            threads: vec![],
            next_fork_number: 1,
            next_connection_id: 1,
        }
    }
}

#[derive(Clone)]
struct FakeThread {
    id: String,
    cwd: String,
    model: String,
    reasoning_effort: String,
    ephemeral: bool,
    archived: bool,
    name: Option<String>,
    turns: Vec<FakeTurn>,
    remaining_read_failures: usize,
    writer_connection_id: Option<u64>,
}

#[derive(Clone)]
struct FakeTurn {
    number: usize,
    prompt: String,
    guidance: Vec<String>,
    answer: String,
    completed: bool,
}

struct FakeConnectionLease {
    state: Arc<Mutex<FakeState>>,
    connection_id: u64,
}

impl Drop for FakeConnectionLease {
    fn drop(&mut self) {
        let mut state = self.state.lock().unwrap();
        for thread in &mut state.threads {
            if thread.writer_connection_id == Some(self.connection_id) {
                thread.writer_connection_id = None;
            }
        }
    }
}

async fn serve_connection(stream: DuplexStream, state: Arc<Mutex<FakeState>>, connection_id: u64) {
    let _lease = FakeConnectionLease {
        state: Arc::clone(&state),
        connection_id,
    };
    let (reader, mut writer) = split(stream);
    let mut lines = BufReader::new(reader).lines();
    if initialize(&mut lines, &mut writer).await.is_err() {
        return;
    }

    while let Ok(Some(line)) = lines.next_line().await {
        let Ok(request) = serde_json::from_str::<Value>(&line) else {
            return;
        };
        let Some(messages) = handle_client_message(request, &state, connection_id) else {
            return;
        };
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

fn handle_client_message(
    message: Value,
    state: &Arc<Mutex<FakeState>>,
    connection_id: u64,
) -> Option<Vec<Value>> {
    let id = message.get("id").cloned().unwrap_or(Value::Null);
    let Some(method) = message.get("method").and_then(Value::as_str) else {
        return Some(handle_client_response(&message, state));
    };
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
    Some(match method {
        "model/list" => vec![model_list_response(id, &params, state)],
        "thread/archive" => vec![thread_archive_response(id, &params, state)],
        "thread/fork" => {
            return thread_fork_response(id, &params, state, connection_id)
                .map(|response| vec![response]);
        }
        "thread/list" => vec![thread_list_response(id, &params, state)],
        "thread/read" => vec![thread_read_response(id, &params, state)],
        "thread/start" => vec![thread_start_response(id, &params, state, connection_id)],
        "thread/resume" => vec![thread_resume_response(id, &params, state, connection_id)],
        "thread/name/set" => vec![thread_set_name_response(id, &params, state)],
        "thread/unarchive" => vec![thread_unarchive_response(id, &params, state)],
        "turn/start" => turn_start_messages(id, &params, state, connection_id),
        "turn/steer" => turn_steer_messages(id, &params, state, connection_id),
        "turn/interrupt" => vec![json!({ "id": id, "result": {} })],
        _ => vec![json!({
            "id": id,
            "error": { "code": -32601, "message": format!("unsupported fake method: {method}") }
        })],
    })
}

fn model_list_response(id: Value, params: &Value, state: &Arc<Mutex<FakeState>>) -> Value {
    let mut state = state.lock().unwrap();
    if state.options.model_list_failures > 0 {
        state.options.model_list_failures -= 1;
        return json!({
            "id": id,
            "error": {
                "code": -32603,
                "message": "the model catalog is temporarily unavailable"
            }
        });
    }
    drop(state);

    let (page, next_cursor) = match params.get("cursor").and_then(Value::as_str) {
        None => (FakeModelPage::First, Some(SECOND_MODEL_PAGE_CURSOR)),
        Some(SECOND_MODEL_PAGE_CURSOR) => (FakeModelPage::Second, None),
        Some(cursor) => {
            return json!({
                "id": id,
                "error": {
                    "code": -32600,
                    "message": format!("unknown model-list cursor: {cursor}")
                }
            });
        }
    };
    let include_hidden = params
        .get("includeHidden")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let data = FAKE_MODELS
        .iter()
        .filter(|model| model.page == page && (include_hidden || !model.hidden))
        .map(FakeModelDefinition::to_json)
        .collect::<Vec<_>>();
    json!({
        "id": id,
        "result": {
            "data": data,
            "nextCursor": next_cursor,
        }
    })
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

fn requested_thread_mut<'a>(
    response_id: &Value,
    params: &Value,
    state: &'a mut FakeState,
) -> Result<&'a mut FakeThread, Value> {
    let requested_thread_id = params.get("threadId").and_then(Value::as_str).unwrap_or("");
    state
        .threads
        .iter_mut()
        .find(|thread| thread.id == requested_thread_id)
        .ok_or_else(|| {
            json!({
                "id": response_id,
                "error": { "code": -32600, "message": format!("thread not loaded: {requested_thread_id}") }
            })
        })
}

fn thread_archive_response(id: Value, params: &Value, state: &Arc<Mutex<FakeState>>) -> Value {
    let mut state = state.lock().unwrap();
    let thread = match requested_thread_mut(&id, params, &mut state) {
        Ok(thread) => thread,
        Err(response) => return response,
    };
    thread.archived = true;
    json!({ "id": id, "result": {} })
}

fn thread_fork_response(
    id: Value,
    params: &Value,
    state: &Arc<Mutex<FakeState>>,
    connection_id: u64,
) -> Option<Value> {
    let requested_thread_id = params.get("threadId").and_then(Value::as_str).unwrap_or("");
    let mut state = state.lock().unwrap();
    let Some(source) = state
        .threads
        .iter()
        .find(|thread| thread.id == requested_thread_id && !thread.archived)
        .cloned()
    else {
        return Some(json!({
            "id": id,
            "error": { "code": -32600, "message": format!("thread not loaded: {requested_thread_id}") }
        }));
    };
    let last_turn_id = params.get("lastTurnId").and_then(Value::as_str);
    let mut fork = source;
    if let Some(last_turn_id) = last_turn_id {
        let Some(last_turn_index) = fork.turns.iter().position(|turn| {
            persisted_turn_id(turn, state.options) == last_turn_id && turn.completed
        }) else {
            return Some(json!({
                "id": id,
                "error": {
                    "code": -32600,
                    "message": format!("turn not loaded or still in progress: {last_turn_id}")
                }
            }));
        };
        fork.turns.truncate(last_turn_index + 1);
    }
    let fork_id = format!("thread-fork-{}", state.next_fork_number);
    state.next_fork_number += 1;
    fork.id = fork_id;
    fork.remaining_read_failures = 0;
    fork.writer_connection_id = Some(connection_id);
    state.threads.push(fork);
    if state.options.lose_first_fork_response {
        state.options.lose_first_fork_response = false;
        return None;
    }
    let fork = state.threads.last().expect("the fork was inserted above");
    let model = fork.model.clone();
    let reasoning_effort = fork.reasoning_effort.clone();
    let thread = thread_json(fork, state.options, true);
    Some(json!({
        "id": id,
        "result": {
            "model": model,
            "reasoningEffort": reasoning_effort,
            "thread": thread
        }
    }))
}

fn thread_list_response(id: Value, params: &Value, state: &Arc<Mutex<FakeState>>) -> Value {
    let state = state.lock().unwrap();
    let archived = params
        .get("archived")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let data = state
        .threads
        .iter()
        .filter(|thread| !thread.ephemeral && thread.archived == archived)
        .map(|thread| thread_json(thread, state.options, true))
        .collect::<Vec<_>>();
    json!({ "id": id, "result": { "data": data, "nextCursor": null } })
}

fn thread_read_response(id: Value, params: &Value, state: &Arc<Mutex<FakeState>>) -> Value {
    let requested_thread_id = params.get("threadId").and_then(Value::as_str).unwrap_or("");
    let mut state = state.lock().unwrap();
    let Some(thread_index) = state
        .threads
        .iter()
        .position(|thread| thread.id == requested_thread_id)
    else {
        return json!({
            "id": id,
            "error": { "code": -32600, "message": format!("thread not loaded: {requested_thread_id}") }
        });
    };
    if state.threads[thread_index].remaining_read_failures > 0 {
        state.threads[thread_index].remaining_read_failures -= 1;
        return json!({
            "id": id,
            "error": { "code": -32600, "message": format!("thread not loaded: {requested_thread_id}") }
        });
    }
    let thread = thread_json(&state.threads[thread_index], state.options, true);
    json!({ "id": id, "result": { "thread": thread } })
}

fn thread_set_name_response(id: Value, params: &Value, state: &Arc<Mutex<FakeState>>) -> Value {
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return json!({
            "id": id,
            "error": { "code": -32600, "message": "the thread name is missing" }
        });
    };
    let mut state = state.lock().unwrap();
    let thread = match requested_thread_mut(&id, params, &mut state) {
        Ok(thread) => thread,
        Err(response) => return response,
    };
    thread.name = Some(name.to_owned());
    json!({ "id": id, "result": {} })
}

fn thread_unarchive_response(id: Value, params: &Value, state: &Arc<Mutex<FakeState>>) -> Value {
    let mut state = state.lock().unwrap();
    let options = state.options;
    let thread = match requested_thread_mut(&id, params, &mut state) {
        Ok(thread) => thread,
        Err(response) => return response,
    };
    thread.archived = false;
    let thread = thread_json(thread, options, true);
    json!({ "id": id, "result": { "thread": thread } })
}

fn thread_start_response(
    id: Value,
    params: &Value,
    state: &Arc<Mutex<FakeState>>,
    connection_id: u64,
) -> Value {
    let cwd = params
        .get("cwd")
        .and_then(Value::as_str)
        .unwrap_or("/workspace")
        .to_owned();
    let requested_ephemeral = params
        .get("ephemeral")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let model = params
        .get("model")
        .and_then(Value::as_str)
        .unwrap_or(default_fake_model().model)
        .to_owned();
    let reasoning_effort = params
        .get("effort")
        .and_then(Value::as_str)
        .or_else(|| fake_model(&model).map(|model| model.default_reasoning_effort))
        .unwrap_or(default_fake_model().default_reasoning_effort)
        .to_owned();
    let mut state = state.lock().unwrap();
    let ephemeral = requested_ephemeral && state.options.confirm_ephemeral_thread_starts;
    state.threads.retain(|thread| thread.id != THREAD_ID);
    let initial_read_failures = state.options.initial_thread_read_failures;
    state.threads.push(FakeThread {
        id: THREAD_ID.to_owned(),
        cwd,
        model,
        reasoning_effort,
        ephemeral,
        archived: false,
        name: None,
        turns: vec![],
        remaining_read_failures: initial_read_failures,
        writer_connection_id: Some(connection_id),
    });
    let thread = thread_json(
        state
            .threads
            .iter()
            .find(|thread| thread.id == THREAD_ID)
            .expect("the thread was inserted above"),
        state.options,
        false,
    );
    let model = state
        .threads
        .iter()
        .find(|thread| thread.id == THREAD_ID)
        .expect("the thread was inserted above")
        .model
        .clone();
    let reasoning_effort = state
        .threads
        .iter()
        .find(|thread| thread.id == THREAD_ID)
        .expect("the thread was inserted above")
        .reasoning_effort
        .clone();
    json!({
        "id": id,
        "result": {
            "model": model,
            "reasoningEffort": reasoning_effort,
            "thread": thread
        }
    })
}

fn thread_resume_response(
    id: Value,
    params: &Value,
    state: &Arc<Mutex<FakeState>>,
    connection_id: u64,
) -> Value {
    let requested_thread_id = params.get("threadId").and_then(Value::as_str).unwrap_or("");
    let mut state = state.lock().unwrap();
    let options = state.options;
    let Some(thread) = state
        .threads
        .iter_mut()
        .find(|thread| thread.id == requested_thread_id)
    else {
        return json!({
            "id": id,
            "error": { "code": -32600, "message": format!("thread not loaded: {requested_thread_id}") }
        });
    };
    if thread
        .writer_connection_id
        .is_some_and(|owner| owner != connection_id)
    {
        return json!({
            "id": id,
            "error": {
                "code": -32600,
                "message": format!("thread {requested_thread_id} already has an active writer")
            }
        });
    }
    thread.writer_connection_id = Some(connection_id);
    let model = thread.model.clone();
    let reasoning_effort = thread.reasoning_effort.clone();
    json!({
        "id": id,
        "result": {
            "model": model,
            "reasoningEffort": reasoning_effort,
            "thread": thread_json(thread, options, true)
        }
    })
}

fn turn_start_messages(
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
    let prompt = params
        .get("input")
        .and_then(Value::as_array)
        .and_then(|input| input.first())
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
        "content": [{ "type": "text", "text": prompt }]
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

fn turn_steer_messages(
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

fn thread_json(thread: &FakeThread, options: FakeCodexAppServerOptions, persisted: bool) -> Value {
    let turns = thread
        .turns
        .iter()
        .map(|turn| turn_json(turn, options, persisted))
        .collect::<Vec<_>>();
    let status = if thread.turns.last().is_some_and(|turn| !turn.completed) {
        json!({ "type": "active", "activeFlags": [] })
    } else {
        json!({ "type": "idle" })
    };
    json!({
        "id": thread.id,
        "name": thread.name,
        "preview": thread.turns.last().map(|turn| turn.prompt.as_str()).unwrap_or(""),
        "cwd": thread.cwd,
        "createdAt": 10,
        "updatedAt": if thread.turns.is_empty() { 10 } else { 19 + thread.turns.len() },
        "ephemeral": thread.ephemeral,
        "status": status,
        "turns": turns
    })
}

fn turn_json(turn: &FakeTurn, options: FakeCodexAppServerOptions, persisted: bool) -> Value {
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

fn persisted_turn_id(turn: &FakeTurn, options: FakeCodexAppServerOptions) -> String {
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
