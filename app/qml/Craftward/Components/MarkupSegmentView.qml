// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Window
import Craftward.Design
import Craftward.Highlighting
import Craftward.Markup
import Craftward.Components as Components

Control {
    id: root

    required property bool codeBlock
    required property string segmentText
    required property string language
    property color textColor: palette.text
    property font codeFont: font
    property var renderParts: null
    property var selectionCoordinator: null
    property var selectionHost: null
    readonly property var activeSelectionHost: selectionHost || localSelectionHost

    padding: 0
    implicitWidth: 0
    implicitHeight: segmentLoader.implicitHeight
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

    contentItem: Loader {
        id: segmentLoader

        sourceComponent: root.codeBlock ? codeSegment : (root.renderParts ? semanticProseSegment : proseSegment)
    }

    Component {
        id: semanticProseSegment

        Column {
            id: parts
            spacing: 8
            Repeater {
                model: root.renderParts || []
                delegate: Loader {
                    id: partLoader
                    required property var modelData
                    width: parts.width
                    sourceComponent: modelData.kind === "table" ? tablePart : textPart
                    Component {
                        id: textPart
                        MarkupSelectableText {
                            surface: partLoader.modelData.surface
                            coordinator: root.selectionCoordinator
                            selectionHost: root.activeSelectionHost
                            color: root.textColor
                            font: root.font
                            codeFont: root.codeFont
                            linkColor: root.palette.link
                        }
                    }
                    Component {
                        id: tablePart
                        Item {
                            implicitHeight: table.implicitHeight
                            MarkupTable {
                                id: table
                                x: partLoader.modelData.indent
                                width: Math.max(1, parent.width - x)
                                part: partLoader.modelData
                                coordinator: root.selectionCoordinator
                                selectionHost: root.activeSelectionHost
                                textColor: root.textColor
                                font: root.font
                                codeFont: root.codeFont
                                linkColor: root.palette.link
                            }
                        }
                    }
                }
            }
        }
    }

    Component {
        id: proseSegment

        TextEdit {
            objectName: "markupProseText"
            text: root.segmentText
            color: root.textColor
            font: root.font
            readOnly: true
            selectByMouse: true
            selectedTextColor: Theme.textSelectionForeground
            selectionColor: Theme.textSelectionBackground
            wrapMode: TextEdit.Wrap
            textFormat: TextEdit.PlainText
        }
    }

    Component {
        id: codeSegment

        Rectangle {
            id: codeSurface

            readonly property string displaySyntaxName: {
                const language = root.language.trim();
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
                    id: horizontalBar
                    policy: ScrollBar.AsNeeded
                }

                MarkupSelectableText {
                    id: codeText

                    objectName: "markupCodeText"
                    width: Math.max(codeFlick.width, implicitWidth)
                    surface: root.renderParts && root.renderParts.length ? root.renderParts[0].surface : null
                    coordinator: root.selectionCoordinator
                    selectionHost: root.activeSelectionHost
                    selectionExclusions: [codeToolbar, horizontalBar]
                    color: root.textColor
                    font: root.codeFont
                    readOnly: true
                    preserveSelectionColors: true
                    wrapMode: TextEdit.NoWrap
                    textFormat: TextEdit.PlainText

                    // The native adapter owns content once its surface arrives.
                    Binding {
                        target: codeText
                        property: "text"
                        when: !codeText.surface
                        value: root.segmentText
                        restoreMode: Binding.RestoreNone
                    }

                    SyntaxDocumentHighlighter {
                        id: syntaxHighlighter

                        textDocument: codeText.textDocument
                        language: root.language
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
                            if (Components.ApplicationClipboard.copyText(root.segmentText))
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
    MarkupSelectionHost {
        id: localSelectionHost
        anchors.fill: parent
        visible: !root.selectionHost
        z: 2
    }
}
