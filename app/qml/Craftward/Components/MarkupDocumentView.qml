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
    readonly property var renderModel: documentModel ? documentModel["renderModel"] : null

    padding: 0
    implicitWidth: 0
    implicitHeight: segmentColumn.implicitHeight
    background: null

    contentItem: Column {
        id: segmentColumn

        width: root.availableWidth
        spacing: 8

        Repeater {
            model: root.renderModel

            delegate: Loader {
                id: segmentLoader

                required property string segmentId
                required property bool codeBlock
                required property string segmentText
                required property string language
                required property bool markdown

                function synchronizeItem() {
                    if (!item)
                        return;
                    item.segmentText = segmentText;
                    item.segmentLanguage = language;
                    item.segmentMarkdown = markdown;
                }

                width: segmentColumn.width
                sourceComponent: codeBlock ? codeSegment : proseSegment
                onLoaded: synchronizeItem()
                onSegmentTextChanged: synchronizeItem()
                onLanguageChanged: synchronizeItem()
                onMarkdownChanged: synchronizeItem()
            }
        }
    }

    Component {
        id: proseSegment

        TextEdit {
            property string segmentText
            property string segmentLanguage
            property bool segmentMarkdown

            text: segmentText
            color: root.textColor
            font: root.font
            readOnly: true
            selectByMouse: true
            selectedTextColor: Theme.textSelectionForeground
            selectionColor: Theme.textSelectionBackground
            wrapMode: TextEdit.Wrap
            textFormat: segmentMarkdown ? TextEdit.MarkdownText : TextEdit.PlainText
        }
    }

    Component {
        id: codeSegment

        Rectangle {
            id: codeSurface

            property string segmentText
            property string segmentLanguage
            property bool segmentMarkdown

            implicitHeight: codeColumn.implicitHeight + 16
            radius: 8
            color: Theme.dark ? TailwindColors.zinc900 : TailwindColors.zinc100
            border.color: Theme.dark ? TailwindColors.zinc700 : TailwindColors.zinc300

            Column {
                id: codeColumn

                x: 10
                y: 8
                width: parent.width - 20
                spacing: 5

                Label {
                    text: codeSurface.segmentLanguage
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
                        text: codeSurface.segmentText
                        color: root.textColor
                        font: root.codeFont
                        readOnly: true
                        selectByMouse: true
                        selectedTextColor: Theme.textSelectionForeground
                        selectionColor: Theme.textSelectionBackground
                        wrapMode: TextEdit.NoWrap
                        textFormat: TextEdit.PlainText
                    }
                }
            }
        }
    }
}
