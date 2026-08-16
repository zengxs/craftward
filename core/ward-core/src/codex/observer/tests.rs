// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use ward_codex::{TurnMode, TurnOptions, TurnPermissionPreset};

use super::{ObserverOperation, ObserverOperationGate, decode_turn_options};

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
