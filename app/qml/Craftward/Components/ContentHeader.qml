// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Pane {
    id: root

    property string locationText
    property string statusText
    property color statusColor: palette.placeholderText
    default property alias actions: actionRow.data

    implicitHeight: 36
    padding: 0

    background: Rectangle {
        color: root.palette.window
        Rectangle {
            anchors {
                left: parent.left
                right: parent.right
                bottom: parent.bottom
            }
            height: 1
            color: root.palette.windowText
            opacity: 0.1
        }
    }

    contentItem: RowLayout {
        spacing: 8

        Label {
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            Layout.leftMargin: 12
            text: root.locationText
            font.pixelSize: 12
            color: root.palette.placeholderText
            elide: Text.ElideMiddle
            ToolTip.visible: locationHover.hovered && truncated
            ToolTip.delay: 500
            ToolTip.text: text
            HoverHandler {
                id: locationHover
            }
        }

        Label {
            text: root.statusText
            visible: text.length > 0
            font.pixelSize: 11
            color: root.statusColor
        }

        Row {
            id: actionRow
            Layout.rightMargin: 4
            spacing: 0
        }
    }
}
