// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Map, Value};

use crate::{
    Activity, ActivityKind, ActivityStatus, AgentMessagePhase, CommandAction, CommandActionKind,
    Thread, ThreadActiveFlag, ThreadItem, ThreadRuntimeStatus, ThreadSubscription, ThreadSummary,
    Turn, TurnStatus, UserInput,
};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WireThread {
    id: String,
    name: Option<String>,
    preview: String,
    cwd: PathBuf,
    created_at: i64,
    updated_at: i64,
    #[serde(default)]
    ephemeral: Option<bool>,
    status: WireThreadStatus,
    turns: Vec<WireTurn>,
}

impl WireThread {
    pub(super) fn ephemeral(&self) -> Option<bool> {
        self.ephemeral
    }

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

    pub(super) fn into_subscription(self) -> Result<ThreadSubscription, serde_json::Error> {
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
pub(super) struct WireThreadStatus {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    active_flags: Vec<String>,
}

impl WireThreadStatus {
    pub(super) fn into_model(self) -> ThreadRuntimeStatus {
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
pub(super) struct WireTurn {
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
pub(super) struct WireThreadItem {
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
pub(super) struct FileUpdateChange {
    path: PathBuf,
    diff: String,
}

pub(super) fn file_change_content(changes: &[FileUpdateChange]) -> (String, Option<String>) {
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
