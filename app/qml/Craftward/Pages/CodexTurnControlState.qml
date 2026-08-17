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
    readonly property bool inputEnabled: !steerPending && !interruptPending && (!turnInFlight || turnRunning)
    readonly property bool sendEnabled: inputEnabled && writable && promptReady
    readonly property string sendLabel: steerPending ? qsTr("Guiding…") : (turnRunning ? qsTr("Guide") : (turnInFlight ? qsTr("Starting…") : qsTr("Send")))
    readonly property bool stopVisible: turnInFlight
    readonly property bool stopEnabled: turnInFlight && !interruptPending
    readonly property string stopLabel: interruptPending ? qsTr("Stopping…") : qsTr("Stop")

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
