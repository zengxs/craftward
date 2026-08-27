// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    enum Kind {
        SendAction,
        StopAction,
        ContinueAction
    }

    function iconSource(action) {
        switch (action) {
        case CodexComposerAction.StopAction:
            return "qrc:///icons/fluent/stop-20-filled.svg";
        case CodexComposerAction.ContinueAction:
            return "qrc:///icons/fluent/play-20-filled.svg";
        default:
            return "qrc:///icons/fluent/arrow-up-20-filled.svg";
        }
    }

    function label(action) {
        switch (action) {
        case CodexComposerAction.StopAction:
            return /*% "Stop" */ qsTrId("craftward.codex.turn.stop");
        case CodexComposerAction.ContinueAction:
            return /*% "Continue" */ qsTrId("craftward.codex.turn.continue");
        default:
            return /*% "Send" */ qsTrId("craftward.codex.turn.send");
        }
    }
}
