// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    Activity, ActivityKind, ActivityStatus, AgentMessagePhase, CommandAction, CommandActionKind,
    ServerInfo, Thread, ThreadItem, ThreadSummary, Turn, TurnStatus, UserInput,
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

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct WireThread {
    id: String,
    name: Option<String>,
    preview: String,
    cwd: PathBuf,
    created_at: i64,
    updated_at: i64,
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
        Ok(Thread { summary, turns })
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
    fn into_model(self) -> Result<Turn, serde_json::Error> {
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
    fn into_model(self) -> Result<ThreadItem, serde_json::Error> {
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
            "reasoning" => Ok(ThreadItem::Other {
                id: self.id,
                kind: self.kind,
            }),
            "commandExecution" => {
                let fields: CommandExecutionFields =
                    serde_json::from_value(Value::Object(self.fields))?;
                Ok(ThreadItem::Activity(Activity {
                    id: self.id,
                    kind: ActivityKind::CommandExecution,
                    status: activity_status(fields.status),
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
                let summary = fields
                    .changes
                    .iter()
                    .map(|change| change.path.to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join(", ");
                let detail = fields
                    .changes
                    .iter()
                    .filter(|change| !change.diff.trim().is_empty())
                    .map(|change| {
                        format!("{}\n{}", change.path.to_string_lossy(), change.diff.trim())
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                Ok(ThreadItem::Activity(Activity {
                    id: self.id,
                    kind: ActivityKind::FileChange,
                    status: activity_status(fields.status),
                    summary,
                    detail: nonempty(Some(detail)),
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
                ActivityStatus::Completed,
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
                ThreadItem::Other {
                    id: "item-3".to_owned(),
                    kind: "reasoning".to_owned()
                },
                ThreadItem::Activity(Activity {
                    id: "item-4".to_owned(),
                    kind: ActivityKind::CommandExecution,
                    status: ActivityStatus::Completed,
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
}
