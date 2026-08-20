// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::CString;
use std::sync::Arc;

use tokio::runtime::Handle;
use tokio::sync::mpsc;
use ward_codex::{
    CodexHistoryCancellation, InferenceOverride, ReasoningEffort, TurnInput, TurnMode, TurnOptions,
    TurnPermissionPreset,
};

use super::commands::{
    ObserverCommand, ThreadForkRequest, ThreadLifecycleAction, ThreadLifecycleRequest,
    ThreadListScope, ThreadRenameRequest, TurnRequest,
};
use super::{
    COMMAND_QUEUE_CAPACITY, ObserverOperation, ObserverOperationGate, WardCodexHistoryObserver,
    WardCodexPermissionPreset, WardCodexTurnAttachment, WardCodexTurnAttachmentKind,
    WardCodexTurnMode, decode_turn_options, ward_core_codex_history_observer_archive_thread_async,
    ward_core_codex_history_observer_fork_thread_async,
    ward_core_codex_history_observer_rename_thread_async,
    ward_core_codex_history_observer_restore_thread_async,
    ward_core_codex_history_observer_show_archived_async,
    ward_core_codex_history_observer_start_turn_async,
};

#[tokio::test]
async fn queues_a_thread_fork_through_the_private_c_interface() {
    let (commands, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let observer = WardCodexHistoryObserver {
        commands,
        cancellation: CodexHistoryCancellation::new(),
        active_operation: Arc::new(ObserverOperationGate::new()),
        runtime: Handle::current(),
        worker: None,
    };
    let thread_id = CString::new("thread-1").expect("the thread ID is valid");
    let last_turn_id = CString::new("turn-2").expect("the turn ID is valid");
    let mut error = std::ptr::null_mut();

    // SAFETY: The observer, thread ID, and turn ID remain live for the duration
    // of the private C interface call, and the error output pointer is writable.
    assert!(unsafe {
        ward_core_codex_history_observer_fork_thread_async(
            std::ptr::from_ref(&observer).cast_mut(),
            thread_id.as_ptr(),
            last_turn_id.as_ptr(),
            &mut error,
        )
    });
    assert!(error.is_null());
    assert_eq!(
        receiver.recv().await,
        Some(ObserverCommand::ForkThread(ThreadForkRequest {
            thread_id: "thread-1".to_owned(),
            last_turn_id: "turn-2".to_owned(),
        }))
    );
}

#[tokio::test]
async fn queues_conversation_inference_overrides_through_the_private_c_interface() {
    let (commands, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let observer = WardCodexHistoryObserver {
        commands,
        cancellation: CodexHistoryCancellation::new(),
        active_operation: Arc::new(ObserverOperationGate::new()),
        runtime: Handle::current(),
        worker: None,
    };
    let thread_id = CString::new("thread-1").expect("the thread ID is valid");
    let prompt = CString::new("Continue").expect("the prompt is valid");
    let model = CString::new("gpt-fast").expect("the model is valid");
    let reasoning_effort = CString::new("low").expect("the reasoning effort is valid");
    let mut error = std::ptr::null_mut();

    // SAFETY: The observer and strings remain live for the duration of the
    // private C interface call, and the error output pointer is writable.
    assert!(unsafe {
        ward_core_codex_history_observer_start_turn_async(
            std::ptr::from_ref(&observer).cast_mut(),
            thread_id.as_ptr(),
            prompt.as_ptr(),
            std::ptr::null(),
            0,
            model.as_ptr(),
            reasoning_effort.as_ptr(),
            WardCodexTurnMode::Default,
            WardCodexPermissionPreset::Inherit,
            &mut error,
        )
    });
    assert!(error.is_null());
    assert_eq!(
        receiver.recv().await,
        Some(ObserverCommand::StartTurn(TurnRequest {
            thread_id: "thread-1".to_owned(),
            input: vec![TurnInput::Text("Continue".to_owned())],
            options: TurnOptions {
                inference: ReasoningEffort::new("low")
                    .map(|effort| InferenceOverride::selection("gpt-fast", effort)),
                ..TurnOptions::default()
            },
        }))
    );
}

#[tokio::test]
async fn queues_typed_attachments_through_the_private_c_interface() {
    let (commands, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let observer = WardCodexHistoryObserver {
        commands,
        cancellation: CodexHistoryCancellation::new(),
        active_operation: Arc::new(ObserverOperationGate::new()),
        runtime: Handle::current(),
        worker: None,
    };
    let thread_id = CString::new("thread-1").expect("the thread ID is valid");
    let prompt = CString::new("").expect("the empty prompt is valid");
    let image_name = CString::new("first.png").expect("the image name is valid");
    let image_path = CString::new("/workspace/first.png").expect("the image path is valid");
    let audio_name = CString::new("note.wav").expect("the audio name is valid");
    let audio_path = CString::new("/workspace/note.wav").expect("the audio path is valid");
    let file_name = CString::new("requirements.pdf").expect("the file name is valid");
    let file_path = CString::new("/workspace/requirements.pdf").expect("the file path is valid");
    let attachments = [
        WardCodexTurnAttachment {
            kind: WardCodexTurnAttachmentKind::LocalImage,
            name: image_name.as_ptr(),
            path: image_path.as_ptr(),
        },
        WardCodexTurnAttachment {
            kind: WardCodexTurnAttachmentKind::LocalAudio,
            name: audio_name.as_ptr(),
            path: audio_path.as_ptr(),
        },
        WardCodexTurnAttachment {
            kind: WardCodexTurnAttachmentKind::Mention,
            name: file_name.as_ptr(),
            path: file_path.as_ptr(),
        },
    ];
    let mut error = std::ptr::null_mut();

    // SAFETY: The observer, strings, and attachment array remain live for the
    // duration of the private C interface call, and the error output pointer
    // is writable.
    assert!(unsafe {
        ward_core_codex_history_observer_start_turn_async(
            std::ptr::from_ref(&observer).cast_mut(),
            thread_id.as_ptr(),
            prompt.as_ptr(),
            attachments.as_ptr(),
            attachments.len(),
            std::ptr::null(),
            std::ptr::null(),
            WardCodexTurnMode::Default,
            WardCodexPermissionPreset::Inherit,
            &mut error,
        )
    });
    assert!(error.is_null());
    assert_eq!(
        receiver.recv().await,
        Some(ObserverCommand::StartTurn(TurnRequest {
            thread_id: "thread-1".to_owned(),
            input: vec![
                TurnInput::LocalImage {
                    path: "/workspace/first.png".into(),
                },
                TurnInput::LocalAudio {
                    path: "/workspace/note.wav".into(),
                },
                TurnInput::Mention {
                    name: "requirements.pdf".to_owned(),
                    path: "/workspace/requirements.pdf".into(),
                },
            ],
            options: TurnOptions::default(),
        }))
    );
}

#[test]
fn reserves_only_one_observer_operation_at_a_time() {
    let gate = ObserverOperationGate::new();

    assert!(gate.reserve(ObserverOperation::ThreadStart).is_ok());
    assert!(matches!(
        gate.reserve(ObserverOperation::Turn),
        Err(ObserverOperation::ThreadStart)
    ));
    gate.release();
    assert!(gate.reserve(ObserverOperation::Turn).is_ok());
    gate.release();
    assert!(gate.reserve(ObserverOperation::ThreadFork).is_ok());
    assert!(matches!(
        gate.reserve(ObserverOperation::ThreadStart),
        Err(ObserverOperation::ThreadFork)
    ));
}

#[test]
fn decodes_the_private_turn_control_values() {
    assert_eq!(
        decode_turn_options(
            WardCodexTurnMode::Plan,
            WardCodexPermissionPreset::RequestApproval,
        ),
        TurnOptions {
            mode: TurnMode::Plan,
            permission_preset: TurnPermissionPreset::RequestApproval,
            inference: None,
        }
    );
    assert_eq!(
        decode_turn_options(
            WardCodexTurnMode::Default,
            WardCodexPermissionPreset::ReadOnly,
        ),
        TurnOptions {
            mode: TurnMode::Default,
            permission_preset: TurnPermissionPreset::ReadOnly,
            inference: None,
        }
    );
}

#[tokio::test]
async fn queues_a_thread_rename_through_the_private_c_interface() {
    let (commands, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let observer = WardCodexHistoryObserver {
        commands,
        cancellation: CodexHistoryCancellation::new(),
        active_operation: Arc::new(ObserverOperationGate::new()),
        runtime: Handle::current(),
        worker: None,
    };
    let thread_id = CString::new("thread-1").expect("the thread ID is valid");
    let name = CString::new("Focused work").expect("the thread name is valid");
    let mut error = std::ptr::null_mut();

    // SAFETY: The observer and strings remain live for the duration of the
    // private C interface call, and the error output pointer is writable.
    assert!(unsafe {
        ward_core_codex_history_observer_rename_thread_async(
            std::ptr::from_ref(&observer).cast_mut(),
            thread_id.as_ptr(),
            name.as_ptr(),
            &mut error,
        )
    });
    assert!(error.is_null());

    let Some(ObserverCommand::RenameThread(request)) = receiver.recv().await else {
        panic!("the observer did not queue a thread rename");
    };
    assert_eq!(
        request,
        ThreadRenameRequest {
            thread_id: "thread-1".to_owned(),
            name: "Focused work".to_owned(),
        }
    );
}

#[tokio::test]
async fn queues_history_scope_and_lifecycle_changes_through_the_private_c_interface() {
    let (commands, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let observer = WardCodexHistoryObserver {
        commands,
        cancellation: CodexHistoryCancellation::new(),
        active_operation: Arc::new(ObserverOperationGate::new()),
        runtime: Handle::current(),
        worker: None,
    };
    let thread_id = CString::new("thread-1").expect("the thread ID is valid");
    let mut error = std::ptr::null_mut();

    // SAFETY: The observer and thread ID remain live for all private C
    // interface calls, and the error output pointer is writable.
    assert!(unsafe {
        ward_core_codex_history_observer_show_archived_async(
            std::ptr::from_ref(&observer).cast_mut(),
            true,
            &mut error,
        )
    });
    assert!(unsafe {
        ward_core_codex_history_observer_archive_thread_async(
            std::ptr::from_ref(&observer).cast_mut(),
            thread_id.as_ptr(),
            &mut error,
        )
    });
    assert!(unsafe {
        ward_core_codex_history_observer_restore_thread_async(
            std::ptr::from_ref(&observer).cast_mut(),
            thread_id.as_ptr(),
            &mut error,
        )
    });
    assert!(error.is_null());

    assert_eq!(
        receiver.recv().await,
        Some(ObserverCommand::SetThreadListScope(
            ThreadListScope::Archived
        ))
    );
    assert_eq!(
        receiver.recv().await,
        Some(ObserverCommand::ChangeThreadLifecycle(
            ThreadLifecycleRequest {
                thread_id: "thread-1".to_owned(),
                action: ThreadLifecycleAction::Archive,
            }
        ))
    );
    assert_eq!(
        receiver.recv().await,
        Some(ObserverCommand::ChangeThreadLifecycle(
            ThreadLifecycleRequest {
                thread_id: "thread-1".to_owned(),
                action: ThreadLifecycleAction::Restore,
            }
        ))
    );
}
