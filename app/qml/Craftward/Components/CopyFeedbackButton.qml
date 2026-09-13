// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

IconButton {
    id: control

    property int feedbackDuration: 1600
    readonly property bool copied: feedbackTimer.running

    function confirmCopied() {
        feedbackTimer.restart();
    }

    implicitWidth: 24
    implicitHeight: 24
    padding: 4
    icon.source: copied ? "qrc:///icons/hugeicons/checkmark-square-04.svg" : "qrc:///icons/hugeicons/copy-01.svg"
    icon.width: 16
    icon.height: 16
    toolTipText: copied ? /*% "Copied" */ qsTrId("craftward.components.copy.copied") : /*% "Copy" */ qsTrId("craftward.components.copy.action")
    forceToolTipVisible: copied

    Timer {
        id: feedbackTimer

        interval: control.feedbackDuration
    }
}
