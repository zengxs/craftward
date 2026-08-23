// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import Craftward.Design

Control {
    id: root

    required property var documentModel
    property color textColor: palette.text
    property font codeFont: font

    padding: 0
    implicitWidth: Math.max(220, blockColumn.implicitWidth)
    implicitHeight: blockColumn.implicitHeight
    background: null

    contentItem: Column {
        id: blockColumn

        width: root.availableWidth
        spacing: 8

        Repeater {
            model: root.documentModel

            delegate: Item {
                id: blockDelegate

                required property string blockId
                required property bool codeBlock
                required property string blockText
                required property string plainText
                required property string language
                required property bool markdown

                width: blockColumn.width
                implicitWidth: codeBlock ? 560 : proseText.implicitWidth
                implicitHeight: codeBlock ? codeSurface.implicitHeight : proseText.implicitHeight

                TextEdit {
                    id: proseText

                    width: parent.width
                    text: blockDelegate.blockText
                    color: root.textColor
                    font: root.font
                    readOnly: true
                    selectByMouse: true
                    wrapMode: TextEdit.Wrap
                    textFormat: blockDelegate.markdown ? TextEdit.MarkdownText : TextEdit.PlainText
                    visible: !blockDelegate.codeBlock
                }

                Rectangle {
                    id: codeSurface

                    width: parent.width
                    implicitWidth: 560
                    implicitHeight: codeColumn.implicitHeight + 16
                    radius: 8
                    color: Theme.dark ? TailwindColors.zinc900 : TailwindColors.zinc100
                    border.color: Theme.dark ? TailwindColors.zinc700 : TailwindColors.zinc300
                    visible: blockDelegate.codeBlock

                    Column {
                        id: codeColumn

                        x: 10
                        y: 8
                        width: parent.width - 20
                        spacing: 5

                        Label {
                            text: blockDelegate.language
                            color: root.palette.placeholderText
                            font.pixelSize: 10
                            font.weight: Font.DemiBold
                            visible: text.length > 0
                        }

                        Flickable {
                            id: codeFlick

                            width: parent.width
                            height: codeText.implicitHeight
                            contentWidth: Math.max(width, codeText.implicitWidth)
                            contentHeight: height
                            boundsBehavior: Flickable.StopAtBounds
                            flickableDirection: Flickable.HorizontalFlick
                            interactive: contentWidth > width
                            clip: true

                            ScrollBar.horizontal: ScrollBar {
                                policy: ScrollBar.AsNeeded
                            }

                            TextEdit {
                                id: codeText

                                width: Math.max(codeFlick.width, implicitWidth)
                                text: blockDelegate.blockText
                                color: root.textColor
                                font: root.codeFont
                                readOnly: true
                                selectByMouse: true
                                wrapMode: TextEdit.NoWrap
                                textFormat: TextEdit.PlainText
                            }
                        }
                    }
                }
            }
        }
    }
}
