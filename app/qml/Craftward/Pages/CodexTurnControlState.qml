// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    id: root

    property bool turnInFlight: false
    property bool interruptPending: false
    property bool writable: false
    property bool promptReady: false
    readonly property bool enabled: turnInFlight ? !interruptPending : writable && promptReady
    readonly property string label: turnInFlight ? (interruptPending ? qsTr("Stopping…") : qsTr("Stop")) : qsTr("Send")

    signal sendRequested
    signal stopRequested

    function activate() {
        if (!enabled)
            return;
        if (turnInFlight)
            stopRequested();
        else
            sendRequested();
    }
}
