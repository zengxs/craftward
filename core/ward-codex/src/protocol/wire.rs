// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{
    AgentMessagePhase, ServerInfo, Thread, ThreadItem, ThreadSummary, Turn, TurnStatus, UserInput,
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
                    kind: "futureItem".to_owned()
                }
            ]
        );
    }
}
