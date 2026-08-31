// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Window
import Craftward.Design
import Craftward.Highlighting
import Craftward.Components as Components

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

    function titleCaseLanguage(language) {
        const words = language.trim().replace(/[_-]+/g, " ").split(/\s+/);
        for (let index = 0; index < words.length; ++index) {
            const word = words[index];
            if (word.length > 0)
                words[index] = word.charAt(0).toUpperCase() + word.slice(1).toLowerCase();
        }
        return words.join(" ");
    }

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
            readonly property string displaySyntaxName: {
                const language = segmentLanguage.trim();
                if (language.length === 0)
                    return "";
                if (syntaxHighlighter.syntaxName.length === 0)
                    return "";
                if (syntaxHighlighter.languageRecognized)
                    return syntaxHighlighter.syntaxName;
                return root.titleCaseLanguage(language);
            }
            readonly property bool actionsVisible: codeHover.hovered || codeText.activeFocus || copyButton.activeFocus || copyButton.copied

            objectName: "markupCodeSurface"
            implicitHeight: codeFlick.height + 16
            radius: 8
            color: Theme.dark ? TailwindColors.zinc900 : TailwindColors.zinc50
            border.width: 1 / Math.max(1, Screen.devicePixelRatio)
            border.color: Qt.rgba(root.textColor.r, root.textColor.g, root.textColor.b, Theme.dark ? 0.22 : 0.16)

            Flickable {
                id: codeFlick

                x: 10
                y: 8
                width: parent.width - 20
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

                    objectName: "markupCodeText"
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

                    SyntaxDocumentHighlighter {
                        id: syntaxHighlighter

                        textDocument: codeText.textDocument
                        language: codeSurface.segmentLanguage
                        darkTheme: Theme.dark
                    }
                }
            }

            HoverHandler {
                id: codeHover
            }

            Item {
                id: codeToolbar

                objectName: "markupCodeToolbar"
                z: 1
                anchors.top: parent.top
                anchors.right: parent.right
                anchors.topMargin: 4
                anchors.rightMargin: 6
                implicitWidth: toolbarRow.implicitWidth
                implicitHeight: toolbarRow.implicitHeight
                width: implicitWidth
                height: implicitHeight
                visible: codeSurface.displaySyntaxName.length > 0 || codeSurface.actionsVisible
                opacity: codeSurface.actionsVisible ? 1 : 0.48

                Behavior on opacity {
                    NumberAnimation {
                        duration: 80
                        easing.type: Easing.OutCubic
                    }
                }

                Row {
                    id: toolbarRow

                    spacing: 2

                    Components.CopyFeedbackButton {
                        id: copyButton

                        objectName: "markupCodeCopyButton"
                        visible: codeSurface.actionsVisible
                        onClicked: {
                            if (Components.ApplicationClipboard.copyText(codeText.text))
                                confirmCopied();
                        }
                    }

                    Label {
                        objectName: "markupCodeSyntaxLabel"
                        height: 24
                        leftPadding: 4
                        rightPadding: 4
                        text: codeSurface.displaySyntaxName
                        color: root.palette.placeholderText
                        font.pixelSize: 10
                        font.weight: Font.DemiBold
                        verticalAlignment: Text.AlignVCenter
                        visible: text.length > 0
                    }
                }
            }
        }
    }
}
