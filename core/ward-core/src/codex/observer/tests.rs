// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::CString;
use std::sync::Arc;

use tokio::runtime::Handle;
use tokio::sync::mpsc;
use ward_codex::{CodexHistoryCancellation, TurnMode, TurnOptions, TurnPermissionPreset};

use super::commands::{ObserverCommand, ThreadRenameRequest};
use super::{
    COMMAND_QUEUE_CAPACITY, ObserverOperation, ObserverOperationGate, WardCodexHistoryObserver,
    decode_turn_options, ward_core_codex_history_observer_rename_thread,
};

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
}

#[test]
fn decodes_the_private_turn_control_values() {
    assert_eq!(
        decode_turn_options(1, 1),
        Ok(TurnOptions {
            mode: TurnMode::Plan,
            permission_preset: TurnPermissionPreset::RequestApproval,
        })
    );
    assert_eq!(
        decode_turn_options(0, 2),
        Ok(TurnOptions {
            mode: TurnMode::Default,
            permission_preset: TurnPermissionPreset::ReadOnly,
        })
    );
    assert_eq!(
        decode_turn_options(7, 0),
        Err("the Codex turn mode is invalid")
    );
    assert_eq!(
        decode_turn_options(0, 7),
        Err("the Codex permission preset is invalid")
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
        ward_core_codex_history_observer_rename_thread(
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
