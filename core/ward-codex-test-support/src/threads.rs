// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::catalog::{default_fake_model, fake_model};
use super::state::{FakeState, FakeThread};
use super::turns::{persisted_turn_id, turn_json};
use super::{FakeCodexAppServerOptions, FakeThreadListRequest, FakeThreadTurnsListRequest};

const THREAD_ID: &str = "thread-new";
const THREAD_LIST_CURSOR_PREFIX: &str = "thread-list-offset-";
const REPEATED_THREAD_LIST_CURSOR: &str = "thread-list-repeated";
const THREAD_TURN_CURSOR_PREFIX: &str = "thread-turn-index-";

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

pub(super) fn thread_archive_response(
    id: Value,
    params: &Value,
    state: &Arc<Mutex<FakeState>>,
) -> Value {
    let mut state = state.lock().unwrap();
    let thread = match requested_thread_mut(&id, params, &mut state) {
        Ok(thread) => thread,
        Err(response) => return response,
    };
    thread.archived = true;
    json!({ "id": id, "result": {} })
}

pub(super) fn thread_fork_response(
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

pub(super) fn thread_list_response(
    id: Value,
    params: &Value,
    state: &Arc<Mutex<FakeState>>,
    connection_id: u64,
) -> Option<Value> {
    let mut state = state.lock().unwrap();
    let requested_archived = params.get("archived").and_then(Value::as_bool);
    let requested_cursor = params
        .get("cursor")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let requested_limit = params
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|limit| u32::try_from(limit).ok());
    let follows_request_on_same_connection = state
        .thread_list_requests
        .iter()
        .any(|request| request.connection_id == connection_id);
    state.thread_list_requests.push(FakeThreadListRequest {
        connection_id,
        cursor: requested_cursor.clone(),
        limit: requested_limit,
        archived: requested_archived,
    });

    let archived = requested_archived.unwrap_or(false);
    let matching_threads = state
        .threads
        .iter()
        .filter(|thread| !thread.ephemeral && thread.archived == archived)
        .collect::<Vec<_>>();
    if follows_request_on_same_connection
        && state.options.lose_first_thread_list_continuation_response
    {
        state.options.lose_first_thread_list_continuation_response = false;
        return None;
    }
    let offset = match requested_cursor.as_deref() {
        None => 0,
        Some(REPEATED_THREAD_LIST_CURSOR) if state.options.repeat_thread_list_cursor => state
            .options
            .thread_list_page_size
            .unwrap_or(matching_threads.len().max(1)),
        Some(cursor) => {
            let Some(offset) = cursor
                .strip_prefix(THREAD_LIST_CURSOR_PREFIX)
                .and_then(|offset| offset.parse::<usize>().ok())
            else {
                return Some(json!({
                    "id": id,
                    "error": {
                        "code": -32600,
                        "message": format!("unknown thread-list cursor: {cursor}")
                    }
                }));
            };
            offset
        }
    };
    let requested_limit = requested_limit.and_then(|limit| usize::try_from(limit).ok());
    let page_size = state
        .options
        .thread_list_page_size
        .map(|page_size| requested_limit.map_or(page_size, |limit| limit.min(page_size)))
        .or(requested_limit)
        .unwrap_or(matching_threads.len().max(1))
        .max(1);
    let start = offset.min(matching_threads.len());
    let end = start.saturating_add(page_size).min(matching_threads.len());
    let data = matching_threads[start..end]
        .iter()
        .map(|thread| thread_json(thread, state.options, true))
        .collect::<Vec<_>>();
    let next_cursor = if state.options.repeat_thread_list_cursor {
        Some(REPEATED_THREAD_LIST_CURSOR.to_owned())
    } else if end < matching_threads.len() {
        let next_offset = if state.options.overlap_thread_list_pages && page_size > 1 {
            end - 1
        } else {
            end
        };
        Some(format!("{THREAD_LIST_CURSOR_PREFIX}{next_offset}"))
    } else {
        None
    };
    Some(json!({ "id": id, "result": { "data": data, "nextCursor": next_cursor } }))
}

pub(super) fn thread_read_response(
    id: Value,
    params: &Value,
    state: &Arc<Mutex<FakeState>>,
) -> Value {
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

pub(super) fn thread_turns_list_response(
    id: Value,
    params: &Value,
    state: &Arc<Mutex<FakeState>>,
    connection_id: u64,
) -> Value {
    let requested_thread_id = params
        .get("threadId")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    let requested_cursor = params
        .get("cursor")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let requested_limit = params
        .get("limit")
        .and_then(Value::as_u64)
        .and_then(|limit| u32::try_from(limit).ok());
    let requested_sort_direction = params
        .get("sortDirection")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let requested_items_view = params
        .get("itemsView")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let mut state = state.lock().unwrap();
    state
        .thread_turns_list_requests
        .push(FakeThreadTurnsListRequest {
            connection_id,
            thread_id: requested_thread_id.clone(),
            cursor: requested_cursor.clone(),
            limit: requested_limit,
            sort_direction: requested_sort_direction.clone(),
            items_view: requested_items_view,
        });
    if !state.options.support_thread_turns_list {
        return json!({
            "id": id,
            "error": { "code": -32601, "message": "method not found: thread/turns/list" }
        });
    }
    let Some(thread) = state
        .threads
        .iter()
        .find(|thread| thread.id == requested_thread_id)
    else {
        return json!({
            "id": id,
            "error": { "code": -32600, "message": format!("thread not loaded: {requested_thread_id}") }
        });
    };
    let cursor_index = match requested_cursor.as_deref() {
        None => None,
        Some(cursor) => match cursor
            .strip_prefix(THREAD_TURN_CURSOR_PREFIX)
            .and_then(|index| index.parse::<usize>().ok())
        {
            Some(index) if index < thread.turns.len() => Some(index),
            _ => {
                return json!({
                    "id": id,
                    "error": { "code": -32600, "message": format!("unknown thread-turn cursor: {cursor}") }
                });
            }
        },
    };
    let ascending = requested_sort_direction.as_deref() == Some("asc");
    let mut indices = if ascending {
        let start = cursor_index.unwrap_or(0);
        (start..thread.turns.len()).collect::<Vec<_>>()
    } else {
        let start = cursor_index.unwrap_or_else(|| thread.turns.len().saturating_sub(1));
        if thread.turns.is_empty() {
            vec![]
        } else {
            (0..=start).rev().collect::<Vec<_>>()
        }
    };
    if let Some(limit) = requested_limit.and_then(|limit| usize::try_from(limit).ok()) {
        indices.truncate(limit);
    }
    let backwards_cursor = indices
        .first()
        .map(|index| format!("{THREAD_TURN_CURSOR_PREFIX}{index}"));
    let data = indices
        .into_iter()
        .map(|index| turn_json(&thread.turns[index], state.options, true))
        .collect::<Vec<_>>();
    json!({
        "id": id,
        "result": {
            "data": data,
            "nextCursor": null,
            "backwardsCursor": backwards_cursor
        }
    })
}

pub(super) fn thread_set_name_response(
    id: Value,
    params: &Value,
    state: &Arc<Mutex<FakeState>>,
) -> Value {
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

pub(super) fn thread_unarchive_response(
    id: Value,
    params: &Value,
    state: &Arc<Mutex<FakeState>>,
) -> Value {
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

pub(super) fn thread_start_response(
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

pub(super) fn thread_resume_response(
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
