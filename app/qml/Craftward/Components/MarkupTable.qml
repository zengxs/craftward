// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import Craftward.Design

Column {
    id: root

    required property var part
    required property font font
    required property font codeFont
    required property color textColor
    required property color linkColor
    property var coordinator: null
    property var selectionHost: null
    readonly property color lineColor: Qt.rgba(textColor.r, textColor.g, textColor.b, Theme.dark ? 0.25 : 0.22)
    readonly property int columns: Math.max(1, part.columns)

    objectName: "markupTable"

    Repeater {
        model: root.part.rows
        delegate: Item {
            id: tableRow
            required property var modelData
            width: root.width
            implicitHeight: {
                let tallest = 0;
                for (let i = 0; i < cells.count; ++i) {
                    const cell = cells.itemAt(i);
                    if (cell)
                        tallest = Math.max(tallest, cell.textHeight);
                }
                return tallest + 14;
            }
            height: implicitHeight

            Repeater {
                id: cells
                model: tableRow.modelData.cells
                delegate: Rectangle {
                    id: cell
                    required property int index
                    required property var modelData
                    readonly property real textHeight: cellText.implicitHeight
                    x: index * tableRow.width / root.columns
                    width: tableRow.width / root.columns
                    height: tableRow.height
                    color: tableRow.modelData.header ? (Theme.dark ? TailwindColors.zinc800 : TailwindColors.zinc100) : "transparent"
                    border.width: 0.5
                    border.color: root.lineColor

                    MarkupSelectableText {
                        id: cellText
                        x: 7
                        y: 7
                        width: Math.max(1, cell.width - 14)
                        surface: cell.modelData
                        coordinator: root.coordinator
                        selectionHost: root.selectionHost
                        font: root.font
                        codeFont: root.codeFont
                        color: root.textColor
                        linkColor: root.linkColor
                    }
                }
            }
        }
    }
}
