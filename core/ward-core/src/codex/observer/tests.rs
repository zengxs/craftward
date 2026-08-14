// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use ward_codex::{TurnMode, TurnOptions, TurnPermissionPreset};

use super::decode_turn_options;

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
