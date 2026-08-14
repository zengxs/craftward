// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub(crate) use self::notifications::turn_stream_event;
use self::thread::{WireThread, WireTurn};
use crate::{ServerInfo, ThreadSubscription, Turn};

mod notifications;
mod thread;

#[cfg(test)]
mod tests;

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
