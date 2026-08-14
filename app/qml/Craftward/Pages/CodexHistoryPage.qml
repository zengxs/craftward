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
                        text: qsTr("Codex")
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
                        enabled: !root.controller.loadingThreads && !root.controller.turnInFlight
                        onClicked: root.controller.refresh()
                    }
                }

                Label {
                    Layout.fillWidth: true
                    text: qsTr("Continue persisted conversations through the local Codex app-server.")
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
                        enabled: !root.controller.turnInFlight
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

                    Rectangle {
                        implicitWidth: runtimeStateLayout.implicitWidth + 16
                        implicitHeight: runtimeStateLayout.implicitHeight + 8
                        radius: height / 2
                        color: root.controller.turnState === CodexHistoryController.SystemError ? Qt.rgba(0.82, 0.12, 0.16, 0.08) : root.palette.alternateBase
                        border.color: root.controller.turnState === CodexHistoryController.SystemError ? Qt.rgba(0.82, 0.12, 0.16, 0.24) : root.palette.mid
                        visible: root.controller.selectedThreadId.length > 0

                        RowLayout {
                            id: runtimeStateLayout

                            anchors.centerIn: parent
                            spacing: 6

                            Rectangle {
                                Layout.preferredWidth: 7
                                Layout.preferredHeight: 7
                                radius: width / 2
                                color: {
                                    if (root.controller.turnState === CodexHistoryController.SystemError)
                                        return "#B4232A";
                                    if (root.controller.turnState === CodexHistoryController.Running || root.controller.turnState === CodexHistoryController.Starting)
                                        return root.palette.highlight;
                                    return root.palette.mid;
                                }
                            }

                            Label {
                                text: {
                                    if (root.controller.turnState === CodexHistoryController.Starting)
                                        return qsTr("Starting…");
                                    if (root.controller.turnState === CodexHistoryController.Running) {
                                        if (root.controller.waitingOnApproval)
                                            return qsTr("Waiting for approval");
                                        if (root.controller.waitingOnUserInput)
                                            return qsTr("Waiting for input");
                                        return qsTr("Running");
                                    }
                                    if (root.controller.turnState === CodexHistoryController.Idle)
                                        return qsTr("Live · Idle");
                                    if (root.controller.turnState === CodexHistoryController.SystemError)
                                        return qsTr("Runtime error");
                                    if (root.controller.turnState === CodexHistoryController.Unknown)
                                        return qsTr("Status unknown");
                                    return qsTr("History only");
                                }
                                color: root.controller.turnState === CodexHistoryController.SystemError ? "#B4232A" : root.palette.placeholderText
                                font.pixelSize: 11
                            }
                        }
                    }

                    BusyIndicator {
                        Layout.preferredWidth: 22
                        Layout.preferredHeight: 22
                        running: root.controller.loadingConversation || root.controller.turnInFlight
                        visible: running
                    }
                }

                ListView {
                    id: interactionList

                    Layout.fillWidth: true
                    Layout.preferredHeight: Math.min(contentHeight, 360)
                    Layout.maximumHeight: 360
                    clip: true
                    spacing: 8
                    model: root.controller.interactions
                    visible: count > 0
                    ScrollBar.vertical: OverlayScrollBar {}

                    delegate: CodexInteractionCard {
                        id: interactionCard

                        width: ListView.view.width
                        onApprovalSubmitted: decision => root.controller.respondToApproval(interactionId, decision)
                        onUserInputSubmitted: answers => root.controller.respondToUserInput(interactionId, answers)
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

                Label {
                    Layout.fillWidth: true
                    text: qsTr("Some runtime activity may be unavailable in persisted history.")
                    color: root.palette.placeholderText
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                    visible: root.controller.selectedThreadId.length > 0 && root.controller.activityHistoryPartial
                }

                CodexTimelineView {
                    id: timelineView

                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    controller: root.controller
                }

                CodexComposer {
                    Layout.fillWidth: true
                    controller: root.controller
                    onTurnSubmitted: timelineView.followLatest()
                }
            }
        }
    }
}
