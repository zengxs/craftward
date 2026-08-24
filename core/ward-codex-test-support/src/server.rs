// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::{Arc, Mutex};

use serde_json::{Value, json};
use tokio::io::{
    AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream, ReadHalf, WriteHalf, duplex, split,
};
use tokio::task::JoinHandle;
use ward_codex::{CodexAppServerConnector, CodexAppServerTransport, CodexError};

use super::catalog::model_list_response;
use super::state::FakeState;
use super::threads::{
    thread_archive_response, thread_fork_response, thread_list_response, thread_read_response,
    thread_resume_response, thread_set_name_response, thread_start_response,
    thread_turns_list_response, thread_unarchive_response,
};
use super::turns::{handle_client_response, turn_start_messages, turn_steer_messages};

const STREAM_CAPACITY: usize = 64 * 1024;

pub(super) struct FakeConnector {
    state: Arc<Mutex<FakeState>>,
}

impl FakeConnector {
    pub(super) fn new(state: Arc<Mutex<FakeState>>) -> Self {
        Self { state }
    }
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
    if method == "thread/read" {
        state.lock().unwrap().thread_read_request_count += 1;
    }
    let params = message.get("params").cloned().unwrap_or_else(|| json!({}));
    Some(match method {
        "model/list" => vec![model_list_response(id, &params, state)],
        "thread/archive" => vec![thread_archive_response(id, &params, state)],
        "thread/fork" => {
            return thread_fork_response(id, &params, state, connection_id)
                .map(|response| vec![response]);
        }
        "thread/list" => {
            return thread_list_response(id, &params, state, connection_id)
                .map(|response| vec![response]);
        }
        "thread/read" => vec![thread_read_response(id, &params, state)],
        "thread/turns/list" => vec![thread_turns_list_response(
            id,
            &params,
            state,
            connection_id,
        )],
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
