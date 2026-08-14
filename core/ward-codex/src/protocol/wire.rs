// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    Activity, ActivityKind, ActivityStatus, ActivityUpdate, AgentMessagePhase, CommandAction,
    CommandActionKind, ServerInfo, Thread, ThreadActiveFlag, ThreadItem, ThreadRuntimeStatus,
    ThreadStreamEvent, ThreadSubscription, ThreadSummary, Turn, TurnStatus, UserInput,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InitializeParams {
    client_info: ClientInfo,
    capabilities: InitializeCapabilities,
}

impl InitializeParams {
    pub(crate) fn craftward() -> Self {
        Self {
            client_info: ClientInfo {
                name: "craftward",
                title: "Craftward",
                version: env!("CARGO_PKG_VERSION"),
            },
            capabilities: InitializeCapabilities {
                experimental_api: false,
            },
        }
    }
}

#[derive(Serialize)]
struct ClientInfo {
    name: &'static str,
    title: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InitializeCapabilities {
    experimental_api: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct InitializeResponse {
    codex_home: PathBuf,
    platform_family: String,
    platform_os: String,
    user_agent: String,
}

impl From<InitializeResponse> for ServerInfo {
    fn from(value: InitializeResponse) -> Self {
        Self {
            codex_home: value.codex_home,
            platform_family: value.platform_family,
            platform_os: value.platform_os,
            user_agent: value.user_agent,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadListParams<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    cursor: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    archived: Option<bool>,
    use_state_db_only: bool,
}

impl<'a> ThreadListParams<'a> {
    pub(crate) fn new(cursor: Option<&'a str>, limit: Option<u32>, archived: Option<bool>) -> Self {
        Self {
            cursor,
            limit,
            archived,
            // Avoid the scan-and-repair behavior of the default endpoint while
            // Craftward is operating as a read-only history viewer.
            use_state_db_only: true,
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadListResponse {
    pub(crate) data: Vec<WireThread>,
    pub(crate) next_cursor: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadReadParams<'a> {
    pub(crate) thread_id: &'a str,
    pub(crate) include_turns: bool,
}

#[derive(Deserialize)]
pub(crate) struct ThreadReadResponse {
    pub(crate) thread: WireThread,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadResumeParams<'a> {
    pub(crate) thread_id: &'a str,
}

#[derive(Deserialize)]
pub(crate) struct ThreadResumeResponse {
    pub(crate) thread: WireThread,
}

impl ThreadResumeResponse {
    pub(crate) fn into_subscription(self) -> Result<ThreadSubscription, serde_json::Error> {
        self.thread.into_subscription()
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TurnStartParams<'a> {
    pub(crate) thread_id: &'a str,
    input: Vec<TextTurnInput<'a>>,
}

impl<'a> TurnStartParams<'a> {
    pub(crate) fn text(thread_id: &'a str, text: &'a str) -> Self {
        Self {
            thread_id,
            input: vec![TextTurnInput { kind: "text", text }],
        }
    }
}

#[derive(Serialize)]
struct TextTurnInput<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    text: &'a str,
}

#[derive(Deserialize)]
pub(crate) struct TurnStartResponse {
    turn: WireTurn,
}

impl TurnStartResponse {
    pub(crate) fn into_turn(self) -> Result<Turn, serde_json::Error> {
        self.turn.into_model()
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WireThread {
    id: String,
    name: Option<String>,
    preview: String,
    cwd: PathBuf,
    created_at: i64,
    updated_at: i64,
    status: WireThreadStatus,
    turns: Vec<WireTurn>,
}

impl WireThread {
    pub(crate) fn into_summary(self) -> ThreadSummary {
        make_summary(
            self.id,
            self.name,
            self.preview,
            self.cwd,
            self.created_at,
            self.updated_at,
        )
    }

    pub(crate) fn into_thread(self) -> Result<Thread, serde_json::Error> {
        Ok(self.into_subscription()?.thread)
    }

    fn into_subscription(self) -> Result<ThreadSubscription, serde_json::Error> {
        let summary = make_summary(
            self.id,
            self.name,
            self.preview,
            self.cwd,
            self.created_at,
            self.updated_at,
        );
        let turns = self
            .turns
            .into_iter()
            .map(WireTurn::into_model)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ThreadSubscription {
            thread: Thread { summary, turns },
            runtime_status: self.status.into_model(),
        })
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireThreadStatus {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    active_flags: Vec<String>,
}

impl WireThreadStatus {
    fn into_model(self) -> ThreadRuntimeStatus {
        match self.kind.as_str() {
            "notLoaded" => ThreadRuntimeStatus::NotLoaded,
            "idle" => ThreadRuntimeStatus::Idle,
            "active" => ThreadRuntimeStatus::Active {
                active_flags: self
                    .active_flags
                    .into_iter()
                    .map(|flag| match flag.as_str() {
                        "waitingOnApproval" => ThreadActiveFlag::WaitingOnApproval,
                        "waitingOnUserInput" => ThreadActiveFlag::WaitingOnUserInput,
                        _ => ThreadActiveFlag::Unknown(flag),
                    })
                    .collect(),
            },
            "systemError" => ThreadRuntimeStatus::SystemError,
            _ => ThreadRuntimeStatus::Unknown(self.kind),
        }
    }
}

fn make_summary(
    id: String,
    name: Option<String>,
    preview: String,
    cwd: PathBuf,
    created_at_unix_seconds: i64,
    updated_at_unix_seconds: i64,
) -> ThreadSummary {
    ThreadSummary {
        id,
        name,
        preview,
        cwd,
        created_at_unix_seconds,
        updated_at_unix_seconds,
    }
}

#[derive(Deserialize)]
struct WireTurn {
    id: String,
    status: String,
    items: Vec<WireThreadItem>,
}

impl WireTurn {
    pub(crate) fn into_model(self) -> Result<Turn, serde_json::Error> {
        let items = self
            .items
            .into_iter()
            .map(WireThreadItem::into_model)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Turn {
            id: self.id,
            status: match self.status.as_str() {
                "completed" => TurnStatus::Completed,
                "interrupted" => TurnStatus::Interrupted,
                "failed" => TurnStatus::Failed,
                "inProgress" => TurnStatus::InProgress,
                _ => TurnStatus::Unknown(self.status),
            },
            items,
        })
    }
}

#[derive(Deserialize)]
struct WireThreadItem {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(flatten)]
    fields: Map<String, Value>,
}

impl WireThreadItem {
    pub(crate) fn into_model(self) -> Result<ThreadItem, serde_json::Error> {
        match self.kind.as_str() {
            "userMessage" => {
                let fields: UserMessageFields = serde_json::from_value(Value::Object(self.fields))?;
                let content = fields
                    .content
                    .into_iter()
                    .map(WireUserInput::into_model)
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ThreadItem::UserMessage {
                    id: self.id,
                    content,
                })
            }
            "agentMessage" => {
                let fields: AgentMessageFields =
                    serde_json::from_value(Value::Object(self.fields))?;
                Ok(ThreadItem::AgentMessage {
                    id: self.id,
                    text: fields.text,
                    phase: fields.phase.map(|phase| match phase.as_str() {
                        "commentary" => AgentMessagePhase::Commentary,
                        "final_answer" => AgentMessagePhase::FinalAnswer,
                        _ => AgentMessagePhase::Unknown(phase),
                    }),
                })
            }
            "plan" => {
                let fields: PlanFields = serde_json::from_value(Value::Object(self.fields))?;
                Ok(activity(
                    self.id,
                    ActivityKind::Plan,
                    ActivityStatus::Unspecified,
                    fields.text,
                ))
            }
            "reasoning" => {
                let fields: ReasoningFields = serde_json::from_value(Value::Object(self.fields))?;
                Ok(activity(
                    self.id,
                    ActivityKind::Reasoning,
                    ActivityStatus::Unspecified,
                    fields.summary.join("\n"),
                ))
            }
            "commandExecution" => {
                let fields: CommandExecutionFields =
                    serde_json::from_value(Value::Object(self.fields))?;
                Ok(ThreadItem::Activity(Activity {
                    id: self.id,
                    kind: ActivityKind::CommandExecution,
                    status: activity_status(fields.status),
                    started_at_unix_milliseconds: None,
                    completed_at_unix_milliseconds: None,
                    summary: fields.command,
                    detail: nonempty(fields.aggregated_output),
                    context: Some(fields.cwd.to_string_lossy().into_owned()),
                    command_actions: fields
                        .command_actions
                        .into_iter()
                        .map(WireCommandAction::into_model)
                        .collect(),
                }))
            }
            "fileChange" => {
                let fields: FileChangeFields = serde_json::from_value(Value::Object(self.fields))?;
                let (summary, detail) = file_change_content(&fields.changes);
                Ok(ThreadItem::Activity(Activity {
                    id: self.id,
                    kind: ActivityKind::FileChange,
                    status: activity_status(fields.status),
                    started_at_unix_milliseconds: None,
                    completed_at_unix_milliseconds: None,
                    summary,
                    detail,
                    context: None,
                    command_actions: Vec::new(),
                }))
            }
            "mcpToolCall" => {
                let fields: McpToolCallFields = serde_json::from_value(Value::Object(self.fields))?;
                Ok(ThreadItem::Activity(Activity {
                    id: self.id,
                    kind: ActivityKind::ToolCall,
                    status: activity_status(fields.status),
                    started_at_unix_milliseconds: None,
                    completed_at_unix_milliseconds: None,
                    summary: format!("{} / {}", fields.server, fields.tool),
                    detail: json_detail(fields.arguments),
                    context: None,
                    command_actions: Vec::new(),
                }))
            }
            "dynamicToolCall" => {
                let fields: DynamicToolCallFields =
                    serde_json::from_value(Value::Object(self.fields))?;
                let summary = match fields.namespace {
                    Some(namespace) if !namespace.trim().is_empty() => {
                        format!("{namespace} / {}", fields.tool)
                    }
                    _ => fields.tool,
                };
                Ok(ThreadItem::Activity(Activity {
                    id: self.id,
                    kind: ActivityKind::ToolCall,
                    status: activity_status(fields.status),
                    started_at_unix_milliseconds: None,
                    completed_at_unix_milliseconds: None,
                    summary,
                    detail: json_detail(fields.arguments),
                    context: None,
                    command_actions: Vec::new(),
                }))
            }
            "collabAgentToolCall" => {
                let fields: CollabAgentToolCallFields =
                    serde_json::from_value(Value::Object(self.fields))?;
                Ok(ThreadItem::Activity(Activity {
                    id: self.id,
                    kind: ActivityKind::Collaboration,
                    status: activity_status(fields.status),
                    started_at_unix_milliseconds: None,
                    completed_at_unix_milliseconds: None,
                    summary: fields.tool,
                    detail: nonempty(fields.prompt),
                    context: nonempty(fields.model),
                    command_actions: Vec::new(),
                }))
            }
            "subAgentActivity" => {
                let fields: SubAgentActivityFields =
                    serde_json::from_value(Value::Object(self.fields))?;
                Ok(ThreadItem::Activity(Activity {
                    id: self.id,
                    kind: ActivityKind::Collaboration,
                    status: ActivityStatus::Unspecified,
                    started_at_unix_milliseconds: None,
                    completed_at_unix_milliseconds: None,
                    summary: fields.agent_path,
                    detail: nonempty(Some(fields.kind)),
                    context: nonempty(Some(fields.agent_thread_id)),
                    command_actions: Vec::new(),
                }))
            }
            "webSearch" => {
                let fields: WebSearchFields = serde_json::from_value(Value::Object(self.fields))?;
                Ok(activity(
                    self.id,
                    ActivityKind::WebSearch,
                    ActivityStatus::Unspecified,
                    fields.query,
                ))
            }
            "imageView" => {
                let fields: ImageViewFields = serde_json::from_value(Value::Object(self.fields))?;
                Ok(activity(
                    self.id,
                    ActivityKind::ImageView,
                    ActivityStatus::Unspecified,
                    fields.path.to_string_lossy().into_owned(),
                ))
            }
            "sleep" => {
                let fields: SleepFields = serde_json::from_value(Value::Object(self.fields))?;
                Ok(activity(
                    self.id,
                    ActivityKind::Wait,
                    ActivityStatus::Completed,
                    format!("{} ms", fields.duration_ms),
                ))
            }
            "imageGeneration" => {
                let fields: ImageGenerationFields =
                    serde_json::from_value(Value::Object(self.fields))?;
                let saved_path = fields
                    .saved_path
                    .map(|path| path.to_string_lossy().into_owned());
                let summary = fields
                    .revised_prompt
                    .filter(|prompt| !prompt.trim().is_empty())
                    .or_else(|| saved_path.clone())
                    .unwrap_or_else(|| fields.result.clone());
                Ok(ThreadItem::Activity(Activity {
                    id: self.id,
                    kind: ActivityKind::ImageGeneration,
                    status: activity_status(fields.status),
                    started_at_unix_milliseconds: None,
                    completed_at_unix_milliseconds: None,
                    summary,
                    detail: nonempty(Some(fields.result)),
                    context: saved_path,
                    command_actions: Vec::new(),
                }))
            }
            "enteredReviewMode" => {
                let fields: ReviewFields = serde_json::from_value(Value::Object(self.fields))?;
                Ok(activity(
                    self.id,
                    ActivityKind::ReviewStarted,
                    ActivityStatus::Completed,
                    fields.review,
                ))
            }
            "exitedReviewMode" => {
                let fields: ReviewFields = serde_json::from_value(Value::Object(self.fields))?;
                Ok(activity(
                    self.id,
                    ActivityKind::ReviewCompleted,
                    ActivityStatus::Completed,
                    fields.review,
                ))
            }
            "contextCompaction" => Ok(activity(
                self.id,
                ActivityKind::ContextCompaction,
                ActivityStatus::Unspecified,
                String::new(),
            )),
            _ => Ok(ThreadItem::Other {
                id: self.id,
                kind: self.kind,
            }),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnNotification {
    thread_id: String,
    turn: WireTurn,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemNotification {
    thread_id: String,
    turn_id: String,
    item: WireThreadItem,
    #[serde(default)]
    started_at_ms: Option<i64>,
    #[serde(default)]
    completed_at_ms: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeltaNotification {
    thread_id: String,
    turn_id: String,
    item_id: String,
    delta: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReasoningSummaryPartNotification {
    thread_id: String,
    turn_id: String,
    item_id: String,
    summary_index: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorNotification {
    thread_id: String,
    turn_id: String,
    error: TurnErrorFields,
    will_retry: bool,
}

#[derive(Deserialize)]
struct TurnErrorFields {
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadStatusNotification {
    thread_id: String,
    status: WireThreadStatus,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileChangePatchNotification {
    thread_id: String,
    turn_id: String,
    item_id: String,
    changes: Vec<FileUpdateChange>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpToolCallProgressNotification {
    thread_id: String,
    turn_id: String,
    item_id: String,
    message: String,
}

pub(crate) fn turn_stream_event(
    method: &str,
    params: Value,
) -> Result<Option<ThreadStreamEvent>, serde_json::Error> {
    match method {
        "thread/status/changed" => {
            let notification: ThreadStatusNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::ThreadStatusChanged {
                thread_id: notification.thread_id,
                status: notification.status.into_model(),
            }))
        }
        "turn/started" => {
            let notification: TurnNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::TurnStarted {
                thread_id: notification.thread_id,
                turn: notification.turn.into_model()?,
            }))
        }
        "item/started" | "item/completed" => {
            let notification: ItemNotification = serde_json::from_value(params)?;
            let mut item = notification.item.into_model()?;
            if let ThreadItem::Activity(activity) = &mut item
                && activity.status == ActivityStatus::Unspecified
            {
                activity.status = if method == "item/started" {
                    ActivityStatus::InProgress
                } else {
                    ActivityStatus::Completed
                };
            }
            if let ThreadItem::Activity(activity) = &mut item {
                if method == "item/started" {
                    activity.started_at_unix_milliseconds = notification.started_at_ms;
                } else {
                    activity.completed_at_unix_milliseconds = notification.completed_at_ms;
                }
            }
            let event = if method == "item/started" {
                ThreadStreamEvent::ItemStarted {
                    thread_id: notification.thread_id,
                    turn_id: notification.turn_id,
                    item,
                }
            } else {
                ThreadStreamEvent::ItemCompleted {
                    thread_id: notification.thread_id,
                    turn_id: notification.turn_id,
                    item,
                }
            };
            Ok(Some(event))
        }
        "item/agentMessage/delta" => {
            let notification: DeltaNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::AgentMessageDelta {
                thread_id: notification.thread_id,
                turn_id: notification.turn_id,
                item_id: notification.item_id,
                delta: notification.delta,
            }))
        }
        "item/plan/delta" | "item/reasoning/summaryTextDelta" => {
            let notification: DeltaNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::ActivityUpdated {
                thread_id: notification.thread_id,
                turn_id: notification.turn_id,
                item_id: notification.item_id,
                update: ActivityUpdate::AppendSummary(notification.delta),
            }))
        }
        "item/reasoning/summaryPartAdded" => {
            let notification: ReasoningSummaryPartNotification = serde_json::from_value(params)?;
            if notification.summary_index == 0 {
                return Ok(None);
            }
            Ok(Some(ThreadStreamEvent::ActivityUpdated {
                thread_id: notification.thread_id,
                turn_id: notification.turn_id,
                item_id: notification.item_id,
                update: ActivityUpdate::StartSummarySection,
            }))
        }
        "item/commandExecution/outputDelta" | "item/fileChange/outputDelta" => {
            let notification: DeltaNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::ActivityOutputDelta {
                thread_id: notification.thread_id,
                turn_id: notification.turn_id,
                item_id: notification.item_id,
                delta: notification.delta,
            }))
        }
        "item/fileChange/patchUpdated" => {
            let notification: FileChangePatchNotification = serde_json::from_value(params)?;
            let (summary, detail) = file_change_content(&notification.changes);
            Ok(Some(ThreadStreamEvent::ActivityUpdated {
                thread_id: notification.thread_id,
                turn_id: notification.turn_id,
                item_id: notification.item_id,
                update: ActivityUpdate::ReplaceContent { summary, detail },
            }))
        }
        "item/mcpToolCall/progress" => {
            let notification: McpToolCallProgressNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::ActivityUpdated {
                thread_id: notification.thread_id,
                turn_id: notification.turn_id,
                item_id: notification.item_id,
                update: ActivityUpdate::AppendDetail(format!("{}\n", notification.message)),
            }))
        }
        "error" => {
            let notification: ErrorNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::RuntimeError {
                thread_id: notification.thread_id,
                turn_id: notification.turn_id,
                message: notification.error.message,
                will_retry: notification.will_retry,
            }))
        }
        "turn/completed" => {
            let notification: TurnNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::TurnCompleted {
                thread_id: notification.thread_id,
                turn: notification.turn.into_model()?,
            }))
        }
        _ => Ok(None),
    }
}

#[derive(Deserialize)]
struct UserMessageFields {
    content: Vec<WireUserInput>,
}

#[derive(Deserialize)]
struct AgentMessageFields {
    text: String,
    #[serde(default)]
    phase: Option<String>,
}

#[derive(Deserialize)]
struct PlanFields {
    text: String,
}

#[derive(Deserialize)]
struct ReasoningFields {
    #[serde(default)]
    summary: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandExecutionFields {
    command: String,
    command_actions: Vec<WireCommandAction>,
    cwd: PathBuf,
    status: String,
    #[serde(default)]
    aggregated_output: Option<String>,
}

#[derive(Deserialize)]
struct WireCommandAction {
    command: String,
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    path: Option<PathBuf>,
    #[serde(default)]
    query: Option<String>,
}

impl WireCommandAction {
    fn into_model(self) -> CommandAction {
        CommandAction {
            kind: match self.kind.as_str() {
                "read" => CommandActionKind::Read,
                "listFiles" => CommandActionKind::ListFiles,
                "search" => CommandActionKind::Search,
                _ => CommandActionKind::Unknown,
            },
            command: self.command,
            name: self.name,
            path: self.path,
            query: self.query,
        }
    }
}

#[derive(Deserialize)]
struct FileChangeFields {
    changes: Vec<FileUpdateChange>,
    status: String,
}

#[derive(Deserialize)]
struct FileUpdateChange {
    path: PathBuf,
    diff: String,
}

fn file_change_content(changes: &[FileUpdateChange]) -> (String, Option<String>) {
    let summary = changes
        .iter()
        .map(|change| change.path.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join(", ");
    let detail = changes
        .iter()
        .filter(|change| !change.diff.trim().is_empty())
        .map(|change| format!("{}\n{}", change.path.to_string_lossy(), change.diff.trim()))
        .collect::<Vec<_>>()
        .join("\n\n");
    (summary, nonempty(Some(detail)))
}

#[derive(Deserialize)]
struct McpToolCallFields {
    arguments: Value,
    server: String,
    status: String,
    tool: String,
}

#[derive(Deserialize)]
struct DynamicToolCallFields {
    arguments: Value,
    #[serde(default)]
    namespace: Option<String>,
    status: String,
    tool: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CollabAgentToolCallFields {
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    prompt: Option<String>,
    status: String,
    tool: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubAgentActivityFields {
    agent_path: String,
    agent_thread_id: String,
    kind: String,
}

#[derive(Deserialize)]
struct WebSearchFields {
    query: String,
}

#[derive(Deserialize)]
struct ImageViewFields {
    path: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SleepFields {
    duration_ms: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ImageGenerationFields {
    result: String,
    #[serde(default)]
    revised_prompt: Option<String>,
    #[serde(default)]
    saved_path: Option<PathBuf>,
    status: String,
}

#[derive(Deserialize)]
struct ReviewFields {
    review: String,
}

fn activity(id: String, kind: ActivityKind, status: ActivityStatus, summary: String) -> ThreadItem {
    ThreadItem::Activity(Activity {
        id,
        kind,
        status,
        started_at_unix_milliseconds: None,
        completed_at_unix_milliseconds: None,
        summary,
        detail: None,
        context: None,
        command_actions: Vec::new(),
    })
}

fn activity_status(status: String) -> ActivityStatus {
    match status.as_str() {
        "inProgress" => ActivityStatus::InProgress,
        "completed" => ActivityStatus::Completed,
        "failed" => ActivityStatus::Failed,
        "declined" => ActivityStatus::Declined,
        _ => ActivityStatus::Unknown(status),
    }
}

fn nonempty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn json_detail(value: Value) -> Option<String> {
    match &value {
        Value::Null => None,
        Value::Object(fields) if fields.is_empty() => None,
        Value::Array(items) if items.is_empty() => None,
        _ => Some(serde_json::to_string_pretty(&value).expect("JSON values always serialize")),
    }
}

#[derive(Deserialize)]
struct WireUserInput {
    #[serde(rename = "type")]
    kind: String,
    #[serde(flatten)]
    fields: Map<String, Value>,
}

impl WireUserInput {
    fn into_model(self) -> Result<UserInput, serde_json::Error> {
        let fields = Value::Object(self.fields);
        match self.kind.as_str() {
            "text" => {
                let fields: TextInputFields = serde_json::from_value(fields)?;
                Ok(UserInput::Text(fields.text))
            }
            "image" => {
                let fields: UrlInputFields = serde_json::from_value(fields)?;
                Ok(UserInput::Image { url: fields.url })
            }
            "localImage" => {
                let fields: PathInputFields = serde_json::from_value(fields)?;
                Ok(UserInput::LocalImage { path: fields.path })
            }
            "audio" => {
                let fields: UrlInputFields = serde_json::from_value(fields)?;
                Ok(UserInput::Audio { url: fields.url })
            }
            "localAudio" => {
                let fields: PathInputFields = serde_json::from_value(fields)?;
                Ok(UserInput::LocalAudio { path: fields.path })
            }
            "skill" => {
                let fields: NamedPathInputFields = serde_json::from_value(fields)?;
                Ok(UserInput::Skill {
                    name: fields.name,
                    path: fields.path,
                })
            }
            "mention" => {
                let fields: NamedPathInputFields = serde_json::from_value(fields)?;
                Ok(UserInput::Mention {
                    name: fields.name,
                    path: fields.path,
                })
            }
            _ => Ok(UserInput::Other { kind: self.kind }),
        }
    }
}

#[derive(Deserialize)]
struct TextInputFields {
    text: String,
}

#[derive(Deserialize)]
struct UrlInputFields {
    url: String,
}

#[derive(Deserialize)]
struct PathInputFields {
    path: PathBuf,
}

#[derive(Deserialize)]
struct NamedPathInputFields {
    name: String,
    path: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

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

        let subscription = response
            .into_subscription()
            .expect("the subscription should map");

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
            serde_json::to_value(TurnStartParams::text("thread-1", "Continue")).unwrap(),
            serde_json::json!({
                "threadId": "thread-1",
                "input": [{ "type": "text", "text": "Continue" }]
            })
        );
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
}
