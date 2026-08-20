// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::{BTreeMap, HashSet, VecDeque};
use std::ffi::OsStr;
use std::path::Path;
use std::time::Duration;

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::io::{AsyncBufRead, AsyncWrite, BufReader, BufWriter};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::app_server::{AppServerReader, AppServerShutdown, AppServerWriter};
use crate::protocol::{
    Connection, INITIALIZE_METHOD, InitializeParams, InitializeResponse, MODEL_LIST_METHOD,
    ModelListParams, ModelListResponse, ServerMessage, THREAD_ARCHIVE_METHOD, THREAD_FORK_METHOD,
    THREAD_LIST_METHOD, THREAD_READ_METHOD, THREAD_RESUME_METHOD, THREAD_SET_NAME_METHOD,
    THREAD_START_METHOD, THREAD_UNARCHIVE_METHOD, TURN_INTERRUPT_METHOD, TURN_START_METHOD,
    TURN_STEER_METHOD, ThreadArchiveParams, ThreadArchiveResponse, ThreadForkParams,
    ThreadForkResponse, ThreadListParams, ThreadListResponse, ThreadReadParams, ThreadReadResponse,
    ThreadResumeParams, ThreadResumeResponse, ThreadSetNameParams, ThreadSetNameResponse,
    ThreadStartParams, ThreadStartResponse, ThreadUnarchiveParams, ThreadUnarchiveResponse,
    TurnInterruptParams, TurnInterruptResponse, TurnStartParams, TurnStartResponse,
    TurnSteerParams, TurnSteerResponse, interaction_result, pending_interaction,
    resolved_server_request, turn_stream_event,
};
use crate::{
    CodexAppServerSource, CodexError, InteractionId, InteractionResponse, ModelCatalog,
    PendingInteraction, ServerInfo, Thread, ThreadInferenceState, ThreadPage, ThreadStartOptions,
    ThreadStreamEvent, ThreadSubscription, TurnInput, TurnOptions,
};

type AppServerConnection = Connection<BufReader<AppServerReader>, BufWriter<AppServerWriter>>;
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

/// An asynchronous client for one Codex app-server connection.
pub struct CodexClient {
    connection: Option<AppServerConnection>,
    shutdown: Option<AppServerShutdown>,
    server_info: ServerInfo,
    cancellation: CancellationToken,
    subscription_state: SubscriptionState,
    subscribed_thread_inference: ThreadInferenceState,
}

impl CodexClient {
    /// Starts the specified Codex executable in app-server stdio mode and
    /// completes the initialization handshake.
    pub async fn spawn(executable: impl AsRef<OsStr>) -> Result<Self, CodexError> {
        Self::spawn_with_cancellation(executable, CancellationToken::new()).await
    }

    /// Opens an initialized client through a reusable app-server source.
    pub async fn connect(source: CodexAppServerSource) -> Result<Self, CodexError> {
        Self::connect_with_cancellation(&source, CancellationToken::new()).await
    }

    pub(crate) async fn spawn_with_cancellation(
        executable: impl AsRef<OsStr>,
        cancellation: CancellationToken,
    ) -> Result<Self, CodexError> {
        let source = CodexAppServerSource::executable(executable);
        Self::connect_with_cancellation(&source, cancellation).await
    }

    pub(crate) async fn connect_with_cancellation(
        source: &CodexAppServerSource,
        cancellation: CancellationToken,
    ) -> Result<Self, CodexError> {
        if cancellation.is_cancelled() {
            return Err(CodexError::Interrupted);
        }
        let transport = source.connect()?;
        let (output, input, mut shutdown) = transport.into_parts();

        let setup_result = async {
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
                connection: Some(connection),
                shutdown: Some(shutdown),
                server_info,
                cancellation,
                subscription_state: SubscriptionState::default(),
                subscribed_thread_inference: ThreadInferenceState::default(),
            }),
            Err(error) => {
                shutdown.shutdown().await;
                Err(error)
            }
        }
    }

    #[must_use]
    pub fn server_info(&self) -> &ServerInfo {
        &self.server_info
    }

    /// Returns the active model reported for the subscribed thread.
    #[must_use]
    pub fn active_model(&self) -> Option<&str> {
        self.subscribed_thread_inference.model()
    }

    /// Returns the active reasoning effort reported for the subscribed thread.
    #[must_use]
    pub fn active_reasoning_effort(&self) -> Option<&str> {
        self.subscribed_thread_inference.reasoning_effort()
    }

    /// Lists the complete visible model catalog in app-server order.
    ///
    /// Protocol pagination is consumed internally so callers always receive
    /// one authoritative catalog snapshot.
    pub async fn list_models(&mut self) -> Result<ModelCatalog, CodexError> {
        let mut models = Vec::new();
        let mut cursor = None;
        let mut requested_cursors = HashSet::new();

        loop {
            let params = ModelListParams::visible(cursor.as_deref());
            let response: ModelListResponse = self.request(MODEL_LIST_METHOD, &params).await?;
            let (mut page, next_cursor) = response.into_parts();
            models.append(&mut page);

            let Some(next_cursor) = next_cursor else {
                break;
            };
            if !requested_cursors.insert(next_cursor.clone()) {
                return Err(CodexError::UnexpectedMessage {
                    method: MODEL_LIST_METHOD,
                    description: "the app-server repeated a model-list pagination cursor"
                        .to_owned(),
                });
            }
            cursor = Some(next_cursor);
        }

        Ok(ModelCatalog { models })
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

    /// Lists all matching persisted threads in app-server order.
    ///
    /// Protocol pagination is consumed internally. Thread identifiers that
    /// overlap adjacent pages are retained only at their first position, and
    /// the returned snapshot has no continuation cursor.
    pub async fn list_all_threads(
        &mut self,
        options: &ThreadListOptions,
    ) -> Result<ThreadPage, CodexError> {
        let mut page_options = options.clone();
        let mut requested_cursors = page_options.cursor.iter().cloned().collect::<HashSet<_>>();
        let mut seen_thread_ids = HashSet::new();
        let mut threads = Vec::new();

        loop {
            let page = self.list_threads(&page_options).await?;
            threads.extend(
                page.threads
                    .into_iter()
                    .filter(|thread| seen_thread_ids.insert(thread.id.clone())),
            );

            let Some(next_cursor) = page.next_cursor else {
                break;
            };
            if !requested_cursors.insert(next_cursor.clone()) {
                return Err(CodexError::UnexpectedMessage {
                    method: THREAD_LIST_METHOD,
                    description: "the app-server repeated a thread-list pagination cursor"
                        .to_owned(),
                });
            }
            page_options.cursor = Some(next_cursor);
        }

        Ok(ThreadPage {
            threads,
            next_cursor: None,
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

    /// Sets the user-facing name of one persisted thread.
    pub async fn rename_thread(&mut self, thread_id: &str, name: &str) -> Result<(), CodexError> {
        let _: ThreadSetNameResponse = self
            .request(
                THREAD_SET_NAME_METHOD,
                &ThreadSetNameParams { thread_id, name },
            )
            .await?;
        Ok(())
    }

    /// Moves one persisted thread out of the active history list.
    pub async fn archive_thread(&mut self, thread_id: &str) -> Result<(), CodexError> {
        let _: ThreadArchiveResponse = self
            .request(THREAD_ARCHIVE_METHOD, &ThreadArchiveParams { thread_id })
            .await?;
        Ok(())
    }

    /// Copies one persisted thread, optionally through an inclusive last turn,
    /// and subscribes this connection to the fork.
    pub async fn fork_thread(
        &mut self,
        thread_id: &str,
        last_turn_id: Option<&str>,
    ) -> Result<ThreadSubscription, CodexError> {
        let response: ThreadForkResponse = self
            .request(
                THREAD_FORK_METHOD,
                &ThreadForkParams {
                    thread_id,
                    last_turn_id,
                },
            )
            .await?;
        let (subscription, inference) =
            response
                .into_parts()
                .map_err(|source| CodexError::InvalidResponse {
                    method: THREAD_FORK_METHOD,
                    source,
                })?;
        self.subscription_state.reset();
        self.subscribed_thread_inference = inference;
        Ok(subscription)
    }

    /// Restores one archived thread and returns its persisted snapshot.
    pub async fn unarchive_thread(&mut self, thread_id: &str) -> Result<Thread, CodexError> {
        let response: ThreadUnarchiveResponse = self
            .request(
                THREAD_UNARCHIVE_METHOD,
                &ThreadUnarchiveParams { thread_id },
            )
            .await?;
        response
            .into_thread()
            .map_err(|source| CodexError::InvalidResponse {
                method: THREAD_UNARCHIVE_METHOD,
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
            shutdown_app_server(&mut self.shutdown).await;
        }
        let (subscription, inference) = result?;
        self.subscription_state.reset();
        self.subscribed_thread_inference = inference;
        Ok(subscription)
    }

    /// Resumes a persisted thread and subscribes this connection to its events.
    pub async fn resume_thread(
        &mut self,
        thread_id: &str,
    ) -> Result<ThreadSubscription, CodexError> {
        let params = ThreadResumeParams { thread_id };
        let response: ThreadResumeResponse = self.request(THREAD_RESUME_METHOD, &params).await?;
        let (subscription, inference) =
            response
                .into_parts()
                .map_err(|source| CodexError::InvalidResponse {
                    method: THREAD_RESUME_METHOD,
                    source,
                })?;
        self.subscription_state.reset();
        self.subscribed_thread_inference = inference;
        Ok(subscription)
    }

    /// Starts one text turn and returns its initial streamed event.
    pub async fn begin_text_turn(
        &mut self,
        thread_id: &str,
        text: &str,
        options: TurnOptions,
    ) -> Result<ThreadStreamEvent, CodexError> {
        self.begin_turn(thread_id, &[TurnInput::Text(text.to_owned())], options)
            .await
    }

    /// Starts one turn from typed user input and returns its initial streamed event.
    pub async fn begin_turn(
        &mut self,
        thread_id: &str,
        input: &[TurnInput],
        mut options: TurnOptions,
    ) -> Result<ThreadStreamEvent, CodexError> {
        TurnStartParams::validate_input(input)?;
        if let Some(selected_model) = options
            .inference
            .as_ref()
            .and_then(crate::InferenceOverride::model_override)
            .map(str::to_owned)
            && options
                .inference
                .as_ref()
                .and_then(crate::InferenceOverride::reasoning_effort_override)
                .is_none()
        {
            let active_reasoning_effort = self
                .subscribed_thread_inference
                .reasoning_effort()
                .map(str::to_owned);
            let model = self
                .list_models()
                .await?
                .models
                .into_iter()
                .find(|model| model.model == selected_model)
                .ok_or_else(|| CodexError::UnsupportedTurnControls {
                    description: format!(
                        "the selected model is not in the visible catalog: {selected_model}"
                    ),
                })?;
            let resolved_reasoning_effort =
                model.resolve_reasoning_effort(active_reasoning_effort.as_deref());
            options
                .inference
                .as_mut()
                .expect("the model override was inspected above")
                .set_reasoning_effort(resolved_reasoning_effort);
        }
        let active_inference = self.subscribed_thread_inference.clone();
        let selected_inference = options.inference.clone();
        let connection = self
            .connection
            .as_mut()
            .ok_or(CodexError::UnexpectedEof(TURN_START_METHOD))?;
        let result = tokio::select! {
            biased;
            _ = self.cancellation.cancelled() => Err(CodexError::Interrupted),
            result = begin_turn_on_connection(
                connection,
                thread_id,
                input,
                &active_inference,
                options,
            ) => result,
        };
        if result.is_ok()
            && let Some(inference) = selected_inference.as_ref()
        {
            self.subscribed_thread_inference.apply(inference);
        }
        result
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

    /// Adds text guidance to the expected active turn on this subscribed thread.
    pub async fn steer_text_turn(
        &mut self,
        thread_id: &str,
        expected_turn_id: &str,
        text: &str,
    ) -> Result<(), CodexError> {
        let response: TurnSteerResponse = self
            .request(
                TURN_STEER_METHOD,
                &TurnSteerParams::text(thread_id, expected_turn_id, text),
            )
            .await?;
        if response.turn_id != expected_turn_id {
            return Err(CodexError::UnexpectedMessage {
                method: TURN_STEER_METHOD,
                description: format!(
                    "the app-server confirmed guidance for turn {} instead of {expected_turn_id}",
                    response.turn_id
                ),
            });
        }
        Ok(())
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

    /// Closes the connection and waits for its backing transport to stop.
    pub async fn shutdown(mut self) {
        self.connection.take();
        shutdown_app_server(&mut self.shutdown).await;
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
            shutdown_app_server(&mut self.shutdown).await;
        }
        result
    }
}

async fn shutdown_app_server(shutdown: &mut Option<AppServerShutdown>) {
    if let Some(mut shutdown) = shutdown.take() {
        shutdown.shutdown().await;
    }
}

async fn start_thread_on_connection<R, W>(
    connection: &mut Connection<R, W>,
    working_directory: &Path,
    options: ThreadStartOptions,
) -> Result<(ThreadSubscription, ThreadInferenceState), CodexError>
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
    let (subscription, inference, ephemeral) =
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
    Ok((subscription, inference))
}

async fn begin_turn_on_connection<R, W>(
    connection: &mut Connection<R, W>,
    thread_id: &str,
    input: &[TurnInput],
    active_inference: &ThreadInferenceState,
    options: TurnOptions,
) -> Result<ThreadStreamEvent, CodexError>
where
    R: AsyncBufRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let params = TurnStartParams::new(thread_id, input, active_inference, &options)?;
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::process::Stdio;
    use std::time::Duration;

    use tokio::process::Command;

    use super::*;
    use crate::app_server::terminate_child;

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
            "\"reasoningEffort\":\"high\",",
            "\"thread\":{",
            "\"id\":\"thread-new\",\"name\":null,\"preview\":\"\",",
            "\"cwd\":\"/workspace\",\"createdAt\":10,\"updatedAt\":10,",
            "\"ephemeral\":false,\"status\":{\"type\":\"idle\"},\"turns\":[]",
            "}}}\n"
        );
        let mut connection = Connection::new(BufReader::new(Cursor::new(input)), Vec::new());

        let (subscription, inference) = start_thread_on_connection(
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
        assert_eq!(inference.model(), Some("gpt-5.6-sol"));
        assert_eq!(inference.reasoning_effort(), Some("high"));
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
        let active_inference = ThreadInferenceState::default();

        events.push(
            begin_turn_on_connection(
                &mut connection,
                "thread-1",
                &[TurnInput::Text("Continue".to_owned())],
                &active_inference,
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
