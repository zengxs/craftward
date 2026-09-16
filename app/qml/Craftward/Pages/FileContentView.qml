// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Editor

Pane {
    id: root
    required property var file
    readonly property var details: file || ({
            text: "",
            error: "",
            path: ""
        })
    signal openExternallyRequested(string path)

    padding: 0
    background: Rectangle {
        color: root.palette.base
    }
    onDetailsChanged: if (editor)
        editor.reveal()

    CodeEditor {
        id: editor
        anchors.fill: parent
        padding: 8
        visible: root.details.error.length === 0
        readOnly: false
        showLineNumbers: true
        text: root.details.text
        filePath: root.details.path
        function reveal() {
            Qt.callLater(() => revealLocation(root.details.startLine || 0, root.details.endLine || 0));
        }
        Component.onCompleted: reveal()
    }

    ColumnLayout {
        anchors.centerIn: parent
        width: Math.max(0, Math.min(parent.width - 40, 420))
        visible: root.details.error.length > 0
        spacing: 12
        Label {
            Layout.fillWidth: true
            text: root.details.error
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            color: root.palette.placeholderText
        }
        Button {
            Layout.alignment: Qt.AlignHCenter
            text: /*% "Open in Default Application" */ qsTrId("craftward.file.open_external")
            onClicked: root.openExternallyRequested(root.details.path)
        }
    }
}
