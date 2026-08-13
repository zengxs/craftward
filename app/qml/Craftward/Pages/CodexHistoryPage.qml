// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Codex
import Craftward.Components

Page {
    id: root

    required property CodexHistoryController controller

    background: Rectangle {
        color: root.palette.window
    }

    WindowMoveHandler {
        targetWindow: root.ApplicationWindow.window
    }

    SplitView {
        anchors {
            fill: parent
            topMargin: root.SafeArea.margins.top + 20
            leftMargin: Math.max(24, root.SafeArea.margins.left)
            rightMargin: Math.max(24, root.SafeArea.margins.right)
            bottomMargin: Math.max(24, root.SafeArea.margins.bottom)
        }
        orientation: Qt.Horizontal

        Rectangle {
            SplitView.minimumWidth: 240
            SplitView.preferredWidth: 310
            SplitView.maximumWidth: 420
            radius: 12
            color: root.palette.base
            border.color: root.palette.mid

            ColumnLayout {
                anchors {
                    fill: parent
                    margins: 14
                }
                spacing: 10

                RowLayout {
                    Layout.fillWidth: true

                    Label {
                        Layout.fillWidth: true
                        text: qsTr("Codex History")
                        font.pixelSize: 20
                        font.weight: Font.DemiBold
                    }

                    BusyIndicator {
                        Layout.preferredWidth: 20
                        Layout.preferredHeight: 20
                        running: root.controller.loadingThreads
                        visible: running
                    }

                    Button {
                        text: qsTr("Refresh")
                        enabled: !root.controller.loadingThreads
                        onClicked: root.controller.refresh()
                    }
                }

                Label {
                    Layout.fillWidth: true
                    text: qsTr("Persisted conversations from the local Codex app-server.")
                    color: root.palette.placeholderText
                    wrapMode: Text.WordWrap
                }

                ListView {
                    id: threadList

                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 4
                    model: root.controller.threads
                    ScrollBar.vertical: OverlayScrollBar {}

                    delegate: ItemDelegate {
                        id: threadDelegate

                        required property string threadId
                        required property string title
                        required property string preview
                        required property string workingDirectory
                        required property date updatedAt

                        width: ListView.view.width
                        checkable: true
                        checked: root.controller.selectedThreadId === threadId
                        hoverEnabled: true
                        leftPadding: 12
                        rightPadding: 12
                        topPadding: 10
                        bottomPadding: 10
                        onClicked: root.controller.selectThread(threadId, title)

                        contentItem: ColumnLayout {
                            spacing: 3

                            Label {
                                Layout.fillWidth: true
                                text: threadDelegate.title || qsTr("Untitled conversation")
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                            }

                            Label {
                                Layout.fillWidth: true
                                text: threadDelegate.preview
                                color: root.palette.placeholderText
                                maximumLineCount: 2
                                elide: Text.ElideRight
                                wrapMode: Text.WordWrap
                            }

                            Label {
                                Layout.fillWidth: true
                                text: threadDelegate.workingDirectory
                                color: root.palette.placeholderText
                                font.pixelSize: 11
                                elide: Text.ElideMiddle
                            }
                        }
                    }

                    Label {
                        anchors.centerIn: parent
                        width: Math.min(parent.width - 32, 240)
                        text: root.controller.loadingThreads ? qsTr("Loading conversations…") : qsTr("No persisted conversations were found.")
                        color: root.palette.placeholderText
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        visible: threadList.count === 0
                    }
                }
            }
        }

        Item {
            SplitView.minimumWidth: 360
            SplitView.fillWidth: true

            ColumnLayout {
                anchors {
                    fill: parent
                    leftMargin: 22
                }
                spacing: 12

                RowLayout {
                    Layout.fillWidth: true

                    Label {
                        Layout.fillWidth: true
                        text: root.controller.selectedThreadTitle || qsTr("Conversation")
                        font.pixelSize: 24
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }

                    BusyIndicator {
                        Layout.preferredWidth: 22
                        Layout.preferredHeight: 22
                        running: root.controller.loadingConversation
                        visible: running
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: errorLayout.implicitHeight + 20
                    radius: 9
                    color: Qt.rgba(0.82, 0.12, 0.16, 0.08)
                    border.color: Qt.rgba(0.82, 0.12, 0.16, 0.24)
                    visible: root.controller.errorMessage.length > 0

                    RowLayout {
                        id: errorLayout

                        anchors {
                            fill: parent
                            margins: 10
                        }

                        Label {
                            Layout.fillWidth: true
                            text: root.controller.errorMessage
                            color: "#B4232A"
                            wrapMode: Text.WordWrap
                        }

                        IconButton {
                            icon.source: "qrc:///icons/phosphor/x-circle.svg"
                            toolTipText: qsTr("Dismiss error")
                            onClicked: root.controller.clearError()
                        }
                    }
                }

                ListView {
                    id: messageList

                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 14
                    model: root.controller.messages
                    // Temporary mitigation for ListView's variable-height content estimate.
                    // Replace this with cached row heights and anchored scrolling.
                    cacheBuffer: Math.max(height * 4, 2048)
                    ScrollBar.vertical: OverlayScrollBar {}

                    delegate: Item {
                        id: messageDelegate

                        required property bool fromUser
                        required property bool commentary
                        required property string text

                        width: ListView.view.width
                        implicitHeight: messageCard.implicitHeight

                        Rectangle {
                            id: messageCard

                            anchors.right: messageDelegate.fromUser ? parent.right : undefined
                            anchors.left: messageDelegate.fromUser ? undefined : parent.left
                            width: Math.min(implicitWidth, parent.width * 0.86)
                            implicitWidth: Math.max(220, messageContent.implicitWidth + 28)
                            implicitHeight: messageContent.implicitHeight + 24
                            radius: 12
                            color: messageDelegate.fromUser ? root.palette.alternateBase : root.palette.base
                            border.color: root.palette.mid
                            opacity: messageDelegate.commentary ? 0.72 : 1

                            ColumnLayout {
                                id: messageContent

                                anchors {
                                    fill: parent
                                    margins: 12
                                }
                                spacing: 6

                                Label {
                                    text: messageDelegate.fromUser ? qsTr("You") : (messageDelegate.commentary ? qsTr("Codex · Commentary") : qsTr("Codex"))
                                    color: root.palette.placeholderText
                                    font.pixelSize: 11
                                    font.weight: Font.DemiBold
                                }

                                TextEdit {
                                    Layout.fillWidth: true
                                    text: messageDelegate.text
                                    color: root.palette.text
                                    font: root.font
                                    readOnly: true
                                    selectByMouse: true
                                    wrapMode: TextEdit.Wrap
                                    textFormat: TextEdit.MarkdownText
                                }
                            }
                        }
                    }

                    Label {
                        anchors.centerIn: parent
                        width: Math.min(parent.width - 48, 360)
                        text: root.controller.loadingConversation ? qsTr("Loading conversation…") : (root.controller.selectedThreadId ? qsTr("This conversation contains no displayable messages.") : qsTr("Select a conversation to read it."))
                        color: root.palette.placeholderText
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        visible: messageList.count === 0
                    }
                }
            }
        }
    }
}
