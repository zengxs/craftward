// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use ward_codex::{
    Activity as CodexActivity, ActivityKind as CodexActivityKind,
    ActivityStatus as CodexActivityStatus, AgentMessagePhase, CommandAction as CodexCommandAction,
    CommandActionKind as CodexCommandActionKind, InteractionDecision, InteractionId,
    InteractionOption as CodexInteractionOption, InteractionQuestion as CodexInteractionQuestion,
    InteractionResponse as CodexInteractionResponse, InteractionResponseBody,
    PendingInteraction as CodexPendingInteraction,
    PendingInteractionKind as CodexPendingInteractionKind, Thread as CodexThread,
    ThreadItem as CodexThreadItem, ThreadPage as CodexThreadPage,
    ThreadSummary as CodexThreadSummary, UserInput,
};

include!(concat!(env!("OUT_DIR"), "/ward.codex.v1.rs"));

impl From<CodexThreadSummary> for ThreadSummary {
    fn from(thread: CodexThreadSummary) -> Self {
        Self {
            thread_id: thread.id,
            name: thread.name,
            preview: thread.preview,
            working_directory: thread.cwd.to_string_lossy().into_owned(),
            created_at_unix_seconds: thread.created_at_unix_seconds,
            updated_at_unix_seconds: thread.updated_at_unix_seconds,
        }
    }
}

impl From<CodexThreadPage> for ThreadPage {
    fn from(page: CodexThreadPage) -> Self {
        Self {
            threads: page.threads.into_iter().map(Into::into).collect(),
            next_cursor: page.next_cursor,
        }
    }
}

impl Conversation {
    pub(super) fn from_thread(thread: CodexThread, forkable_turn_ids: Vec<String>) -> Self {
        let title = thread
            .summary
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(thread.summary.preview);
        let mut timeline = Vec::new();
        for turn in thread.turns {
            for item in turn.items {
                let body = match item {
                    CodexThreadItem::Activity(activity) => {
                        Some(timeline_item::Body::Activity(activity_to_wire(activity)))
                    }
                    item => message_from_item(item).map(timeline_item::Body::Message),
                };
                if let Some(body) = body {
                    timeline.push(TimelineItem {
                        turn_id: turn.id.clone(),
                        body: Some(body),
                    });
                }
            }
        }
        Self {
            title,
            timeline,
            activity_history_is_partial: true,
            forkable_turn_ids,
        }
    }
}

impl From<CodexPendingInteraction> for PendingInteraction {
    fn from(interaction: CodexPendingInteraction) -> Self {
        Self {
            interaction_id: interaction.id.get(),
            thread_id: interaction.thread_id,
            turn_id: interaction.turn_id,
            item_id: interaction.item_id,
            kind: pending_interaction_kind_to_wire(interaction.kind) as i32,
            command: interaction.command,
            working_directory: interaction
                .working_directory
                .map(|path| path.to_string_lossy().into_owned()),
            reason: interaction.reason,
            grant_root: interaction
                .grant_root
                .map(|path| path.to_string_lossy().into_owned()),
            available_decisions: interaction
                .available_decisions
                .into_iter()
                .map(|decision| interaction_decision_to_wire(decision) as i32)
                .collect(),
            questions: interaction.questions.into_iter().map(Into::into).collect(),
            user_input_is_blocking: interaction.user_input_is_blocking,
        }
    }
}

impl From<CodexInteractionQuestion> for PendingInteractionQuestion {
    fn from(question: CodexInteractionQuestion) -> Self {
        Self {
            question_id: question.id,
            header: question.header,
            prompt: question.prompt,
            options: question.options.into_iter().map(Into::into).collect(),
            allows_other: question.allows_other,
            secret: question.secret,
        }
    }
}

impl From<CodexInteractionOption> for PendingInteractionOption {
    fn from(option: CodexInteractionOption) -> Self {
        Self {
            label: option.label,
            description: option.description,
        }
    }
}

impl TryFrom<PendingInteractionResponse> for CodexInteractionResponse {
    type Error = String;

    fn try_from(response: PendingInteractionResponse) -> Result<Self, Self::Error> {
        let interaction_id = InteractionId::new(response.interaction_id)
            .ok_or_else(|| "the Codex interaction identifier is missing".to_owned())?;
        let body = match response.body {
            Some(pending_interaction_response::Body::Decision(decision)) => {
                let decision = PendingInteractionDecision::try_from(decision)
                    .map_err(|_| "the Codex interaction decision is unknown".to_owned())?;
                InteractionResponseBody::Decision(interaction_decision_from_wire(decision)?)
            }
            Some(pending_interaction_response::Body::UserInput(user_input)) => {
                InteractionResponseBody::UserInput(
                    user_input
                        .answers
                        .into_iter()
                        .map(|answer| ward_codex::InteractionAnswer {
                            question_id: answer.question_id,
                            answers: answer.answers,
                        })
                        .collect(),
                )
            }
            None => return Err("the Codex interaction response body is missing".to_owned()),
        };
        Ok(Self {
            interaction_id,
            body,
        })
    }
}

fn pending_interaction_kind_to_wire(kind: CodexPendingInteractionKind) -> PendingInteractionKind {
    match kind {
        CodexPendingInteractionKind::CommandApproval => PendingInteractionKind::CommandApproval,
        CodexPendingInteractionKind::FileChangeApproval => {
            PendingInteractionKind::FileChangeApproval
        }
        CodexPendingInteractionKind::UserInput => PendingInteractionKind::UserInput,
        _ => PendingInteractionKind::Unspecified,
    }
}

fn interaction_decision_to_wire(decision: InteractionDecision) -> PendingInteractionDecision {
    match decision {
        InteractionDecision::Accept => PendingInteractionDecision::Accept,
        InteractionDecision::AcceptForSession => PendingInteractionDecision::AcceptForSession,
        InteractionDecision::Decline => PendingInteractionDecision::Decline,
        InteractionDecision::Cancel => PendingInteractionDecision::Cancel,
        _ => PendingInteractionDecision::Unspecified,
    }
}

fn interaction_decision_from_wire(
    decision: PendingInteractionDecision,
) -> Result<InteractionDecision, String> {
    match decision {
        PendingInteractionDecision::Accept => Ok(InteractionDecision::Accept),
        PendingInteractionDecision::AcceptForSession => Ok(InteractionDecision::AcceptForSession),
        PendingInteractionDecision::Decline => Ok(InteractionDecision::Decline),
        PendingInteractionDecision::Cancel => Ok(InteractionDecision::Cancel),
        PendingInteractionDecision::Unspecified => {
            Err("the Codex interaction decision is missing".to_owned())
        }
    }
}

fn message_from_item(item: CodexThreadItem) -> Option<Message> {
    match item {
        CodexThreadItem::UserMessage { id, content } => Some(Message {
            message_id: id,
            role: MessageRole::User as i32,
            phase: MessagePhase::Unspecified as i32,
            text: content
                .into_iter()
                .map(user_input_text)
                .collect::<Vec<_>>()
                .join("\n"),
        }),
        CodexThreadItem::AgentMessage { id, text, phase } => Some(Message {
            message_id: id,
            role: MessageRole::Agent as i32,
            phase: match phase {
                None => MessagePhase::Unspecified,
                Some(AgentMessagePhase::Commentary) => MessagePhase::Commentary,
                Some(AgentMessagePhase::FinalAnswer) => MessagePhase::FinalAnswer,
                Some(AgentMessagePhase::Unknown(_)) => MessagePhase::Other,
                Some(_) => MessagePhase::Other,
            } as i32,
            text,
        }),
        CodexThreadItem::Activity(_) => None,
        CodexThreadItem::Other { .. } => None,
        _ => None,
    }
}

fn activity_to_wire(activity: CodexActivity) -> Activity {
    Activity {
        activity_id: activity.id,
        kind: match activity.kind {
            CodexActivityKind::Reasoning => ActivityKind::Reasoning,
            CodexActivityKind::Plan => ActivityKind::Plan,
            CodexActivityKind::CommandExecution => ActivityKind::CommandExecution,
            CodexActivityKind::FileChange => ActivityKind::FileChange,
            CodexActivityKind::ToolCall => ActivityKind::ToolCall,
            CodexActivityKind::Collaboration => ActivityKind::Collaboration,
            CodexActivityKind::WebSearch => ActivityKind::WebSearch,
            CodexActivityKind::ImageView => ActivityKind::ImageView,
            CodexActivityKind::Wait => ActivityKind::Wait,
            CodexActivityKind::ImageGeneration => ActivityKind::ImageGeneration,
            CodexActivityKind::ReviewStarted => ActivityKind::ReviewStarted,
            CodexActivityKind::ReviewCompleted => ActivityKind::ReviewCompleted,
            CodexActivityKind::ContextCompaction => ActivityKind::ContextCompaction,
            _ => ActivityKind::Unspecified,
        } as i32,
        status: match activity.status {
            CodexActivityStatus::Unspecified => ActivityStatus::Unspecified,
            CodexActivityStatus::InProgress => ActivityStatus::InProgress,
            CodexActivityStatus::Completed => ActivityStatus::Completed,
            CodexActivityStatus::Failed => ActivityStatus::Failed,
            CodexActivityStatus::Declined => ActivityStatus::Declined,
            CodexActivityStatus::Unknown(_) => ActivityStatus::Other,
            _ => ActivityStatus::Other,
        } as i32,
        summary: activity.summary,
        detail: activity.detail,
        context: activity.context,
        command_actions: activity
            .command_actions
            .into_iter()
            .map(command_action_to_wire)
            .collect(),
        started_at_unix_milliseconds: activity.started_at_unix_milliseconds,
        completed_at_unix_milliseconds: activity.completed_at_unix_milliseconds,
    }
}

fn command_action_to_wire(action: CodexCommandAction) -> CommandAction {
    CommandAction {
        kind: match action.kind {
            CodexCommandActionKind::Read => CommandActionKind::Read,
            CodexCommandActionKind::ListFiles => CommandActionKind::ListFiles,
            CodexCommandActionKind::Search => CommandActionKind::Search,
            CodexCommandActionKind::Unknown => CommandActionKind::Other,
            _ => CommandActionKind::Other,
        } as i32,
        command: action.command,
        name: action.name,
        path: action.path.map(|path| path.to_string_lossy().into_owned()),
        query: action.query,
    }
}

fn user_input_text(input: UserInput) -> String {
    match input {
        UserInput::Text(text) => text,
        UserInput::Image { url } => format!("[image: {url}]"),
        UserInput::LocalImage { path } => format!("[image: {}]", path.display()),
        UserInput::Audio { url } => format!("[audio: {url}]"),
        UserInput::LocalAudio { path } => format!("[audio: {}]", path.display()),
        UserInput::Skill { name, path } => format!("[skill: {name} ({})]", path.display()),
        UserInput::Mention { name, path } => {
            format!("[mention: {name} ({})]", path.display())
        }
        UserInput::Other { kind } => format!("[{kind}]"),
        _ => "[unsupported input]".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use prost::Message as _;
    use ward_codex::{
        Activity as CodexActivity, ActivityKind as CodexActivityKind,
        ActivityStatus as CodexActivityStatus, AgentMessagePhase,
        CommandAction as CodexCommandAction, CommandActionKind as CodexCommandActionKind,
        InteractionAnswer, InteractionResponse as CodexInteractionResponse,
        InteractionResponseBody, Thread as CodexThread, ThreadItem as CodexThreadItem,
        ThreadSummary as CodexThreadSummary, Turn, TurnStatus, UserInput,
    };

    use super::*;

    #[test]
    fn serializes_a_thread_as_an_ordered_timeline() {
        let conversation = Conversation::from_thread(
            CodexThread {
                summary: CodexThreadSummary {
                    id: "thread-1".to_owned(),
                    name: Some("Example".to_owned()),
                    preview: "Preview".to_owned(),
                    cwd: PathBuf::from("/workspace"),
                    created_at_unix_seconds: 10,
                    updated_at_unix_seconds: 20,
                },
                turns: vec![Turn {
                    id: "turn-1".to_owned(),
                    status: TurnStatus::Completed,
                    items: vec![
                        CodexThreadItem::UserMessage {
                            id: "user-1".to_owned(),
                            content: vec![
                                UserInput::Text("Hello".to_owned()),
                                UserInput::LocalImage {
                                    path: PathBuf::from("/workspace/image.png"),
                                },
                            ],
                        },
                        CodexThreadItem::AgentMessage {
                            id: "commentary-1".to_owned(),
                            text: "I will inspect the file.".to_owned(),
                            phase: Some(AgentMessagePhase::Commentary),
                        },
                        CodexThreadItem::Other {
                            id: "other-1".to_owned(),
                            kind: "futureItem".to_owned(),
                        },
                        CodexThreadItem::Activity(CodexActivity {
                            id: "activity-1".to_owned(),
                            kind: CodexActivityKind::CommandExecution,
                            status: CodexActivityStatus::Completed,
                            started_at_unix_milliseconds: Some(1_000),
                            completed_at_unix_milliseconds: Some(4_250),
                            summary: "sed -n 1,80p src/main.rs".to_owned(),
                            detail: Some("fn main() {}".to_owned()),
                            context: Some("/workspace".to_owned()),
                            command_actions: vec![CodexCommandAction {
                                kind: CodexCommandActionKind::Read,
                                command: "sed -n 1,80p src/main.rs".to_owned(),
                                name: Some("src/main.rs".to_owned()),
                                path: Some(PathBuf::from("/workspace/src/main.rs")),
                                query: None,
                            }],
                        }),
                        CodexThreadItem::AgentMessage {
                            id: "agent-1".to_owned(),
                            text: "Hi".to_owned(),
                            phase: Some(AgentMessagePhase::FinalAnswer),
                        },
                    ],
                }],
            },
            vec!["turn-1".to_owned()],
        );
        let encoded = conversation.encode_to_vec();
        let decoded = Conversation::decode(encoded.as_slice()).unwrap();

        assert_eq!(decoded.title, "Example");
        assert!(decoded.activity_history_is_partial);
        assert_eq!(decoded.forkable_turn_ids, ["turn-1"]);
        assert_eq!(decoded.timeline.len(), 4);
        assert!(decoded.timeline.iter().all(|item| item.turn_id == "turn-1"));

        let timeline_item::Body::Message(user) = decoded.timeline[0].body.as_ref().unwrap() else {
            panic!("the first timeline item should be the user message");
        };
        assert_eq!(user.message_id, "user-1");
        assert_eq!(user.role(), MessageRole::User);
        assert_eq!(user.text, "Hello\n[image: /workspace/image.png]");

        let timeline_item::Body::Message(commentary) = decoded.timeline[1].body.as_ref().unwrap()
        else {
            panic!("the second timeline item should be commentary");
        };
        assert_eq!(commentary.role(), MessageRole::Agent);
        assert_eq!(commentary.phase(), MessagePhase::Commentary);
        assert_eq!(commentary.text, "I will inspect the file.");

        let timeline_item::Body::Activity(activity) = decoded.timeline[2].body.as_ref().unwrap()
        else {
            panic!("the third timeline item should be an activity");
        };
        assert_eq!(activity.activity_id, "activity-1");
        assert_eq!(activity.kind(), ActivityKind::CommandExecution);
        assert_eq!(activity.started_at_unix_milliseconds, Some(1_000));
        assert_eq!(activity.completed_at_unix_milliseconds, Some(4_250));
        assert_eq!(activity.command_actions.len(), 1);
        assert_eq!(activity.command_actions[0].kind(), CommandActionKind::Read);
        assert_eq!(
            activity.command_actions[0].path.as_deref(),
            Some("/workspace/src/main.rs")
        );

        let timeline_item::Body::Message(final_answer) = decoded.timeline[3].body.as_ref().unwrap()
        else {
            panic!("the fourth timeline item should be the final answer");
        };
        assert_eq!(final_answer.message_id, "agent-1");
        assert_eq!(final_answer.role(), MessageRole::Agent);
        assert_eq!(final_answer.phase(), MessagePhase::FinalAnswer);
        assert_eq!(final_answer.text, "Hi");
    }

    #[test]
    fn decodes_a_typed_interaction_response_from_the_private_wire_format() {
        let response = PendingInteractionResponse {
            interaction_id: 9,
            body: Some(pending_interaction_response::Body::UserInput(
                PendingUserInputResponse {
                    answers: vec![PendingInteractionAnswer {
                        question_id: "scope".to_owned(),
                        answers: vec!["Current turn".to_owned()],
                    }],
                },
            )),
        };

        assert_eq!(
            CodexInteractionResponse::try_from(response).unwrap(),
            CodexInteractionResponse {
                interaction_id: InteractionId::new(9).unwrap(),
                body: InteractionResponseBody::UserInput(vec![InteractionAnswer {
                    question_id: "scope".to_owned(),
                    answers: vec!["Current turn".to_owned()],
                }]),
            }
        );
    }

    #[test]
    fn rejects_an_unspecified_approval_decision() {
        let response = PendingInteractionResponse {
            interaction_id: 3,
            body: Some(pending_interaction_response::Body::Decision(
                PendingInteractionDecision::Unspecified as i32,
            )),
        };

        assert_eq!(
            CodexInteractionResponse::try_from(response).unwrap_err(),
            "the Codex interaction decision is missing"
        );
    }
}
