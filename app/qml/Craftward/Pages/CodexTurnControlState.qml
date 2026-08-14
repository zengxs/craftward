// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    id: root

    property bool turnInFlight: false
    property bool writable: false
    property bool promptReady: false
    readonly property bool enabled: !turnInFlight && writable && promptReady
    readonly property string label: qsTr("Send")

    signal sendRequested

    function activate() {
        if (enabled)
            sendRequested();
    }
}
