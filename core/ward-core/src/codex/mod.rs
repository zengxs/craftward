// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{CStr, c_char};

use ward_codex::{
    Activity, ActivityKind, ActivityStatus, AgentMessagePhase, CommandAction, CommandActionKind,
    Thread, ThreadItem, ThreadPage, ThreadSummary, UserInput,
};

use crate::{WardError, write_error};

mod observer;

mod wire {
    include!(concat!(env!("OUT_DIR"), "/ward.codex.v1.rs"));
}

/// An opaque serialized payload passed through Ward Core's private C interface.
pub struct WardBuffer {
    bytes: Box<[u8]>,
}

impl From<ThreadSummary> for wire::ThreadSummary {
    fn from(thread: ThreadSummary) -> Self {
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

impl From<ThreadPage> for wire::ThreadPage {
    fn from(page: ThreadPage) -> Self {
        Self {
            threads: page.threads.into_iter().map(Into::into).collect(),
            next_cursor: page.next_cursor,
        }
    }
}

impl From<Thread> for wire::Conversation {
    fn from(thread: Thread) -> Self {
        let title = thread
            .summary
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(thread.summary.preview);
        let mut timeline = Vec::new();
        for turn in thread.turns {
            for item in turn.items {
                let body = match item {
                    ThreadItem::Activity(activity) => Some(wire::timeline_item::Body::Activity(
                        activity_to_wire(activity),
                    )),
                    item => message_from_item(item).map(wire::timeline_item::Body::Message),
                };
                if let Some(body) = body {
                    timeline.push(wire::TimelineItem {
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
        }
    }
}

fn message_from_item(item: ThreadItem) -> Option<wire::Message> {
    match item {
        ThreadItem::UserMessage { id, content } => Some(wire::Message {
            message_id: id,
            role: wire::MessageRole::User as i32,
            phase: wire::MessagePhase::Unspecified as i32,
            text: content
                .into_iter()
                .map(user_input_text)
                .collect::<Vec<_>>()
                .join("\n"),
        }),
        ThreadItem::AgentMessage { id, text, phase } => Some(wire::Message {
            message_id: id,
            role: wire::MessageRole::Agent as i32,
            phase: match phase {
                None => wire::MessagePhase::Unspecified,
                Some(AgentMessagePhase::Commentary) => wire::MessagePhase::Commentary,
                Some(AgentMessagePhase::FinalAnswer) => wire::MessagePhase::FinalAnswer,
                Some(AgentMessagePhase::Unknown(_)) => wire::MessagePhase::Other,
                Some(_) => wire::MessagePhase::Other,
            } as i32,
            text,
        }),
        ThreadItem::Activity(_) => None,
        ThreadItem::Other { .. } => None,
        _ => None,
    }
}

fn activity_to_wire(activity: Activity) -> wire::Activity {
    wire::Activity {
        activity_id: activity.id,
        kind: match activity.kind {
            ActivityKind::Plan => wire::ActivityKind::Plan,
            ActivityKind::CommandExecution => wire::ActivityKind::CommandExecution,
            ActivityKind::FileChange => wire::ActivityKind::FileChange,
            ActivityKind::ToolCall => wire::ActivityKind::ToolCall,
            ActivityKind::Collaboration => wire::ActivityKind::Collaboration,
            ActivityKind::WebSearch => wire::ActivityKind::WebSearch,
            ActivityKind::ImageView => wire::ActivityKind::ImageView,
            ActivityKind::Wait => wire::ActivityKind::Wait,
            ActivityKind::ImageGeneration => wire::ActivityKind::ImageGeneration,
            ActivityKind::ReviewStarted => wire::ActivityKind::ReviewStarted,
            ActivityKind::ReviewCompleted => wire::ActivityKind::ReviewCompleted,
            ActivityKind::ContextCompaction => wire::ActivityKind::ContextCompaction,
            _ => wire::ActivityKind::Unspecified,
        } as i32,
        status: match activity.status {
            ActivityStatus::Unspecified => wire::ActivityStatus::Unspecified,
            ActivityStatus::InProgress => wire::ActivityStatus::InProgress,
            ActivityStatus::Completed => wire::ActivityStatus::Completed,
            ActivityStatus::Failed => wire::ActivityStatus::Failed,
            ActivityStatus::Declined => wire::ActivityStatus::Declined,
            ActivityStatus::Unknown(_) => wire::ActivityStatus::Other,
            _ => wire::ActivityStatus::Other,
        } as i32,
        summary: activity.summary,
        detail: activity.detail,
        context: activity.context,
        command_actions: activity
            .command_actions
            .into_iter()
            .map(command_action_to_wire)
            .collect(),
    }
}

fn command_action_to_wire(action: CommandAction) -> wire::CommandAction {
    wire::CommandAction {
        kind: match action.kind {
            CommandActionKind::Read => wire::CommandActionKind::Read,
            CommandActionKind::ListFiles => wire::CommandActionKind::ListFiles,
            CommandActionKind::Search => wire::CommandActionKind::Search,
            CommandActionKind::Unknown => wire::CommandActionKind::Other,
            _ => wire::CommandActionKind::Other,
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

unsafe fn clear_error(output_error: *mut *mut WardError) {
    if !output_error.is_null() {
        // SAFETY: The C caller supplied a writable error output pointer.
        unsafe { *output_error = std::ptr::null_mut() };
    }
}

unsafe fn required_string(
    value: *const c_char,
    name: &'static str,
    output_error: *mut *mut WardError,
) -> Option<String> {
    if value.is_null() {
        // SAFETY: The caller supplied the optional error output pointer.
        unsafe { write_error(output_error, format!("{name} is missing")) };
        return None;
    }
    // SAFETY: The private C interface requires a NUL-terminated UTF-8 string.
    Some(
        unsafe { CStr::from_ptr(value) }
            .to_string_lossy()
            .into_owned(),
    )
}

/// Returns the borrowed bytes in a serialized Ward buffer.
///
/// The returned pointer remains valid for the lifetime of the borrowed buffer.
///
/// # Safety
///
/// `buffer` must be null or a valid borrowed handle supplied by Ward Core.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_buffer_data(buffer: *const WardBuffer) -> *const u8 {
    // SAFETY: A non-null pointer names a valid borrowed handle.
    unsafe { buffer.as_ref() }.map_or(std::ptr::null(), |buffer| buffer.bytes.as_ptr())
}

/// Returns the number of bytes in a serialized Ward buffer.
///
/// # Safety
///
/// `buffer` must be null or a valid borrowed handle supplied by Ward Core.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_buffer_size(buffer: *const WardBuffer) -> usize {
    // SAFETY: A non-null pointer names a valid borrowed handle.
    unsafe { buffer.as_ref() }.map_or(0, |buffer| buffer.bytes.len())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use prost::Message as _;
    use ward_codex::{ThreadSummary, Turn, TurnStatus};

    use super::*;

    #[test]
    fn serializes_a_thread_as_an_ordered_timeline() {
        let conversation = wire::Conversation::from(Thread {
            summary: ThreadSummary {
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
                    ThreadItem::UserMessage {
                        id: "user-1".to_owned(),
                        content: vec![
                            UserInput::Text("Hello".to_owned()),
                            UserInput::LocalImage {
                                path: PathBuf::from("/workspace/image.png"),
                            },
                        ],
                    },
                    ThreadItem::AgentMessage {
                        id: "commentary-1".to_owned(),
                        text: "I will inspect the file.".to_owned(),
                        phase: Some(AgentMessagePhase::Commentary),
                    },
                    ThreadItem::Other {
                        id: "other-1".to_owned(),
                        kind: "futureItem".to_owned(),
                    },
                    ThreadItem::Activity(Activity {
                        id: "activity-1".to_owned(),
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
                    ThreadItem::AgentMessage {
                        id: "agent-1".to_owned(),
                        text: "Hi".to_owned(),
                        phase: Some(AgentMessagePhase::FinalAnswer),
                    },
                ],
            }],
        });
        let encoded = conversation.encode_to_vec();
        let decoded = wire::Conversation::decode(encoded.as_slice()).unwrap();

        assert_eq!(decoded.title, "Example");
        assert!(decoded.activity_history_is_partial);
        assert_eq!(decoded.timeline.len(), 4);
        assert!(decoded.timeline.iter().all(|item| item.turn_id == "turn-1"));

        let wire::timeline_item::Body::Message(user) = decoded.timeline[0].body.as_ref().unwrap()
        else {
            panic!("the first timeline item should be the user message");
        };
        assert_eq!(user.message_id, "user-1");
        assert_eq!(user.role(), wire::MessageRole::User);
        assert_eq!(user.text, "Hello\n[image: /workspace/image.png]");

        let wire::timeline_item::Body::Message(commentary) =
            decoded.timeline[1].body.as_ref().unwrap()
        else {
            panic!("the second timeline item should be commentary");
        };
        assert_eq!(commentary.role(), wire::MessageRole::Agent);
        assert_eq!(commentary.phase(), wire::MessagePhase::Commentary);
        assert_eq!(commentary.text, "I will inspect the file.");

        let wire::timeline_item::Body::Activity(activity) =
            decoded.timeline[2].body.as_ref().unwrap()
        else {
            panic!("the third timeline item should be an activity");
        };
        assert_eq!(activity.activity_id, "activity-1");
        assert_eq!(activity.kind(), wire::ActivityKind::CommandExecution);
        assert_eq!(activity.command_actions.len(), 1);
        assert_eq!(
            activity.command_actions[0].kind(),
            wire::CommandActionKind::Read
        );
        assert_eq!(
            activity.command_actions[0].path.as_deref(),
            Some("/workspace/src/main.rs")
        );

        let wire::timeline_item::Body::Message(final_answer) =
            decoded.timeline[3].body.as_ref().unwrap()
        else {
            panic!("the fourth timeline item should be the final answer");
        };
        assert_eq!(final_answer.message_id, "agent-1");
        assert_eq!(final_answer.role(), wire::MessageRole::Agent);
        assert_eq!(final_answer.phase(), wire::MessagePhase::FinalAnswer);
        assert_eq!(final_answer.text, "Hi");
    }
}
