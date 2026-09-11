// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import Craftward.Markup

// Shared ordinary Markdown surfaces for a segment or an embedded review body.
Control {
    id: root

    required property var renderParts
    property var selectionCoordinator: null
    property var selectionHost: null
    property font codeFont: font
    property color textColor: palette.text
    property color linkColor: palette.link
    readonly property real listIndentWidth: MarkupListMetrics.indentWidth(renderParts, font)

    padding: 0
    implicitWidth: 0
    implicitHeight: parts.implicitHeight
    background: null

    contentItem: Column {
        id: parts
        Repeater {
            id: partRepeater
            model: root.renderParts || []
            delegate: Item {
                id: partItem
                required property var modelData
                required property int index
                width: parts.width
                implicitHeight: partLoader.implicitHeight + (index + 1 < partRepeater.count ? (modelData.spacingAfter ?? 8) : 0)
                Loader {
                    id: partLoader
                    width: parent.width
                    sourceComponent: partItem.modelData.kind === "table" ? tablePart : textPart
                    Component {
                        id: textPart
                        MarkupSelectableText {
                            surface: partItem.modelData.surface
                            coordinator: root.selectionCoordinator
                            selectionHost: root.selectionHost
                            color: root.textColor
                            font: root.font
                            codeFont: root.codeFont
                            listIndentWidth: root.listIndentWidth
                            linkColor: root.linkColor
                        }
                    }
                    Component {
                        id: tablePart
                        Item {
                            implicitHeight: table.implicitHeight
                            MarkupTable {
                                id: table
                                x: partItem.modelData.quoteIndent + partItem.modelData.listDepth * root.listIndentWidth
                                width: Math.max(1, parent.width - x)
                                part: partItem.modelData
                                coordinator: root.selectionCoordinator
                                selectionHost: root.selectionHost
                                textColor: root.textColor
                                font: root.font
                                codeFont: root.codeFont
                                linkColor: root.linkColor
                            }
                        }
                    }
                }
            }
        }
    }
}
