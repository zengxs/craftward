// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    InteractionDecision, InteractionId, InteractionOption, InteractionQuestion,
    InteractionResponse, InteractionResponseBody, PendingInteraction, PendingInteractionKind,
};

const COMMAND_APPROVAL_METHOD: &str = "item/commandExecution/requestApproval";
const FILE_CHANGE_APPROVAL_METHOD: &str = "item/fileChange/requestApproval";
const USER_INPUT_METHOD: &str = "item/tool/requestUserInput";

pub(crate) fn pending_interaction(
    interaction_id: InteractionId,
    method: &str,
    params: Value,
) -> Result<Option<PendingInteraction>, serde_json::Error> {
    match method {
        COMMAND_APPROVAL_METHOD => {
            let params: CommandApprovalParams = serde_json::from_value(params)?;
            Ok(Some(PendingInteraction {
                id: interaction_id,
                thread_id: params.context.thread_id,
                turn_id: Some(params.context.turn_id),
                item_id: Some(params.context.item_id),
                kind: PendingInteractionKind::CommandApproval,
                command: params.command,
                working_directory: params.cwd,
                reason: params.reason,
                grant_root: None,
                available_decisions: approval_decisions(),
                questions: vec![],
                user_input_is_blocking: true,
            }))
        }
        FILE_CHANGE_APPROVAL_METHOD => {
            let params: FileChangeApprovalParams = serde_json::from_value(params)?;
            Ok(Some(PendingInteraction {
                id: interaction_id,
                thread_id: params.context.thread_id,
                turn_id: Some(params.context.turn_id),
                item_id: Some(params.context.item_id),
                kind: PendingInteractionKind::FileChangeApproval,
                command: None,
                working_directory: None,
                reason: params.reason,
                grant_root: params.grant_root,
                available_decisions: approval_decisions(),
                questions: vec![],
                user_input_is_blocking: true,
            }))
        }
        USER_INPUT_METHOD => {
            let params: UserInputParams = serde_json::from_value(params)?;
            Ok(Some(PendingInteraction {
                id: interaction_id,
                thread_id: params.context.thread_id,
                turn_id: Some(params.context.turn_id),
                item_id: Some(params.context.item_id),
                kind: PendingInteractionKind::UserInput,
                command: None,
                working_directory: None,
                reason: None,
                grant_root: None,
                available_decisions: vec![],
                questions: params.questions.into_iter().map(Into::into).collect(),
                user_input_is_blocking: params.is_blocking,
            }))
        }
        _ => Ok(None),
    }
}

pub(crate) fn interaction_result(
    interaction: &PendingInteraction,
    response: &InteractionResponse,
) -> Result<Value, String> {
    if response.interaction_id != interaction.id {
        return Err("the response identifier does not match the pending interaction".to_owned());
    }

    match (&interaction.kind, &response.body) {
        (
            PendingInteractionKind::CommandApproval | PendingInteractionKind::FileChangeApproval,
            InteractionResponseBody::Decision(decision),
        ) => {
            if !interaction.available_decisions.contains(decision) {
                return Err(
                    "the selected decision is not available for this interaction".to_owned(),
                );
            }
            Ok(json!({ "decision": decision_name(*decision) }))
        }
        (PendingInteractionKind::UserInput, InteractionResponseBody::UserInput(answers)) => {
            let mut supplied = BTreeMap::new();
            for answer in answers {
                if supplied
                    .insert(answer.question_id.as_str(), answer.answers.as_slice())
                    .is_some()
                {
                    return Err(format!(
                        "question {} was answered more than once",
                        answer.question_id
                    ));
                }
            }

            let mut encoded = serde_json::Map::new();
            for question in &interaction.questions {
                let Some(answers) = supplied.remove(question.id.as_str()) else {
                    return Err(format!("question {} has no answer", question.id));
                };
                encoded.insert(question.id.clone(), json!({ "answers": answers }));
            }
            if let Some(question_id) = supplied.keys().next() {
                return Err(format!(
                    "question {question_id} is not part of this interaction"
                ));
            }
            Ok(json!({ "answers": encoded }))
        }
        _ => Err("the response kind does not match the pending interaction".to_owned()),
    }
}

pub(crate) fn resolved_server_request(params: Value) -> Result<(Value, String), serde_json::Error> {
    let params: ServerRequestResolvedParams = serde_json::from_value(params)?;
    Ok((params.request_id, params.thread_id))
}

fn approval_decisions() -> Vec<InteractionDecision> {
    vec![
        InteractionDecision::Accept,
        InteractionDecision::AcceptForSession,
        InteractionDecision::Decline,
        InteractionDecision::Cancel,
    ]
}

fn decision_name(decision: InteractionDecision) -> &'static str {
    match decision {
        InteractionDecision::Accept => "accept",
        InteractionDecision::AcceptForSession => "acceptForSession",
        InteractionDecision::Decline => "decline",
        InteractionDecision::Cancel => "cancel",
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandApprovalParams {
    #[serde(flatten)]
    context: InteractionRequestContext,
    command: Option<String>,
    cwd: Option<PathBuf>,
    reason: Option<String>,
    #[allow(dead_code)]
    started_at_ms: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileChangeApprovalParams {
    #[serde(flatten)]
    context: InteractionRequestContext,
    reason: Option<String>,
    grant_root: Option<PathBuf>,
    #[allow(dead_code)]
    started_at_ms: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserInputParams {
    #[serde(flatten)]
    context: InteractionRequestContext,
    is_blocking: bool,
    questions: Vec<WireQuestion>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InteractionRequestContext {
    thread_id: String,
    turn_id: String,
    item_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServerRequestResolvedParams {
    request_id: Value,
    thread_id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireQuestion {
    id: String,
    header: String,
    question: String,
    #[serde(default)]
    options: Option<Vec<WireOption>>,
    #[serde(default)]
    is_other: bool,
    #[serde(default)]
    is_secret: bool,
}

impl From<WireQuestion> for InteractionQuestion {
    fn from(value: WireQuestion) -> Self {
        Self {
            id: value.id,
            header: value.header,
            prompt: value.question,
            options: value
                .options
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            allows_other: value.is_other,
            secret: value.is_secret,
        }
    }
}

#[derive(Deserialize)]
struct WireOption {
    label: String,
    description: String,
}

impl From<WireOption> for InteractionOption {
    fn from(value: WireOption) -> Self {
        Self {
            label: value.label,
            description: value.description,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InteractionAnswer;

    fn interaction_id(value: u64) -> InteractionId {
        InteractionId::new(value).unwrap()
    }

    #[test]
    fn normalizes_every_scalar_command_approval_decision() {
        let interaction = pending_interaction(
            interaction_id(7),
            COMMAND_APPROVAL_METHOD,
            json!({
                "threadId": "thread-1",
                "turnId": "turn-2",
                "itemId": "command-1",
                "command": "cargo test",
                "cwd": "/workspace",
                "reason": "Run the test suite",
                "startedAtMs": 42,
            }),
        )
        .unwrap()
        .unwrap();

        assert_eq!(interaction.id, interaction_id(7));
        assert_eq!(interaction.kind, PendingInteractionKind::CommandApproval);
        assert_eq!(interaction.command.as_deref(), Some("cargo test"));
        assert_eq!(
            interaction.working_directory,
            Some(PathBuf::from("/workspace"))
        );
        assert_eq!(interaction.available_decisions, approval_decisions());
    }

    #[test]
    fn requires_the_schema_mandated_approval_timestamp() {
        let result = pending_interaction(
            interaction_id(1),
            FILE_CHANGE_APPROVAL_METHOD,
            json!({
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "change-1",
            }),
        );

        assert!(result.is_err());
    }

    #[test]
    fn encodes_structured_user_input_answers_by_question_id() {
        let interaction = pending_interaction(
            interaction_id(11),
            USER_INPUT_METHOD,
            json!({
                "threadId": "thread-1",
                "turnId": "turn-3",
                "itemId": "tool-1",
                "isBlocking": true,
                "questions": [{
                    "id": "scope",
                    "header": "Scope",
                    "question": "Which scope?",
                    "options": [{ "label": "Current", "description": "Only this turn" }],
                    "isOther": true,
                    "isSecret": false
                }]
            }),
        )
        .unwrap()
        .unwrap();
        let response = InteractionResponse {
            interaction_id: interaction_id(11),
            body: InteractionResponseBody::UserInput(vec![InteractionAnswer {
                question_id: "scope".to_owned(),
                answers: vec!["Current".to_owned()],
            }]),
        };

        assert_eq!(
            interaction_result(&interaction, &response).unwrap(),
            json!({ "answers": { "scope": { "answers": ["Current"] } } })
        );
    }

    #[test]
    fn rejects_a_response_kind_that_does_not_match_the_request() {
        let interaction = pending_interaction(
            interaction_id(5),
            FILE_CHANGE_APPROVAL_METHOD,
            json!({
                "threadId": "thread-1",
                "turnId": "turn-1",
                "itemId": "change-1",
                "startedAtMs": 9,
            }),
        )
        .unwrap()
        .unwrap();
        let response = InteractionResponse {
            interaction_id: interaction_id(5),
            body: InteractionResponseBody::UserInput(vec![]),
        };

        assert_eq!(
            interaction_result(&interaction, &response).unwrap_err(),
            "the response kind does not match the pending interaction"
        );
    }

    #[test]
    fn validates_the_resolved_server_request_notification() {
        assert_eq!(
            resolved_server_request(json!({
                "requestId": "approval-1",
                "threadId": "thread-1",
            }))
            .unwrap(),
            (json!("approval-1"), "thread-1".to_owned())
        );
        assert!(resolved_server_request(json!({ "requestId": 7 })).is_err());
    }
}
