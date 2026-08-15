// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, VecDeque};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncWrite, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::protocol::{
    Connection, INITIALIZE_METHOD, InitializeParams, InitializeResponse, ServerMessage,
    THREAD_LIST_METHOD, THREAD_READ_METHOD, THREAD_RESUME_METHOD, THREAD_START_METHOD,
    TURN_INTERRUPT_METHOD, TURN_START_METHOD, ThreadListParams, ThreadListResponse,
    ThreadReadParams, ThreadReadResponse, ThreadResumeParams, ThreadResumeResponse,
    ThreadStartParams, ThreadStartResponse, TurnInterruptParams, TurnInterruptResponse,
    TurnStartParams, TurnStartResponse, interaction_result, pending_interaction,
    resolved_server_request, turn_stream_event,
};
use crate::{
    CodexError, InteractionId, InteractionResponse, PendingInteraction, ServerInfo, Thread,
    ThreadPage, ThreadStartOptions, ThreadStreamEvent, ThreadSubscription, TurnOptions,
};

type AppServerConnection = Connection<BufReader<ChildStdout>, BufWriter<ChildStdin>>;
const APP_SERVER_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Filters and pagination controls for a thread history request.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThreadListOptions {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub archived: Option<bool>,
}

struct PendingServerInteraction {
    request_id: Value,
    interaction: PendingInteraction,
}

struct SubscriptionState {
    next_interaction_id: u64,
    pending: BTreeMap<InteractionId, PendingServerInteraction>,
    queued_events: VecDeque<ThreadStreamEvent>,
}

impl Default for SubscriptionState {
    fn default() -> Self {
        Self {
            next_interaction_id: 1,
            pending: BTreeMap::new(),
            queued_events: VecDeque::new(),
        }
    }
}

impl SubscriptionState {
    fn reset(&mut self) {
        self.pending.clear();
        self.queued_events.clear();
    }

    fn pending_for(&self, thread_id: &str) -> Vec<PendingInteraction> {
        self.pending
            .values()
            .filter(|entry| entry.interaction.thread_id == thread_id)
            .map(|entry| entry.interaction.clone())
            .collect()
    }

    fn update_event(&self, thread_id: &str) -> ThreadStreamEvent {
        ThreadStreamEvent::PendingInteractionsUpdated {
            thread_id: thread_id.to_owned(),
            interactions: self.pending_for(thread_id),
        }
    }

    fn allocate_interaction_id(&mut self) -> InteractionId {
        loop {
            let interaction_id = InteractionId::new(self.next_interaction_id)
                .expect("the next interaction identifier is non-zero");
            self.next_interaction_id = self.next_interaction_id.checked_add(1).unwrap_or(1);
            if !self.pending.contains_key(&interaction_id) {
                return interaction_id;
            }
        }
    }

    fn remove_external(&mut self, request_id: &Value) -> Option<PendingInteraction> {
        let interaction_id = self.pending.iter().find_map(|(interaction_id, entry)| {
            (entry.request_id == *request_id).then_some(*interaction_id)
        })?;
        self.pending
            .remove(&interaction_id)
            .map(|entry| entry.interaction)
    }

    fn finish_turn(&mut self, thread_id: &str, turn_id: &str) -> bool {
        let previous_count = self.pending.len();
        self.pending.retain(|_, entry| {
            entry.interaction.thread_id != thread_id
                || entry.interaction.turn_id.as_deref() != Some(turn_id)
        });
        self.pending.len() != previous_count
    }

    async fn next_event<R, W>(
        &mut self,
        connection: &mut Connection<R, W>,
        operation: &'static str,
    ) -> Result<ThreadStreamEvent, CodexError>
    where
        R: AsyncBufRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        if let Some(event) = self.queued_events.pop_front() {
            return Ok(event);
        }

        loop {
            match connection.next_server_message(operation).await? {
                ServerMessage::Notification { method, params } => {
                    if method == "serverRequest/resolved" {
                        let (request_id, _resolved_thread_id) = resolved_server_request(params)
                            .map_err(|source| CodexError::InvalidResponse {
                                method: operation,
                                source,
                            })?;
                        let Some(interaction) = self.remove_external(&request_id) else {
                            continue;
                        };
                        return Ok(self.update_event(&interaction.thread_id));
                    }
                    let event = turn_stream_event(&method, params).map_err(|source| {
                        CodexError::InvalidResponse {
                            method: operation,
                            source,
                        }
                    })?;
                    let Some(event) = event else {
                        continue;
                    };
                    if let ThreadStreamEvent::TurnCompleted { thread_id, turn } = &event
                        && self.finish_turn(thread_id, &turn.id)
                    {
                        let update = self.update_event(thread_id);
                        self.queued_events.push_back(event);
                        return Ok(update);
                    }
                    return Ok(event);
                }
                ServerMessage::Request {
                    id: request_id,
                    method,
                    params,
                } => {
                    let request_thread_id = params
                        .get("threadId")
                        .and_then(Value::as_str)
                        .map(str::to_owned);
                    let interaction_id = self.allocate_interaction_id();
                    let interaction = pending_interaction(interaction_id, &method, params)
                        .map_err(|source| CodexError::InvalidResponse {
                            method: operation,
                            source,
                        })?;
                    if let Some(interaction) = interaction {
                        let thread_id = interaction.thread_id.clone();
                        self.pending.insert(
                            interaction_id,
                            PendingServerInteraction {
                                request_id,
                                interaction,
                            },
                        );
                        return Ok(self.update_event(&thread_id));
                    }
                    connection
                        .respond_error(
                            request_id,
                            -32601,
                            format!("Craftward does not support the server request {method}"),
                        )
                        .await?;
                    return Ok(ThreadStreamEvent::UnsupportedServerRequest {
                        thread_id: request_thread_id,
                        method,
                    });
                }
            }
        }
    }

    async fn resolve<R, W>(
        &mut self,
        connection: &mut Connection<R, W>,
        response: InteractionResponse,
    ) -> Result<ThreadStreamEvent, CodexError>
    where
        R: AsyncBufRead + Unpin,
        W: AsyncWrite + Unpin,
    {
        let entry = self
            .pending
            .get(&response.interaction_id)
            .ok_or(CodexError::UnknownInteraction(response.interaction_id))?;
        let result = interaction_result(&entry.interaction, &response).map_err(|description| {
            CodexError::InvalidInteractionResponse {
                interaction_id: response.interaction_id,
                description,
            }
        })?;
        let request_id = entry.request_id.clone();
        let thread_id = entry.interaction.thread_id.clone();
        connection.respond_result(request_id, result).await?;
        self.pending.remove(&response.interaction_id);
        Ok(self.update_event(&thread_id))
    }
}

/// An asynchronous client for one Codex app-server child process.
pub struct CodexClient {
    child: Child,
    connection: Option<AppServerConnection>,
    server_info: ServerInfo,
    cancellation: CancellationToken,
    subscription_state: SubscriptionState,
    subscribed_thread_model: Option<String>,
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
                subscription_state: SubscriptionState::default(),
                subscribed_thread_model: None,
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

    /// Starts a thread in one app-server-visible working directory and
    /// subscribes this connection to its events.
    pub(crate) async fn start_thread(
        &mut self,
        working_directory: &Path,
        options: ThreadStartOptions,
    ) -> Result<ThreadSubscription, CodexError> {
        let cancellation = self.cancellation.clone();
        let result = {
            let connection = self
                .connection
                .as_mut()
                .ok_or(CodexError::UnexpectedEof(THREAD_START_METHOD))?;
            let request = start_thread_on_connection(connection, working_directory, options);
            tokio::select! {
                biased;
                _ = cancellation.cancelled() => Err(CodexError::Interrupted),
                result = timeout(APP_SERVER_REQUEST_TIMEOUT, request) => {
                    result.unwrap_or(Err(CodexError::TimedOut(THREAD_START_METHOD)))
                },
            }
        };
        if matches!(result, Err(CodexError::TimedOut(_))) {
            self.connection.take();
            terminate_child(&mut self.child).await;
        }
        let (subscription, model) = result?;
        self.subscription_state.reset();
        self.subscribed_thread_model = Some(model);
        Ok(subscription)
    }

    /// Resumes a persisted thread and subscribes this connection to its events.
    pub async fn resume_thread(
        &mut self,
        thread_id: &str,
    ) -> Result<ThreadSubscription, CodexError> {
        let params = ThreadResumeParams { thread_id };
        let response: ThreadResumeResponse = self.request(THREAD_RESUME_METHOD, &params).await?;
        let (subscription, model) =
            response
                .into_parts()
                .map_err(|source| CodexError::InvalidResponse {
                    method: THREAD_RESUME_METHOD,
                    source,
                })?;
        self.subscription_state.reset();
        self.subscribed_thread_model = model;
        Ok(subscription)
    }

    /// Starts one text turn and returns its initial streamed event.
    pub async fn begin_text_turn(
        &mut self,
        thread_id: &str,
        text: &str,
        options: TurnOptions,
    ) -> Result<ThreadStreamEvent, CodexError> {
        let connection = self
            .connection
            .as_mut()
            .ok_or(CodexError::UnexpectedEof(TURN_START_METHOD))?;
        tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(CodexError::Interrupted),
            result = begin_text_turn_on_connection(
                connection,
                thread_id,
                text,
                self.subscribed_thread_model.as_deref(),
                options,
            ) => result,
        }
    }

    /// Waits for the next event on a connection subscribed to a thread.
    pub async fn next_subscription_event(&mut self) -> Result<ThreadStreamEvent, CodexError> {
        let connection = self
            .connection
            .as_mut()
            .ok_or(CodexError::UnexpectedEof(THREAD_RESUME_METHOD))?;
        tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(CodexError::Interrupted),
            result = self.subscription_state.next_event(
                connection,
                THREAD_RESUME_METHOD,
            ) => result,
        }
    }

    /// Resolves one pending approval or user-input request.
    pub async fn resolve_interaction(
        &mut self,
        response: InteractionResponse,
    ) -> Result<ThreadStreamEvent, CodexError> {
        let connection = self
            .connection
            .as_mut()
            .ok_or(CodexError::UnexpectedEof(THREAD_RESUME_METHOD))?;
        self.subscription_state.resolve(connection, response).await
    }

    pub(crate) fn pending_interactions(&self, thread_id: &str) -> Vec<PendingInteraction> {
        self.subscription_state.pending_for(thread_id)
    }

    /// Requests interruption of the active turn on this subscribed thread.
    pub async fn interrupt_turn(
        &mut self,
        thread_id: &str,
        turn_id: &str,
    ) -> Result<(), CodexError> {
        let _: TurnInterruptResponse = self
            .request(
                TURN_INTERRUPT_METHOD,
                &TurnInterruptParams { thread_id, turn_id },
            )
            .await?;
        Ok(())
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

async fn start_thread_on_connection<R, W>(
    connection: &mut Connection<R, W>,
    working_directory: &Path,
    options: ThreadStartOptions,
) -> Result<(ThreadSubscription, String), CodexError>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let response: ThreadStartResponse = connection
        .request(
            THREAD_START_METHOD,
            &ThreadStartParams::new(working_directory, options),
        )
        .await?;
    let (subscription, model, ephemeral) =
        response
            .into_parts()
            .map_err(|source| CodexError::InvalidResponse {
                method: THREAD_START_METHOD,
                source,
            })?;
    if options.ephemeral && ephemeral != Some(true) {
        return Err(CodexError::UnexpectedMessage {
            method: THREAD_START_METHOD,
            description: "the app-server did not confirm an ephemeral thread".to_owned(),
        });
    }
    Ok((subscription, model))
}

async fn begin_text_turn_on_connection<R, W>(
    connection: &mut Connection<R, W>,
    thread_id: &str,
    text: &str,
    model: Option<&str>,
    options: TurnOptions,
) -> Result<ThreadStreamEvent, CodexError>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let params = TurnStartParams::text(thread_id, text, model, options)?;
    let response: TurnStartResponse = connection.request(TURN_START_METHOD, &params).await?;
    let turn = response
        .into_turn()
        .map_err(|source| CodexError::InvalidResponse {
            method: TURN_START_METHOD,
            source,
        })?;
    Ok(ThreadStreamEvent::TurnStarted {
        thread_id: thread_id.to_owned(),
        turn,
    })
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
    async fn starts_a_thread_in_the_requested_working_directory() {
        let input = concat!(
            "{\"id\":1,\"result\":{",
            "\"model\":\"gpt-5.6-sol\",",
            "\"thread\":{",
            "\"id\":\"thread-new\",\"name\":null,\"preview\":\"\",",
            "\"cwd\":\"/workspace\",\"createdAt\":10,\"updatedAt\":10,",
            "\"ephemeral\":false,\"status\":{\"type\":\"idle\"},\"turns\":[]",
            "}}}\n"
        );
        let mut connection = Connection::new(BufReader::new(Cursor::new(input)), Vec::new());

        let (subscription, model) = start_thread_on_connection(
            &mut connection,
            Path::new("/workspace"),
            ThreadStartOptions::default(),
        )
        .await
        .expect("the thread should start");

        assert_eq!(subscription.thread.summary.id, "thread-new");
        assert_eq!(
            subscription.runtime_status,
            crate::ThreadRuntimeStatus::Idle
        );
        assert_eq!(model, "gpt-5.6-sol");
        assert_eq!(
            String::from_utf8(connection.writer().clone()).unwrap(),
            "{\"id\":1,\"method\":\"thread/start\",\"params\":{\"cwd\":\"/workspace\"}}\n"
        );
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
            "{\"id\":\"approval-1\",\"method\":\"item/commandExecution/requestApproval\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"itemId\":\"command-1\",\"startedAtMs\":3}}\n",
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
        let mut subscription_state = SubscriptionState::default();
        let mut events = Vec::new();

        events.push(
            begin_text_turn_on_connection(
                &mut connection,
                "thread-1",
                "Continue",
                None,
                TurnOptions::default(),
            )
            .await
            .expect("the turn should start"),
        );
        loop {
            let event = subscription_state
                .next_event(&mut connection, TURN_START_METHOD)
                .await
                .expect("the streamed turn should continue");
            if let ThreadStreamEvent::PendingInteractionsUpdated { interactions, .. } = &event
                && let Some(interaction) = interactions.first()
            {
                let response = InteractionResponse {
                    interaction_id: interaction.id,
                    body: crate::InteractionResponseBody::Decision(
                        crate::InteractionDecision::Decline,
                    ),
                };
                events.push(event);
                let update = subscription_state
                    .resolve(&mut connection, response)
                    .await
                    .unwrap();
                assert!(matches!(
                    &update,
                    ThreadStreamEvent::PendingInteractionsUpdated { interactions, .. }
                        if interactions.is_empty()
                ));
                events.push(update);
                continue;
            }
            let completed = matches!(event, ThreadStreamEvent::TurnCompleted { .. });
            events.push(event);
            if completed {
                break;
            }
        }

        assert!(matches!(events[0], ThreadStreamEvent::TurnStarted { .. }));
        assert!(matches!(
            events[1],
            ThreadStreamEvent::ItemStarted {
                item: crate::ThreadItem::UserMessage { .. },
                ..
            }
        ));
        assert!(matches!(
            events[3],
            ThreadStreamEvent::ItemStarted {
                item: crate::ThreadItem::AgentMessage {
                    phase: Some(crate::AgentMessagePhase::Commentary),
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            events[6],
            ThreadStreamEvent::ItemStarted {
                item: crate::ThreadItem::Activity(crate::Activity {
                    kind: crate::ActivityKind::CommandExecution,
                    ..
                }),
                ..
            }
        ));
        assert!(matches!(
            events[7],
            ThreadStreamEvent::PendingInteractionsUpdated {
                ref interactions,
                ..
            } if matches!(
                interactions.as_slice(),
                [PendingInteraction {
                    kind: crate::PendingInteractionKind::CommandApproval,
                    ..
                }]
            )
        ));
        assert!(matches!(
            events[8],
            ThreadStreamEvent::PendingInteractionsUpdated {
                ref interactions,
                ..
            } if interactions.is_empty()
        ));
        assert!(matches!(
            events[11],
            ThreadStreamEvent::ItemStarted {
                item: crate::ThreadItem::Activity(crate::Activity {
                    kind: crate::ActivityKind::FileChange,
                    ..
                }),
                ..
            }
        ));
        assert!(matches!(
            events[events.len() - 2],
            ThreadStreamEvent::ItemCompleted {
                item: crate::ThreadItem::AgentMessage {
                    phase: Some(crate::AgentMessagePhase::FinalAnswer),
                    ..
                },
                ..
            }
        ));
        assert!(matches!(
            events.last(),
            Some(ThreadStreamEvent::TurnCompleted { .. })
        ));
        assert_eq!(
            String::from_utf8(connection.writer().clone()).unwrap(),
            concat!(
                "{\"id\":1,\"method\":\"turn/start\",\"params\":{\"threadId\":\"thread-1\",\"input\":[{\"type\":\"text\",\"text\":\"Continue\"}]}}\n",
                "{\"id\":\"approval-1\",\"result\":{\"decision\":\"decline\"}}\n"
            )
        );
    }

    #[tokio::test]
    async fn reads_a_subscription_event_without_starting_a_turn() {
        let input = "{\"method\":\"thread/status/changed\",\"params\":{\"threadId\":\"thread-1\",\"status\":{\"type\":\"active\",\"activeFlags\":[\"waitingOnApproval\"]}}}\n";
        let mut connection = Connection::new(BufReader::new(Cursor::new(input)), Vec::new());
        let mut subscription_state = SubscriptionState::default();

        let event = subscription_state
            .next_event(&mut connection, THREAD_RESUME_METHOD)
            .await
            .expect("the idle subscription event should be readable");

        assert!(matches!(
            event,
            ThreadStreamEvent::ThreadStatusChanged {
                thread_id,
                status: crate::ThreadRuntimeStatus::Active { .. },
            } if thread_id == "thread-1"
        ));
    }

    #[tokio::test]
    async fn clears_unanswered_interactions_before_the_completed_turn_event() {
        let input = concat!(
            "{\"id\":\"approval-1\",\"method\":\"item/commandExecution/requestApproval\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"itemId\":\"command-1\",\"startedAtMs\":3}}\n",
            "{\"method\":\"turn/completed\",\"params\":{\"threadId\":\"thread-1\",\"turn\":{\"id\":\"turn-2\",\"status\":\"completed\",\"items\":[]}}}\n"
        );
        let mut connection = Connection::new(BufReader::new(Cursor::new(input)), Vec::new());
        let mut subscription_state = SubscriptionState::default();

        let requested = subscription_state
            .next_event(&mut connection, THREAD_RESUME_METHOD)
            .await
            .unwrap();
        assert!(matches!(
            requested,
            ThreadStreamEvent::PendingInteractionsUpdated { interactions, .. }
                if interactions.len() == 1
        ));

        let cleared = subscription_state
            .next_event(&mut connection, THREAD_RESUME_METHOD)
            .await
            .unwrap();
        assert!(matches!(
            cleared,
            ThreadStreamEvent::PendingInteractionsUpdated { interactions, .. }
                if interactions.is_empty()
        ));

        assert!(matches!(
            subscription_state
                .next_event(&mut connection, THREAD_RESUME_METHOD)
                .await
                .unwrap(),
            ThreadStreamEvent::TurnCompleted { .. }
        ));
    }

    #[tokio::test]
    async fn resolves_an_interaction_by_its_opaque_server_request_id() {
        let input = concat!(
            "{\"id\":\"approval-1\",\"method\":\"item/fileChange/requestApproval\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\",\"itemId\":\"change-1\",\"startedAtMs\":3}}\n",
            "{\"method\":\"serverRequest/resolved\",\"params\":{\"requestId\":\"approval-1\",\"threadId\":\"thread-1\"}}\n"
        );
        let mut connection = Connection::new(BufReader::new(Cursor::new(input)), Vec::new());
        let mut subscription_state = SubscriptionState::default();

        let requested = subscription_state
            .next_event(&mut connection, THREAD_RESUME_METHOD)
            .await
            .unwrap();
        assert!(matches!(
            requested,
            ThreadStreamEvent::PendingInteractionsUpdated { interactions, .. }
                if interactions.len() == 1
        ));

        let resolved = subscription_state
            .next_event(&mut connection, THREAD_RESUME_METHOD)
            .await
            .unwrap();
        assert!(matches!(
            resolved,
            ThreadStreamEvent::PendingInteractionsUpdated { interactions, .. }
                if interactions.is_empty()
        ));
    }

    #[tokio::test]
    async fn serializes_the_active_turn_interrupt_request() {
        let input = "{\"id\":1,\"result\":{}}\n";
        let mut connection = Connection::new(BufReader::new(Cursor::new(input)), Vec::new());

        let _: TurnInterruptResponse = connection
            .request(
                TURN_INTERRUPT_METHOD,
                &TurnInterruptParams {
                    thread_id: "thread-1",
                    turn_id: "turn-2",
                },
            )
            .await
            .unwrap();

        assert_eq!(
            String::from_utf8(connection.writer().clone()).unwrap(),
            "{\"id\":1,\"method\":\"turn/interrupt\",\"params\":{\"threadId\":\"thread-1\",\"turnId\":\"turn-2\"}}\n"
        );
    }
}
