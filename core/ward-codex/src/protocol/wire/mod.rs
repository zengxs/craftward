// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub(crate) use self::interactions::{
    interaction_result, pending_interaction, resolved_server_request,
};
pub(crate) use self::model_catalog::{ModelListParams, ModelListResponse};
pub(crate) use self::notifications::turn_stream_event;
use self::thread::{WireThread, WireTurn};
use crate::{
    CodexError, ServerInfo, ThreadInferenceState, ThreadStartOptions, ThreadSubscription, Turn,
    TurnInput, TurnMode, TurnOptions, TurnPermissionPreset,
};

mod interactions;
mod model_catalog;
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
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadForkResponse {
    thread: WireThread,
    model: String,
    reasoning_effort: Option<String>,
}

impl ThreadForkResponse {
    pub(crate) fn into_parts(
        self,
    ) -> Result<(ThreadSubscription, ThreadInferenceState), serde_json::Error> {
        Ok((
            self.thread.into_subscription()?,
            ThreadInferenceState::new(Some(self.model), self.reasoning_effort),
        ))
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
#[serde(rename_all = "camelCase")]
pub(crate) struct ThreadStartResponse {
    thread: WireThread,
    model: String,
    reasoning_effort: Option<String>,
}

impl ThreadStartResponse {
    pub(crate) fn into_parts(
        self,
    ) -> Result<(ThreadSubscription, ThreadInferenceState, Option<bool>), serde_json::Error> {
        let ephemeral = self.thread.ephemeral();
        Ok((
            self.thread.into_subscription()?,
            ThreadInferenceState::new(Some(self.model), self.reasoning_effort),
            ephemeral,
        ))
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
    #[serde(default)]
    reasoning_effort: Option<String>,
}

impl ThreadResumeResponse {
    pub(crate) fn into_parts(
        self,
    ) -> Result<(ThreadSubscription, ThreadInferenceState), serde_json::Error> {
        Ok((
            self.thread.into_subscription()?,
            ThreadInferenceState::new(self.model, self.reasoning_effort),
        ))
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct TurnStartParams<'a> {
    pub(crate) thread_id: &'a str,
    input: Vec<WireTurnInput<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effort: Option<&'a str>,
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
    pub(crate) fn validate_input(input: &'a [TurnInput]) -> Result<(), CodexError> {
        if input.is_empty() {
            return Err(CodexError::InvalidTurnInput {
                description: "at least one input item is required".to_owned(),
            });
        }
        for input in input {
            match input {
                TurnInput::Text(text) if text.trim().is_empty() => {
                    return Err(CodexError::InvalidTurnInput {
                        description: "text input is empty".to_owned(),
                    });
                }
                TurnInput::LocalImage { path } if path.as_os_str().is_empty() => {
                    return Err(CodexError::InvalidTurnInput {
                        description: "a local image path is empty".to_owned(),
                    });
                }
                TurnInput::LocalAudio { path } if path.as_os_str().is_empty() => {
                    return Err(CodexError::InvalidTurnInput {
                        description: "a local audio path is empty".to_owned(),
                    });
                }
                TurnInput::Mention { name, .. } if name.trim().is_empty() => {
                    return Err(CodexError::InvalidTurnInput {
                        description: "a mentioned file name is empty".to_owned(),
                    });
                }
                TurnInput::Mention { path, .. } if path.as_os_str().is_empty() => {
                    return Err(CodexError::InvalidTurnInput {
                        description: "a mentioned file path is empty".to_owned(),
                    });
                }
                TurnInput::Text(_)
                | TurnInput::LocalImage { .. }
                | TurnInput::LocalAudio { .. }
                | TurnInput::Mention { .. } => {}
            }
        }
        Ok(())
    }

    pub(crate) fn new(
        thread_id: &'a str,
        input: &'a [TurnInput],
        active_inference: &'a ThreadInferenceState,
        options: &'a TurnOptions,
    ) -> Result<Self, CodexError> {
        Self::validate_input(input)?;
        let input = input
            .iter()
            .map(|input| match input {
                TurnInput::Text(text) => WireTurnInput::Text { text },
                TurnInput::LocalImage { path } => WireTurnInput::LocalImage { path },
                TurnInput::LocalAudio { path } => WireTurnInput::LocalAudio { path },
                TurnInput::Mention { name, path } => WireTurnInput::Mention { name, path },
            })
            .collect();
        Self::with_input(thread_id, input, active_inference, options)
    }

    #[cfg(test)]
    pub(crate) fn text(
        thread_id: &'a str,
        text: &'a str,
        active_inference: &'a ThreadInferenceState,
        options: &'a TurnOptions,
    ) -> Result<Self, CodexError> {
        if text.trim().is_empty() {
            return Err(CodexError::InvalidTurnInput {
                description: "text input is empty".to_owned(),
            });
        }
        Self::with_input(
            thread_id,
            vec![WireTurnInput::Text { text }],
            active_inference,
            options,
        )
    }

    fn with_input(
        thread_id: &'a str,
        input: Vec<WireTurnInput<'a>>,
        active_inference: &'a ThreadInferenceState,
        options: &'a TurnOptions,
    ) -> Result<Self, CodexError> {
        let selected_model = options
            .inference
            .as_ref()
            .and_then(crate::InferenceOverride::model_override);
        let selected_reasoning_effort = options
            .inference
            .as_ref()
            .and_then(crate::InferenceOverride::reasoning_effort_override)
            .map(crate::ReasoningEffort::as_str);
        if selected_model.is_some_and(str::is_empty) {
            return Err(CodexError::UnsupportedTurnControls {
                description: "the selected model is empty".to_owned(),
            });
        }
        if selected_reasoning_effort.is_some_and(str::is_empty) {
            return Err(CodexError::UnsupportedTurnControls {
                description: "the selected reasoning effort is empty".to_owned(),
            });
        }
        let collaboration_model = selected_model.or(active_inference.model());
        let collaboration_reasoning_effort =
            selected_reasoning_effort.or(active_inference.reasoning_effort());
        let collaboration_mode = match collaboration_model {
            Some(model) => Some(CollaborationMode::new(
                options.mode,
                model,
                collaboration_reasoning_effort,
            )),
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
            input,
            model: selected_model,
            effort: selected_reasoning_effort,
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
    fn new(mode: TurnMode, model: &'a str, reasoning_effort: Option<&'a str>) -> Self {
        Self {
            mode: match mode {
                TurnMode::Default => WireTurnMode::Default,
                TurnMode::Plan => WireTurnMode::Plan,
            },
            settings: CollaborationSettings {
                developer_instructions: None,
                model,
                reasoning_effort,
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
#[serde(tag = "type", rename_all = "camelCase")]
enum WireTurnInput<'a> {
    Text { text: &'a str },
    LocalImage { path: &'a Path },
    LocalAudio { path: &'a Path },
    Mention { name: &'a str, path: &'a Path },
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
    input: Vec<WireTurnInput<'a>>,
}

impl<'a> TurnSteerParams<'a> {
    pub(crate) fn text(thread_id: &'a str, expected_turn_id: &'a str, text: &'a str) -> Self {
        Self {
            thread_id,
            expected_turn_id,
            input: vec![WireTurnInput::Text { text }],
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
