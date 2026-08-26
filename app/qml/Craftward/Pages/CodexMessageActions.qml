// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import Craftward.Components

Item {
    id: root

    property bool available: false
    property bool revealed: false
    property bool forkVisible: false
    property bool forkEnabled: false
    property int copyFeedbackDuration: 1600
    property url copyIconSource: "qrc:///icons/fluent/copy-20-regular.svg"
    property url copiedIconSource: "qrc:///icons/fluent/checkmark-20-regular.svg"
    property url forkIconSource: "qrc:///icons/fluent/arrow-split-20-regular.svg"
    readonly property bool copied: copyFeedbackTimer.running
    readonly property bool keyboardRevealed: copyButton.activeFocus || forkButton.activeFocus

    signal copyRequested
    signal forkRequested

    function confirmCopied() {
        copyFeedbackTimer.restart();
    }

    implicitWidth: actionRow.implicitWidth
    implicitHeight: available ? 24 : 0
    width: implicitWidth
    height: implicitHeight
    visible: available
    enabled: available
    opacity: revealed || copied || keyboardRevealed ? 1 : 0

    Behavior on opacity {
        NumberAnimation {
            duration: 80
            easing.type: Easing.OutCubic
        }
    }

    Row {
        id: actionRow

        spacing: 2

        IconButton {
            id: copyButton

            objectName: "codexMessageCopyButton"
            implicitWidth: 24
            implicitHeight: 24
            padding: 4
            icon.source: root.copied ? root.copiedIconSource : root.copyIconSource
            icon.width: 16
            icon.height: 16
            toolTipText: root.copied ? /*% "Copied" */ qsTrId("craftward.codex.timeline.copy.copied") : /*% "Copy" */ qsTrId("craftward.codex.timeline.copy.action")
            forceToolTipVisible: root.copied
            onClicked: root.copyRequested()
        }

        IconButton {
            id: forkButton

            objectName: "codexMessageForkButton"
            implicitWidth: visible ? 24 : 0
            implicitHeight: 24
            padding: 4
            icon.source: root.forkIconSource
            icon.width: 16
            icon.height: 16
            iconRotation: -90
            enabled: root.forkEnabled
            visible: root.forkVisible
            toolTipText: /*% "Fork from here" */ qsTrId("craftward.codex.timeline.fork.action")
            onClicked: root.forkRequested()
        }
    }

    Timer {
        id: copyFeedbackTimer

        interval: root.copyFeedbackDuration
    }
}
