// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::io::{AsyncBufRead, AsyncWrite, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::protocol::{
    Connection, INITIALIZE_METHOD, InitializeParams, InitializeResponse, ServerMessage,
    THREAD_LIST_METHOD, THREAD_READ_METHOD, THREAD_RESUME_METHOD, TURN_START_METHOD,
    ThreadListParams, ThreadListResponse, ThreadReadParams, ThreadReadResponse, ThreadResumeParams,
    ThreadResumeResponse, TurnStartParams, TurnStartResponse, turn_stream_event,
};
use crate::{CodexError, ServerInfo, Thread, ThreadPage, TurnStreamEvent};

type AppServerConnection = Connection<BufReader<ChildStdout>, BufWriter<ChildStdin>>;
const APP_SERVER_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Filters and pagination controls for a thread history request.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThreadListOptions {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub archived: Option<bool>,
}

/// An asynchronous client for one Codex app-server child process.
pub struct CodexClient {
    child: Child,
    connection: Option<AppServerConnection>,
    server_info: ServerInfo,
    cancellation: CancellationToken,
}

impl CodexClient {
    /// Starts the specified Codex executable in app-server stdio mode and
    /// completes the initialization handshake.
    pub async fn spawn(executable: impl AsRef<OsStr>) -> Result<Self, CodexError> {
        Self::spawn_with_cancellation(executable, CancellationToken::new()).await
    }

    pub(crate) async fn spawn_with_cancellation(
        executable: impl AsRef<OsStr>,
        cancellation: CancellationToken,
    ) -> Result<Self, CodexError> {
        if cancellation.is_cancelled() {
            return Err(CodexError::Interrupted);
        }
        let executable = PathBuf::from(executable.as_ref());
        let mut child = Command::new(&executable)
            .args(["app-server", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|source| CodexError::Spawn {
                executable: executable.clone(),
                source,
            })?;

        let setup_result = async {
            let input = child
                .stdin
                .take()
                .ok_or(CodexError::MissingPipe("standard input"))?;
            let output = child
                .stdout
                .take()
                .ok_or(CodexError::MissingPipe("standard output"))?;
            let mut connection = Connection::new(BufReader::new(output), BufWriter::new(input));
            let response: InitializeResponse = connection
                .request(INITIALIZE_METHOD, &InitializeParams::craftward())
                .await?;
            connection.initialized().await?;
            Ok::<_, CodexError>((connection, ServerInfo::from(response)))
        };
        let setup_result = tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(CodexError::Interrupted),
            result = timeout(APP_SERVER_REQUEST_TIMEOUT, setup_result) => {
                result.unwrap_or(Err(CodexError::TimedOut(INITIALIZE_METHOD)))
            },
        };

        match setup_result {
            Ok((connection, server_info)) => Ok(Self {
                child,
                connection: Some(connection),
                server_info,
                cancellation,
            }),
            Err(error) => {
                terminate_child(&mut child).await;
                Err(error)
            }
        }
    }

    #[must_use]
    pub fn server_info(&self) -> &ServerInfo {
        &self.server_info
    }

    /// Lists persisted threads without triggering rollout scan-and-repair.
    pub async fn list_threads(
        &mut self,
        options: &ThreadListOptions,
    ) -> Result<ThreadPage, CodexError> {
        let params =
            ThreadListParams::new(options.cursor.as_deref(), options.limit, options.archived);
        let response: ThreadListResponse = self.request(THREAD_LIST_METHOD, &params).await?;
        Ok(ThreadPage {
            threads: response
                .data
                .into_iter()
                .map(|thread| thread.into_summary())
                .collect(),
            next_cursor: response.next_cursor,
        })
    }

    /// Reads one persisted thread, including its available turns and items.
    pub async fn read_thread(&mut self, thread_id: &str) -> Result<Thread, CodexError> {
        let params = ThreadReadParams {
            thread_id,
            include_turns: true,
        };
        let response: ThreadReadResponse = self.request(THREAD_READ_METHOD, &params).await?;
        response
            .thread
            .into_thread()
            .map_err(|source| CodexError::InvalidResponse {
                method: THREAD_READ_METHOD,
                source,
            })
    }

    /// Resumes a persisted thread and subscribes this connection to its events.
    pub async fn resume_thread(&mut self, thread_id: &str) -> Result<Thread, CodexError> {
        let params = ThreadResumeParams { thread_id };
        let response: ThreadResumeResponse = self.request(THREAD_RESUME_METHOD, &params).await?;
        response
            .thread
            .into_thread()
            .map_err(|source| CodexError::InvalidResponse {
                method: THREAD_RESUME_METHOD,
                source,
            })
    }

    /// Starts one text turn and streams events until that turn completes.
    ///
    /// Server approval requests are declined because Craftward does not expose
    /// approval controls yet. Other server requests receive an unsupported
    /// response so the app-server stream cannot remain blocked indefinitely.
    pub async fn start_text_turn(
        &mut self,
        thread_id: &str,
        text: &str,
        mut on_event: impl FnMut(TurnStreamEvent),
    ) -> Result<(), CodexError> {
        let connection = self
            .connection
            .as_mut()
            .ok_or(CodexError::UnexpectedEof(TURN_START_METHOD))?;
        tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(CodexError::Interrupted),
            result = start_text_turn_on_connection(connection, thread_id, text, &mut on_event) => result,
        }
    }

    /// Terminates and reaps the app-server child process.
    pub async fn shutdown(mut self) {
        self.connection.take();
        terminate_child(&mut self.child).await;
    }

    async fn request<P, T>(&mut self, method: &'static str, params: &P) -> Result<T, CodexError>
    where
        P: Serialize,
        T: DeserializeOwned,
    {
        let cancellation = self.cancellation.clone();
        let Some(connection) = self.connection.as_mut() else {
            return Err(CodexError::UnexpectedEof(method));
        };
        let request = connection.request(method, params);
        let result = tokio::select! {
            biased;
            _ = cancellation.cancelled() => Err(CodexError::Interrupted),
            result = timeout(APP_SERVER_REQUEST_TIMEOUT, request) => {
                result.unwrap_or(Err(CodexError::TimedOut(method)))
            },
        };
        if matches!(result, Err(CodexError::TimedOut(_))) {
            self.connection.take();
            terminate_child(&mut self.child).await;
        }
        result
    }
}

async fn start_text_turn_on_connection<R, W>(
    connection: &mut Connection<R, W>,
    thread_id: &str,
    text: &str,
    mut on_event: impl FnMut(TurnStreamEvent),
) -> Result<(), CodexError>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let response: TurnStartResponse = connection
        .request(TURN_START_METHOD, &TurnStartParams::text(thread_id, text))
        .await?;
    let turn = response
        .into_turn()
        .map_err(|source| CodexError::InvalidResponse {
            method: TURN_START_METHOD,
            source,
        })?;
    let turn_id = turn.id.clone();
    on_event(TurnStreamEvent::TurnStarted {
        thread_id: thread_id.to_owned(),
        turn,
    });

    loop {
        match connection.next_server_message(TURN_START_METHOD).await? {
            ServerMessage::Notification { method, params } => {
                let event = turn_stream_event(&method, params).map_err(|source| {
                    CodexError::InvalidResponse {
                        method: TURN_START_METHOD,
                        source,
                    }
                })?;
                let Some(event) = event else {
                    continue;
                };
                let completed = matches!(
                    &event,
                    TurnStreamEvent::TurnCompleted {
                        thread_id: completed_thread_id,
                        turn,
                    } if completed_thread_id == thread_id && turn.id == turn_id
                );
                on_event(event);
                if completed {
                    return Ok(());
                }
            }
            ServerMessage::Request { id, method, params } => {
                let request_thread_id = params
                    .get("threadId")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                if matches!(
                    method.as_str(),
                    "item/commandExecution/requestApproval" | "item/fileChange/requestApproval"
                ) {
                    connection
                        .respond_result(id, json!({ "decision": "decline" }))
                        .await?;
                    on_event(TurnStreamEvent::ApprovalDeclined {
                        thread_id: request_thread_id,
                        method,
                    });
                } else {
                    connection
                        .respond_error(
                            id,
                            -32601,
                            format!("Craftward does not support the server request {method}"),
                        )
                        .await?;
                    on_event(TurnStreamEvent::UnsupportedServerRequest {
                        thread_id: request_thread_id,
                        method,
                    });
                }
            }
        }
    }
}

async fn terminate_child(child: &mut Child) {
    if matches!(child.try_wait(), Ok(Some(_))) {
        return;
    }
    let _ = child.kill().await;
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::Duration;

    use super::*;

    const CHILD_HELPER_ENV: &str = "WARD_CODEX_CHILD_CLEANUP_HELPER";

    #[test]
    fn child_cleanup_helper() {
        if std::env::var_os(CHILD_HELPER_ENV).is_some() {
            std::thread::sleep(Duration::from_secs(60));
        }
    }

    #[tokio::test]
    async fn pre_cancelled_spawn_does_not_start_a_child() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();

        let result = CodexClient::spawn_with_cancellation(
            "/an/executable/that/does/not/exist",
            cancellation,
        )
        .await;

        assert!(matches!(result, Err(CodexError::Interrupted)));
    }

    #[tokio::test]
    async fn termination_reaps_a_running_child() {
        let executable = std::env::current_exe().unwrap();
        let mut child = Command::new(executable)
            .args(["--exact", "client::tests::child_cleanup_helper"])
            .env(CHILD_HELPER_ENV, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();

        terminate_child(&mut child).await;

        assert!(child.try_wait().unwrap().is_some());
    }

    #[tokio::test]
    async fn streams_a_complete_turn_in_protocol_order() {
        let input = concat!(
            "{\"id\":1,\"result\":{\"turn\":{\"id\":\"turn-2\",\"status\":\"inProgress\",\"items\":[]}}}\n",
            "{\"method\":\"item/started\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"startedAtMs\":0,\"item\":{\"id\":\"user-1\",\"type\":\"userMessage\",\"content\":[{\"type\":\"text\",\"text\":\"Continue\"}]}}}\n",
            "{\"method\":\"item/completed\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"completedAtMs\":0,\"item\":{\"id\":\"user-1\",\"type\":\"userMessage\",\"content\":[{\"type\":\"text\",\"text\":\"Continue\"}]}}}\n",
            "{\"method\":\"item/started\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"startedAtMs\":1,\"item\":{\"id\":\"commentary-1\",\"type\":\"agentMessage\",\"text\":\"\",\"phase\":\"commentary\"}}}\n",
            "{\"method\":\"item/agentMessage/delta\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"itemId\":\"commentary-1\",\"delta\":\"Inspecting.\"}}\n",
            "{\"method\":\"item/completed\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"completedAtMs\":2,\"item\":{\"id\":\"commentary-1\",\"type\":\"agentMessage\",\"text\":\"Inspecting.\",\"phase\":\"commentary\"}}}\n",
            "{\"method\":\"item/started\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"startedAtMs\":3,\"item\":{\"id\":\"command-1\",\"type\":\"commandExecution\",\"command\":\"pwd\",\"commandActions\":[],\"cwd\":\"/workspace\",\"status\":\"inProgress\"}}}\n",
            "{\"id\":\"approval-1\",\"method\":\"item/commandExecution/requestApproval\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"itemId\":\"command-1\"}}\n",
            "{\"method\":\"item/commandExecution/outputDelta\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"itemId\":\"command-1\",\"delta\":\"/workspace\\n\"}}\n",
            "{\"method\":\"item/completed\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"completedAtMs\":4,\"item\":{\"id\":\"command-1\",\"type\":\"commandExecution\",\"command\":\"pwd\",\"commandActions\":[],\"cwd\":\"/workspace\",\"status\":\"completed\",\"aggregatedOutput\":\"/workspace\\n\"}}}\n",
            "{\"method\":\"item/started\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"startedAtMs\":5,\"item\":{\"id\":\"change-1\",\"type\":\"fileChange\",\"changes\":[{\"path\":\"/workspace/a.txt\",\"diff\":\"+hello\"}],\"status\":\"inProgress\"}}}\n",
            "{\"method\":\"item/completed\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"completedAtMs\":6,\"item\":{\"id\":\"change-1\",\"type\":\"fileChange\",\"changes\":[{\"path\":\"/workspace/a.txt\",\"diff\":\"+hello\"}],\"status\":\"completed\"}}}\n",
            "{\"method\":\"item/started\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"startedAtMs\":7,\"item\":{\"id\":\"final-1\",\"type\":\"agentMessage\",\"text\":\"\",\"phase\":\"final_answer\"}}}\n",
            "{\"method\":\"item/agentMessage/delta\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"itemId\":\"final-1\",\"delta\":\"Done.\"}}\n",
            "{\"method\":\"item/completed\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"completedAtMs\":8,\"item\":{\"id\":\"final-1\",\"type\":\"agentMessage\",\"text\":\"Done.\",\"phase\":\"final_answer\"}}}\n",
            "{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thread-1\",\"turn\":{\"id\":\"turn-2\",\"status\":\"completed\",\"items\":[]}}}\n"
        );
        let mut connection = Connection::new(BufReader::new(Cursor::new(input)), Vec::new());
        let mut events = Vec::new();

        start_text_turn_on_connection(&mut connection, "thread-1", "Continue", |event| {
            events.push(event);
        })
        .await
        .expect("the streamed turn should complete");

        assert!(matches!(events[0], TurnStreamEvent::TurnStarted { .. }));
        assert!(matches!(
            events[1],
            TurnStreamEvent::ItemStarted {
                item: crate::ThreadItem::UserMessage { .. },
                ..
            }
        ));
        assert!(matches!(
            events[3],
            TurnStreamEvent::ItemStarted {
                item: crate::ThreadItem::AgentMessage {
                    phase: Some(crate::AgentMessagePhase::Commentary),
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            events[6],
            TurnStreamEvent::ItemStarted {
                item: crate::ThreadItem::Activity(crate::Activity {
                    kind: crate::ActivityKind::CommandExecution,
                    ..
                }),
                ..
            }
        ));
        assert!(matches!(
            events[7],
            TurnStreamEvent::ApprovalDeclined { .. }
        ));
        assert!(matches!(
            events[10],
            TurnStreamEvent::ItemStarted {
                item: crate::ThreadItem::Activity(crate::Activity {
                    kind: crate::ActivityKind::FileChange,
                    ..
                }),
                ..
            }
        ));
        assert!(matches!(
            events[events.len() - 2],
            TurnStreamEvent::ItemCompleted {
                item: crate::ThreadItem::AgentMessage {
                    phase: Some(crate::AgentMessagePhase::FinalAnswer),
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            events.last(),
            Some(TurnStreamEvent::TurnCompleted { .. })
        ));
        assert_eq!(
            String::from_utf8(connection.writer().clone()).unwrap(),
            concat!(
                "{\"id\":1,\"method\":\"turn/start\",\"params\":{\"threadId\":\"thread-1\",\"input\":[{\"type\":\"text\",\"text\":\"Continue\"}]}}\n",
                "{\"id\":\"approval-1\",\"result\":{\"decision\":\"decline\"}}\n"
            )
        );
    }
}
