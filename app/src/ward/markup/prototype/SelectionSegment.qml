// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Column {
    id: root
    required property var segment
    required property var coordinator
    required property var host
    required property int pixelSize
    spacing: 8

    Label {
        text: root.segment.title
        color: root.segment.message === "message-a" ? "#657080" : "#946035"
        font.pixelSize: 11
        font.bold: true
    }
    Rectangle {
        width: root.width
        height: rows.height + 20
        radius: 5
        color: root.segment.kind === "code" ? "#edf0f5" : "white"
        border.color: root.segment.message === "message-a" ? "#dce1e9" : "#dfb78e"
        Column {
            id: rows
            x: 10
            y: 10
            width: parent.width - 20
            spacing: root.segment.kind === "table" ? 1 : 0
            Repeater {
                model: Math.ceil(root.segment.parts.length / root.segment.columns)
                delegate: RowLayout {
                    id: row
                    required property int index
                    width: rows.width
                    spacing: root.segment.kind === "table" ? 1 : 0
                    Repeater {
                        model: root.segment.parts.slice(row.index * root.segment.columns, (row.index + 1) * root.segment.columns)
                        delegate: Rectangle {
                            required property var modelData
                            Layout.fillWidth: true
                            Layout.fillHeight: true
                            Layout.preferredWidth: 1
                            implicitHeight: text.implicitHeight + (root.segment.kind === "table" ? 14 : 0)
                            color: root.segment.kind === "table" ? (row.index === 0 ? "#e9edf5" : "#f6f8fb") : "transparent"
                            SelectionText {
                                id: text
                                x: root.segment.kind === "table" ? 7 : 0
                                y: root.segment.kind === "table" ? 7 : 0
                                width: parent.width - 2 * x
                                height: parent.height - 2 * y
                                blockId: modelData.id
                                nodes: modelData.nodes
                                pixelSize: root.pixelSize
                                coordinator: root.coordinator
                                host: root.host
                            }
                        }
                    }
                }
            }
        }
    }
}
