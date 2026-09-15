// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Codex
import Craftward.Components

ListView {
    id: root
    required property var threads
    property string searchText
    property string selectedThreadId
    property bool busy
    property string emptyText
    signal threadRequested(string threadId, string title)
    clip: true
    model: CodexThreadFilterModel {
        sourceModel: root.threads
        searchText: root.searchText
    }
    onSearchTextChanged: Qt.callLater(() => root.positionViewAtBeginning())
    ScrollBar.vertical: OverlayScrollBar {}
    delegate: ItemDelegate {
        id: threadDelegate
        required property string threadId
        required property string title
        required property string preview
        required property string workingDirectory
        width: ListView.view.width
        checkable: true
        checked: root.selectedThreadId === threadId
        enabled: !root.busy
        hoverEnabled: true
        padding: 10
        onClicked: root.threadRequested(threadId, title)
        contentItem: ColumnLayout {
            spacing: 4
            Label {
                Layout.fillWidth: true
                text: threadDelegate.title || /*% "Untitled conversation" */ qsTrId("craftward.codex.history.untitled")
                font.pixelSize: 13
                font.weight: Font.DemiBold
                elide: Text.ElideRight
            }
            Label {
                Layout.fillWidth: true
                text: threadDelegate.preview
                font.pixelSize: 12
                color: root.palette.placeholderText
                maximumLineCount: 2
                elide: Text.ElideRight
                wrapMode: Text.WordWrap
            }
            Label {
                Layout.fillWidth: true
                text: threadDelegate.workingDirectory
                font.pixelSize: 11
                color: root.palette.placeholderText
                elide: Text.ElideMiddle
            }
        }
    }
    Label {
        anchors.centerIn: parent
        width: Math.max(0, parent.width - 24)
        text: root.emptyText
        color: root.palette.placeholderText
        wrapMode: Text.WordWrap
        horizontalAlignment: Text.AlignHCenter
        visible: root.count === 0
    }
}
