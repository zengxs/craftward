// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import Craftward.Design
import Craftward.Components as Components

Control {
    id: root

    required property var part
    property var coordinator: null
    property var selectionHost: null
    property font codeFont: Components.Typography.codeFont
    property color textColor: palette.text
    property color linkColor: palette.link
    readonly property bool hasPriority: part.priority >= 0
    readonly property color priorityColor: part.priority <= 1 ? Theme.dangerForeground : part.priority === 2 ? (Theme.dark ? TailwindColors.amber300 : TailwindColors.amber800) : palette.placeholderText
    readonly property real fontScale: fontInfo.pixelSize / 14

    signal fileLocationRequested(string file, int start, int end)

    objectName: "markupCodeComment"
    padding: 18
    implicitHeight: contentColumn.implicitHeight + topPadding + bottomPadding
    implicitWidth: 0
    background: Rectangle {
        radius: 10
        color: root.palette.base
        border.color: Theme.metadataBadgeRing
    }

    FontInfo {
        id: fontInfo
        font: root.font
    }

    Rectangle {
        id: badge
        objectName: "markupCommentBadge"
        visible: root.hasPriority
        x: root.leftPadding
        y: contentColumn.y + title.y + title.firstLineBaseline - priorityText.y - priorityText.firstLineBaseline
        width: Math.max(28, priorityText.contentWidth + 12)
        height: Math.max(22, priorityText.implicitHeight + 4)
        radius: 5
        color: Qt.rgba(root.priorityColor.r, root.priorityColor.g, root.priorityColor.b, Theme.dark ? 0.18 : 0.10)
        MarkupSelectableText {
            id: priorityText
            objectName: "markupCommentPriority"
            x: 6
            y: 2
            width: parent.width - 12
            surface: root.part.badge ?? null
            coordinator: root.coordinator
            selectionHost: root.selectionHost
            wrapMode: TextEdit.NoWrap
            font: Qt.font({
                family: root.font.family,
                pixelSize: Math.round(11 * root.fontScale),
                weight: Font.DemiBold
            })
            lineHeightScale: 1.4
            color: root.priorityColor
        }
    }

    contentItem: Column {
        id: contentColumn
        spacing: 7
        MarkupSelectableText {
            id: title
            objectName: "markupCommentTitle"
            x: root.hasPriority ? badge.width + 8 : 0
            width: parent.width - x
            surface: root.part.title
            coordinator: root.coordinator
            selectionHost: root.selectionHost
            font: Qt.font({
                family: root.font.family,
                pixelSize: Math.round(15 * root.fontScale),
                weight: Font.Bold
            })
            lineHeightScale: 1.5
            color: root.textColor
        }
        MarkupSelectableText {
            id: location
            objectName: "markupCommentLocation"
            width: parent.width
            surface: root.part.location
            coordinator: root.coordinator
            selectionHost: root.selectionHost
            font: Qt.font({
                family: root.codeFont.family,
                pixelSize: Math.round(11 * root.fontScale)
            })
            color: root.textColor
            linkColor: root.linkColor
            linkHandler: () => root.fileLocationRequested(root.part.file, root.part.start, root.part.end)
            HoverHandler {
                id: locationHover
                blocking: false
            }
            ToolTip.visible: locationHover.hovered
            ToolTip.delay: 550
            ToolTip.text: root.part.file + (root.part.start ? ":" + root.part.start + (root.part.end !== root.part.start ? "–" + root.part.end : "") : "")
        }
        Item {
            width: 1
            height: 1
        }
        MarkupPartsView {
            objectName: "markupCommentBody"
            width: parent.width
            renderParts: root.part.body
            selectionCoordinator: root.coordinator
            selectionHost: root.selectionHost
            font: Qt.font({
                family: root.font.family,
                pixelSize: Math.round(13 * root.fontScale),
                weight: root.font.weight
            })
            codeFont: root.codeFont
            textColor: root.textColor
            linkColor: root.linkColor
        }
    }
}
