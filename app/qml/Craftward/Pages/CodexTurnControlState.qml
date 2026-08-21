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
    property bool promptReady: false
    property bool attachmentReady: false
    readonly property bool inputEnabled: !steerPending && !interruptPending && (!turnInFlight || turnRunning)
    readonly property bool attachmentInputEnabled: !turnInFlight || turnRunning
    readonly property bool contentReady: promptReady || (!turnRunning && attachmentReady)
    readonly property bool sendEnabled: inputEnabled && writable && contentReady
    readonly property string sendLabel: steerPending ? /*% "Guiding…" */ qsTrId("craftward.codex.turn.guiding") : (turnRunning ? /*% "Guide" */ qsTrId("craftward.codex.turn.guide") : (turnInFlight ? /*% "Starting…" */ qsTrId("craftward.codex.turn.starting") : /*% "Send" */ qsTrId("craftward.codex.turn.send")))
    readonly property bool stopVisible: turnInFlight
    readonly property bool stopEnabled: turnInFlight && !interruptPending
    readonly property string stopLabel: interruptPending ? /*% "Stopping…" */ qsTrId("craftward.codex.turn.stopping") : /*% "Stop" */ qsTrId("craftward.codex.turn.stop")

    signal sendRequested
    signal stopRequested

    function send() {
        if (sendEnabled)
            sendRequested();
    }

    function stop() {
        if (stopEnabled)
            stopRequested();
    }
}
