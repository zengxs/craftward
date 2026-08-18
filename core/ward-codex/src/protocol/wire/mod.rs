// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub(crate) use self::interactions::{
    interaction_result, pending_interaction, resolved_server_request,
};
pub(crate) use self::notifications::turn_stream_event;
use self::thread::{WireThread, WireTurn};
use crate::{
    CodexError, ServerInfo, ThreadStartOptions, ThreadSubscription, Turn, TurnMode, TurnOptions,
    TurnPermissionPreset,
};

mod interactions;
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
                experimental_api: true,
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
pub(crate) struct ThreadArchiveParams<'a> {
    pub(crate) thread_id: &'a str,
}

#[derive(Deserialize)]
pub(crate) struct ThreadArchiveResponse {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadForkParams<'a> {
    pub(crate) thread_id: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) last_turn_id: Option<&'a str>,
}

#[derive(Deserialize)]
pub(crate) struct ThreadForkResponse {
    thread: WireThread,
    model: String,
}

impl ThreadForkResponse {
    pub(crate) fn into_parts(self) -> Result<(ThreadSubscription, String), serde_json::Error> {
        Ok((self.thread.into_subscription()?, self.model))
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
pub(crate) struct ThreadSetNameParams<'a> {
    pub(crate) thread_id: &'a str,
    pub(crate) name: &'a str,
}

#[derive(Deserialize)]
pub(crate) struct ThreadSetNameResponse {}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadUnarchiveParams<'a> {
    pub(crate) thread_id: &'a str,
}

#[derive(Deserialize)]
pub(crate) struct ThreadUnarchiveResponse {
    thread: WireThread,
}

impl ThreadUnarchiveResponse {
    pub(crate) fn into_thread(self) -> Result<crate::Thread, serde_json::Error> {
        self.thread.into_thread()
    }
}

#[derive(Serialize)]
pub(crate) struct ThreadStartParams<'a> {
    cwd: &'a Path,
    #[serde(skip_serializing_if = "is_false")]
    ephemeral: bool,
}

impl<'a> ThreadStartParams<'a> {
    pub(crate) fn new(cwd: &'a Path, options: ThreadStartOptions) -> Self {
        Self {
            cwd,
            ephemeral: options.ephemeral,
        }
    }
}

fn is_false(value: &bool) -> bool {
    !value
}

#[derive(Deserialize)]
pub(crate) struct ThreadStartResponse {
    thread: WireThread,
    model: String,
}

impl ThreadStartResponse {
    pub(crate) fn into_parts(
        self,
    ) -> Result<(ThreadSubscription, String, Option<bool>), serde_json::Error> {
        let ephemeral = self.thread.ephemeral();
        Ok((self.thread.into_subscription()?, self.model, ephemeral))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadResumeParams<'a> {
    pub(crate) thread_id: &'a str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadResumeResponse {
    pub(crate) thread: WireThread,
    #[serde(default)]
    model: Option<String>,
}

impl ThreadResumeResponse {
    pub(crate) fn into_parts(
        self,
    ) -> Result<(ThreadSubscription, Option<String>), serde_json::Error> {
        Ok((self.thread.into_subscription()?, self.model))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TurnStartParams<'a> {
    pub(crate) thread_id: &'a str,
    input: Vec<TextTurnInput<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    collaboration_mode: Option<CollaborationMode<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    approval_policy: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    approvals_reviewer: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sandbox_policy: Option<SandboxPolicy>,
}

impl<'a> TurnStartParams<'a> {
    pub(crate) fn text(
        thread_id: &'a str,
        text: &'a str,
        model: Option<&'a str>,
        options: TurnOptions,
    ) -> Result<Self, CodexError> {
        let collaboration_mode = match model {
            Some(model) => Some(CollaborationMode::new(options.mode, model)),
            None if options.mode == TurnMode::Default => None,
            None => {
                return Err(CodexError::UnsupportedTurnControls {
                    description:
                        "the app-server did not report the active model required for Plan mode"
                            .to_owned(),
                });
            }
        };
        let (approval_policy, approvals_reviewer, sandbox_policy) = match options.permission_preset
        {
            TurnPermissionPreset::Inherit => (None, None, None),
            TurnPermissionPreset::RequestApproval => (
                Some("on-request"),
                Some("user"),
                Some(SandboxPolicy::WorkspaceWrite {
                    network_access: false,
                }),
            ),
            TurnPermissionPreset::ReadOnly => (
                Some("on-request"),
                Some("user"),
                Some(SandboxPolicy::ReadOnly),
            ),
        };
        Ok(Self {
            thread_id,
            input: vec![TextTurnInput { kind: "text", text }],
            collaboration_mode,
            approval_policy,
            approvals_reviewer,
            sandbox_policy,
        })
    }
}

#[derive(Serialize)]
struct CollaborationMode<'a> {
    mode: WireTurnMode,
    settings: CollaborationSettings<'a>,
}

impl<'a> CollaborationMode<'a> {
    fn new(mode: TurnMode, model: &'a str) -> Self {
        Self {
            mode: match mode {
                TurnMode::Default => WireTurnMode::Default,
                TurnMode::Plan => WireTurnMode::Plan,
            },
            settings: CollaborationSettings {
                developer_instructions: None,
                model,
                reasoning_effort: None,
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "lowercase")]
enum WireTurnMode {
    Default,
    Plan,
}

#[derive(Serialize)]
struct CollaborationSettings<'a> {
    developer_instructions: Option<&'a str>,
    model: &'a str,
    reasoning_effort: Option<&'a str>,
}

#[derive(Serialize)]
#[serde(tag = "type")]
enum SandboxPolicy {
    #[serde(rename = "workspaceWrite")]
    WorkspaceWrite {
        #[serde(rename = "networkAccess")]
        network_access: bool,
    },
    #[serde(rename = "readOnly")]
    ReadOnly,
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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TurnSteerParams<'a> {
    pub(crate) thread_id: &'a str,
    pub(crate) expected_turn_id: &'a str,
    input: Vec<TextTurnInput<'a>>,
}

impl<'a> TurnSteerParams<'a> {
    pub(crate) fn text(thread_id: &'a str, expected_turn_id: &'a str, text: &'a str) -> Self {
        Self {
            thread_id,
            expected_turn_id,
            input: vec![TextTurnInput { kind: "text", text }],
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TurnSteerResponse {
    pub(crate) turn_id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TurnInterruptParams<'a> {
    pub(crate) thread_id: &'a str,
    pub(crate) turn_id: &'a str,
}

#[derive(Deserialize)]
pub(crate) struct TurnInterruptResponse {}

impl TurnStartResponse {
    pub(crate) fn into_turn(self) -> Result<Turn, serde_json::Error> {
        self.turn.into_model()
    }
}
