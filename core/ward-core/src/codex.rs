// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{CStr, c_char};
use std::path::PathBuf;

use prost::Message as ProstMessage;
use ward_codex::{
    AgentMessagePhase, CodexClient, Thread, ThreadItem, ThreadListOptions, ThreadSummary, UserInput,
};

use crate::{WardError, write_error};

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

impl From<Thread> for wire::Conversation {
    fn from(thread: Thread) -> Self {
        let title = thread
            .summary
            .name
            .filter(|name| !name.trim().is_empty())
            .unwrap_or(thread.summary.preview);
        let messages = thread
            .turns
            .into_iter()
            .flat_map(|turn| turn.items)
            .filter_map(message_from_item)
            .collect();
        Self { title, messages }
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
        ThreadItem::Other { .. } => None,
        _ => None,
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

fn serialized_buffer(message: &impl ProstMessage) -> *mut WardBuffer {
    Box::into_raw(Box::new(WardBuffer {
        bytes: message.encode_to_vec().into_boxed_slice(),
    }))
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

/// Loads and serializes one page of active Codex thread summaries.
///
/// # Safety
///
/// `executable` must point to a NUL-terminated string. `output_error`, when
/// non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_list_threads(
    executable: *const c_char,
    limit: u32,
    output_error: *mut *mut WardError,
) -> *mut WardBuffer {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    // SAFETY: The private C interface requires the documented string pointer.
    let Some(executable) =
        (unsafe { required_string(executable, "the Codex executable", output_error) })
    else {
        return std::ptr::null_mut();
    };

    let result = CodexClient::spawn(PathBuf::from(executable)).and_then(|mut client| {
        client.list_threads(&ThreadListOptions {
            limit: Some(limit),
            ..ThreadListOptions::default()
        })
    });
    match result {
        Ok(page) => serialized_buffer(&wire::ThreadPage {
            threads: page.threads.into_iter().map(Into::into).collect(),
            next_cursor: page.next_cursor,
        }),
        Err(error) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, error.to_string()) };
            std::ptr::null_mut()
        }
    }
}

/// Loads and serializes the persisted conversation for one Codex thread.
///
/// # Safety
///
/// `executable` and `thread_id` must point to NUL-terminated strings.
/// `output_error`, when non-null, must be writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_codex_read_thread(
    executable: *const c_char,
    thread_id: *const c_char,
    output_error: *mut *mut WardError,
) -> *mut WardBuffer {
    // SAFETY: The caller supplied the optional error output pointer.
    unsafe { clear_error(output_error) };
    // SAFETY: The private C interface requires the documented string pointers.
    let Some(executable) =
        (unsafe { required_string(executable, "the Codex executable", output_error) })
    else {
        return std::ptr::null_mut();
    };
    // SAFETY: The private C interface requires the documented string pointers.
    let Some(thread_id) =
        (unsafe { required_string(thread_id, "the Codex thread identifier", output_error) })
    else {
        return std::ptr::null_mut();
    };

    let result = CodexClient::spawn(PathBuf::from(executable))
        .and_then(|mut client| client.read_thread(&thread_id));
    match result {
        Ok(thread) => serialized_buffer(&wire::Conversation::from(thread)),
        Err(error) => {
            // SAFETY: The caller supplied the optional error output pointer.
            unsafe { write_error(output_error, error.to_string()) };
            std::ptr::null_mut()
        }
    }
}

/// Returns the borrowed bytes in a serialized Ward buffer.
///
/// The returned pointer remains valid until [`ward_core_buffer_destroy`] is
/// called for the same handle.
///
/// # Safety
///
/// `buffer` must be null or a live handle returned by Ward Core.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_buffer_data(buffer: *const WardBuffer) -> *const u8 {
    // SAFETY: A non-null pointer names a live handle owned by the caller.
    unsafe { buffer.as_ref() }.map_or(std::ptr::null(), |buffer| buffer.bytes.as_ptr())
}

/// Returns the number of bytes in a serialized Ward buffer.
///
/// # Safety
///
/// `buffer` must be null or a live handle returned by Ward Core.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_buffer_size(buffer: *const WardBuffer) -> usize {
    // SAFETY: A non-null pointer names a live handle owned by the caller.
    unsafe { buffer.as_ref() }.map_or(0, |buffer| buffer.bytes.len())
}

/// Destroys a serialized Ward buffer.
///
/// # Safety
///
/// `buffer` must be null or a live handle returned by Ward Core, and ownership
/// may be transferred once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ward_core_buffer_destroy(buffer: *mut WardBuffer) {
    if !buffer.is_null() {
        // SAFETY: The caller transfers the live handle exactly once.
        drop(unsafe { Box::from_raw(buffer) });
    }
}

#[cfg(test)]
mod tests {
    use ward_codex::{ThreadSummary, Turn, TurnStatus};

    use super::*;

    #[test]
    fn serializes_a_thread_as_displayable_messages() {
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
                    ThreadItem::Other {
                        id: "other-1".to_owned(),
                        kind: "commandExecution".to_owned(),
                    },
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
        assert_eq!(decoded.messages.len(), 2);
        assert_eq!(decoded.messages[0].message_id, "user-1");
        assert_eq!(decoded.messages[0].role(), wire::MessageRole::User);
        assert_eq!(
            decoded.messages[0].text,
            "Hello\n[image: /workspace/image.png]"
        );
        assert_eq!(decoded.messages[1].message_id, "agent-1");
        assert_eq!(decoded.messages[1].role(), wire::MessageRole::Agent);
        assert_eq!(decoded.messages[1].phase(), wire::MessagePhase::FinalAnswer);
        assert_eq!(decoded.messages[1].text, "Hi");
    }
}
