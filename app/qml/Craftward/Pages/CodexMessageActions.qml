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
    property url forkIconSource: "qrc:///icons/fluent/arrow-split-20-regular.svg"
    readonly property bool copied: copyButton.copied
    readonly property bool keyboardRevealed: copyButton.activeFocus || forkButton.activeFocus

    signal copyRequested
    signal forkRequested

    function confirmCopied() {
        copyButton.confirmCopied();
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

        CopyFeedbackButton {
            id: copyButton

            objectName: "codexMessageCopyButton"
            feedbackDuration: root.copyFeedbackDuration
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
}
