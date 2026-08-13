// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::OsStr;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, Weak};

use serde_json::{Value, json};

use crate::protocol::{
    Connection, INITIALIZE_METHOD, InitializeParams, InitializeResponse, ServerMessage,
    THREAD_LIST_METHOD, THREAD_READ_METHOD, THREAD_RESUME_METHOD, TURN_START_METHOD,
    ThreadListParams, ThreadListResponse, ThreadReadParams, ThreadReadResponse, ThreadResumeParams,
    ThreadResumeResponse, TurnStartParams, TurnStartResponse, turn_stream_event,
};
use crate::{CodexError, ServerInfo, Thread, ThreadPage, TurnStreamEvent};

type AppServerConnection = Connection<BufReader<ChildStdout>, BufWriter<ChildStdin>>;

#[derive(Clone, Default)]
pub(crate) struct ProcessInterrupt {
    inner: Arc<ProcessInterruptState>,
}

#[derive(Default)]
struct ProcessInterruptState {
    interrupted: std::sync::atomic::AtomicBool,
    children: Mutex<Vec<Weak<Mutex<Child>>>>,
}

impl ProcessInterrupt {
    pub(crate) fn interrupt(&self) {
        self.inner
            .interrupted
            .store(true, std::sync::atomic::Ordering::Release);
        let children = self
            .inner
            .children
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        for child in children {
            terminate_child(&child);
        }
    }

    pub(crate) fn is_interrupted(&self) -> bool {
        self.inner
            .interrupted
            .load(std::sync::atomic::Ordering::Acquire)
    }

    fn register(&self, child: &Arc<Mutex<Child>>) -> bool {
        let mut active_children = self
            .inner
            .children
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        active_children.retain(|child| child.strong_count() > 0);
        active_children.push(Arc::downgrade(child));
        let interrupted = self.is_interrupted();
        drop(active_children);
        if interrupted {
            terminate_child(child);
        }
        interrupted
    }
}

fn terminate_child(child: &Arc<Mutex<Child>>) {
    let mut child = child
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if matches!(child.try_wait(), Ok(Some(_))) {
        return;
    }
    if child.kill().is_ok() {
        let _ = child.wait();
    }
}

/// Filters and pagination controls for a thread history request.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThreadListOptions {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub archived: Option<bool>,
}

/// A synchronous client for one Codex app-server child process.
///
/// Calls block until the corresponding app-server response arrives. GUI callers
/// should own the client on a worker thread rather than invoke it on the UI
/// thread.
pub struct CodexClient {
    child: Arc<Mutex<Child>>,
    connection: Option<AppServerConnection>,
    server_info: ServerInfo,
}

impl CodexClient {
    /// Starts the specified Codex executable in app-server stdio mode and
    /// completes the initialization handshake.
    pub fn spawn(executable: impl AsRef<OsStr>) -> Result<Self, CodexError> {
        Self::spawn_inner(executable.as_ref(), None)
    }

    pub(crate) fn spawn_interruptible(
        executable: impl AsRef<OsStr>,
        interrupt: &ProcessInterrupt,
    ) -> Result<Self, CodexError> {
        Self::spawn_inner(executable.as_ref(), Some(interrupt))
    }

    fn spawn_inner(
        executable: &OsStr,
        interrupt: Option<&ProcessInterrupt>,
    ) -> Result<Self, CodexError> {
        if interrupt.is_some_and(ProcessInterrupt::is_interrupted) {
            return Err(CodexError::Interrupted);
        }
        let executable = PathBuf::from(executable);
        let child = Command::new(&executable)
            .args(["app-server", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|source| CodexError::Spawn {
                executable: executable.clone(),
                source,
            })?;

        let child = Arc::new(Mutex::new(child));
        if interrupt.is_some_and(|interrupt| interrupt.register(&child)) {
            return Err(CodexError::Interrupted);
        }

        let setup_result = (|| {
            let (input, output) = {
                let mut child = child
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let input = child
                    .stdin
                    .take()
                    .ok_or(CodexError::MissingPipe("standard input"))?;
                let output = child
                    .stdout
                    .take()
                    .ok_or(CodexError::MissingPipe("standard output"))?;
                (input, output)
            };
            let mut connection = Connection::new(BufReader::new(output), BufWriter::new(input));
            let response: InitializeResponse =
                connection.request(INITIALIZE_METHOD, &InitializeParams::craftward())?;
            connection.initialized()?;
            Ok::<_, CodexError>((connection, ServerInfo::from(response)))
        })();

        match setup_result {
            Ok(_) if interrupt.is_some_and(ProcessInterrupt::is_interrupted) => {
                terminate_child(&child);
                Err(CodexError::Interrupted)
            }
            Ok((connection, server_info)) => Ok(Self {
                child,
                connection: Some(connection),
                server_info,
            }),
            Err(error) => {
                terminate_child(&child);
                if interrupt.is_some_and(ProcessInterrupt::is_interrupted) {
                    Err(CodexError::Interrupted)
                } else {
                    Err(error)
                }
            }
        }
    }

    #[must_use]
    pub fn server_info(&self) -> &ServerInfo {
        &self.server_info
    }

    /// Lists persisted threads without triggering rollout scan-and-repair.
    pub fn list_threads(&mut self, options: &ThreadListOptions) -> Result<ThreadPage, CodexError> {
        let params =
            ThreadListParams::new(options.cursor.as_deref(), options.limit, options.archived);
        let response: ThreadListResponse = self
            .connection
            .as_mut()
            .expect("the connection exists while CodexClient is alive")
            .request(THREAD_LIST_METHOD, &params)?;
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
    pub fn read_thread(&mut self, thread_id: &str) -> Result<Thread, CodexError> {
        let params = ThreadReadParams {
            thread_id,
            include_turns: true,
        };
        let response: ThreadReadResponse = self
            .connection
            .as_mut()
            .expect("the connection exists while CodexClient is alive")
            .request(THREAD_READ_METHOD, &params)?;
        response
            .thread
            .into_thread()
            .map_err(|source| CodexError::InvalidResponse {
                method: THREAD_READ_METHOD,
                source,
            })
    }

    /// Resumes a persisted thread and subscribes this connection to its events.
    pub fn resume_thread(&mut self, thread_id: &str) -> Result<Thread, CodexError> {
        let response: ThreadResumeResponse = self
            .connection
            .as_mut()
            .expect("the connection exists while CodexClient is alive")
            .request(THREAD_RESUME_METHOD, &ThreadResumeParams { thread_id })?;
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
    pub fn start_text_turn(
        &mut self,
        thread_id: &str,
        text: &str,
        mut on_event: impl FnMut(TurnStreamEvent),
    ) -> Result<(), CodexError> {
        let connection = self
            .connection
            .as_mut()
            .expect("the connection exists while CodexClient is alive");
        start_text_turn_on_connection(connection, thread_id, text, &mut on_event)
    }
}

fn start_text_turn_on_connection<R, W>(
    connection: &mut Connection<R, W>,
    thread_id: &str,
    text: &str,
    mut on_event: impl FnMut(TurnStreamEvent),
) -> Result<(), CodexError>
where
    R: BufRead,
    W: Write,
{
    let response: TurnStartResponse =
        connection.request(TURN_START_METHOD, &TurnStartParams::text(thread_id, text))?;
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
        match connection.next_server_message(TURN_START_METHOD)? {
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
                    connection.respond_result(id, json!({ "decision": "decline" }))?;
                    on_event(TurnStreamEvent::ApprovalDeclined {
                        thread_id: request_thread_id,
                        method,
                    });
                } else {
                    connection.respond_error(
                        id,
                        -32601,
                        format!("Craftward does not support the server request {method}"),
                    )?;
                    on_event(TurnStreamEvent::UnsupportedServerRequest {
                        thread_id: request_thread_id,
                        method,
                    });
                }
            }
        }
    }
}

impl Drop for CodexClient {
    fn drop(&mut self) {
        // Closing stdin lets a healthy app-server observe EOF. Kill and reap the
        // child as a bounded fallback so dropping the client never leaves it
        // running or creates a zombie process.
        self.connection.take();
        terminate_child(&self.child);
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead as _, BufReader, Cursor, Write as _};
    use std::time::Duration;

    use super::*;

    const HELPER_ENVIRONMENT_VARIABLE: &str = "CRAFTWARD_CODEX_INTERRUPT_HELPER";
    const HELPER_READY_LINE: &str = "craftward-interrupt-helper-ready";

    #[test]
    fn interrupt_helper_process() {
        if std::env::var_os(HELPER_ENVIRONMENT_VARIABLE).is_none() {
            return;
        }
        println!("{HELPER_READY_LINE}");
        std::io::stdout().flush().unwrap();
        std::thread::sleep(Duration::from_secs(60));
    }

    fn running_helper() -> Arc<Mutex<Child>> {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "client::tests::interrupt_helper_process",
                "--nocapture",
            ])
            .env(HELPER_ENVIRONMENT_VARIABLE, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        loop {
            line.clear();
            assert_ne!(output.read_line(&mut line).unwrap(), 0);
            if line.trim() == HELPER_READY_LINE {
                break;
            }
        }
        Arc::new(Mutex::new(child))
    }

    #[test]
    fn interrupt_terminates_the_registered_child() {
        let child = running_helper();
        let interrupt = ProcessInterrupt::default();
        assert!(!interrupt.register(&child));

        interrupt.interrupt();

        assert!(matches!(
            child
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .try_wait(),
            Ok(Some(_))
        ));
    }

    #[test]
    fn a_preexisting_interrupt_terminates_a_newly_registered_child() {
        let interrupt = ProcessInterrupt::default();
        interrupt.interrupt();
        let child = running_helper();

        assert!(interrupt.register(&child));
        assert!(matches!(
            child
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .try_wait(),
            Ok(Some(_))
        ));
    }

    #[test]
    fn interrupt_terminates_every_registered_child() {
        let first = running_helper();
        let second = running_helper();
        let interrupt = ProcessInterrupt::default();
        assert!(!interrupt.register(&first));
        assert!(!interrupt.register(&second));

        interrupt.interrupt();

        for child in [first, second] {
            assert!(matches!(
                child
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .try_wait(),
                Ok(Some(_))
            ));
        }
    }

    #[test]
    fn streams_a_complete_turn_in_protocol_order() {
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
