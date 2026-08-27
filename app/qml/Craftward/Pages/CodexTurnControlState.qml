// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    id: root

    property bool turnInFlight: false
    property bool turnRunning: false
    property bool steerPending: false
    property bool interruptPending: false
    property bool writable: false
    property bool continuationRequestable: false
    property bool promptReady: false
    property bool attachmentReady: false
    property bool continuationAvailable: false
    readonly property QtObject actionDescriptor: CodexComposerAction {}
    readonly property bool inputEnabled: !steerPending && !interruptPending && (!turnInFlight || turnRunning)
    readonly property bool attachmentInputEnabled: !turnInFlight || turnRunning
    readonly property bool contentReady: promptReady || attachmentReady
    readonly property bool sendEnabled: inputEnabled && writable && contentReady
    readonly property string sendLabel: steerPending ? /*% "Guiding…" */ qsTrId("craftward.codex.turn.guiding") : (turnRunning ? /*% "Guide" */ qsTrId("craftward.codex.turn.guide") : (turnInFlight ? /*% "Starting…" */ qsTrId("craftward.codex.turn.starting") : actionDescriptor.label(CodexComposerAction.SendAction)))
    readonly property bool stopEnabled: turnInFlight && !interruptPending
    readonly property string stopLabel: interruptPending ? /*% "Stopping…" */ qsTrId("craftward.codex.turn.stopping") : actionDescriptor.label(CodexComposerAction.StopAction)
    readonly property string continueLabel: actionDescriptor.label(CodexComposerAction.ContinueAction)
    readonly property int primaryAction: {
        if (turnInFlight)
            return !turnRunning || !contentReady || steerPending || interruptPending ? CodexComposerAction.StopAction : CodexComposerAction.SendAction;
        if (continuationAvailable && !contentReady)
            return CodexComposerAction.ContinueAction;
        return CodexComposerAction.SendAction;
    }
    readonly property bool primaryEnabled: {
        switch (primaryAction) {
        case CodexComposerAction.StopAction:
            return stopEnabled;
        case CodexComposerAction.ContinueAction:
            return inputEnabled && continuationRequestable && continuationAvailable;
        default:
            return sendEnabled;
        }
    }
    readonly property string primaryToolTip: {
        switch (primaryAction) {
        case CodexComposerAction.StopAction:
            return stopLabel;
        case CodexComposerAction.ContinueAction:
            return continueLabel;
        default:
            return sendLabel;
        }
    }

    signal sendRequested
    signal stopRequested
    signal continueRequested

    function send() {
        if (sendEnabled)
            sendRequested();
    }

    function stop() {
        if (stopEnabled)
            stopRequested();
    }

    function continueTurn() {
        if (primaryAction === CodexComposerAction.ContinueAction && primaryEnabled)
            continueRequested();
    }

    function activatePrimaryAction() {
        if (primaryAction === CodexComposerAction.SendAction)
            send();
        else if (primaryAction === CodexComposerAction.StopAction)
            stop();
        else
            continueTurn();
    }
}
