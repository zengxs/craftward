// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import Craftward.Codex
import Craftward.Components

Page {
    id: root

    required property CodexHistoryController controller
    readonly property CodexConversationController conversation: root.controller.conversation

    CodexHistoryActionState {
        id: historyActionState

        archived: root.controller.showingArchived
        hasSelection: root.conversation.threadId.length > 0
        forkReady: (root.conversation.turnState === CodexConversationController.Detached || root.conversation.turnState === CodexConversationController.Idle) && (root.conversation.writeAvailability === CodexConversationController.NotRequested || root.conversation.writeAvailability === CodexConversationController.Writable)
        loadingThreads: root.controller.loadingThreads
        loadingConversation: root.conversation.loading
        startingThread: root.controller.startingThread
        forkingThread: root.controller.forkingThread
        turnInFlight: root.conversation.turnInFlight
        changingThreadLifecycle: root.controller.changingThreadLifecycle
    }

    FolderDialog {
        id: workingDirectoryDialog

        title: qsTr("Choose a working directory for the new conversation")
        onAccepted: root.controller.startThread(selectedFolder)
    }

    CodexConversationRenameDialog {
        id: renameDialog

        currentName: root.conversation.title
        renameAllowed: historyActionState.renameAllowed
        onRenameRequested: name => {
            if (root.controller.renameSelectedThread(name))
                accept();
        }
    }

    ConfirmationDialog {
        id: archiveDialog

        title: qsTr("Archive conversation?")
        message: qsTr("This conversation will move out of Active history. You can restore it later from Archived.")
        acceptText: qsTr("Archive")
        onAccepted: root.controller.archiveSelectedThread()
    }

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
                        running: root.controller.loadingThreads || root.controller.startingThread || root.controller.forkingThread
                        visible: running
                    }

                    Button {
                        text: qsTr("New…")
                        visible: !root.controller.showingArchived
                        enabled: historyActionState.canStartThread
                        onClicked: workingDirectoryDialog.open()
                    }

                    Button {
                        text: qsTr("Refresh")
                        enabled: !historyActionState.busy
                        onClicked: root.controller.refresh()
                    }
                }

                Label {
                    Layout.fillWidth: true
                    text: qsTr("Continue persisted conversations through the local Codex app-server.")
                    color: root.palette.placeholderText
                    wrapMode: Text.WordWrap
                }

                ButtonGroup {
                    id: historyScopeGroup
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 6

                    Button {
                        Layout.fillWidth: true
                        text: qsTr("Active")
                        checkable: true
                        checked: !root.controller.showingArchived
                        enabled: historyActionState.canSwitchScope
                        ButtonGroup.group: historyScopeGroup
                        onClicked: root.controller.showArchivedThreads(false)
                    }

                    Button {
                        Layout.fillWidth: true
                        text: qsTr("Archived")
                        checkable: true
                        checked: root.controller.showingArchived
                        enabled: historyActionState.canSwitchScope
                        ButtonGroup.group: historyScopeGroup
                        onClicked: root.controller.showArchivedThreads(true)
                    }
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
                        checked: root.conversation.threadId === threadId
                        enabled: !historyActionState.busy
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
                        text: root.controller.startingThread ? qsTr("Starting a new conversation…") : (root.controller.loadingThreads ? qsTr("Loading conversations…") : (root.controller.showingArchived ? qsTr("No archived conversations were found.") : qsTr("No active conversations were found.")))
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
                        text: root.conversation.title || qsTr("Conversation")
                        font.pixelSize: 24
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }

                    Button {
                        text: qsTr("Rename…")
                        visible: root.conversation.threadId.length > 0 && !root.controller.showingArchived
                        enabled: renameDialog.renameAllowed
                        onClicked: renameDialog.begin()
                    }

                    Button {
                        text: root.controller.showingArchived ? qsTr("Restore") : qsTr("Archive…")
                        visible: root.conversation.threadId.length > 0
                        enabled: root.controller.showingArchived ? historyActionState.canRestore : historyActionState.canArchive
                        onClicked: {
                            if (root.controller.showingArchived)
                                root.controller.restoreSelectedThread();
                            else
                                archiveDialog.open();
                        }
                    }

                    Rectangle {
                        implicitWidth: runtimeStateLayout.implicitWidth + 16
                        implicitHeight: runtimeStateLayout.implicitHeight + 8
                        radius: height / 2
                        color: root.conversation.turnState === CodexConversationController.SystemError ? Qt.rgba(0.82, 0.12, 0.16, 0.08) : root.palette.alternateBase
                        border.color: root.conversation.turnState === CodexConversationController.SystemError ? Qt.rgba(0.82, 0.12, 0.16, 0.24) : root.palette.mid
                        visible: root.conversation.threadId.length > 0

                        RowLayout {
                            id: runtimeStateLayout

                            anchors.centerIn: parent
                            spacing: 6

                            Rectangle {
                                Layout.preferredWidth: 7
                                Layout.preferredHeight: 7
                                radius: width / 2
                                color: {
                                    if (root.conversation.turnState === CodexConversationController.SystemError)
                                        return "#B4232A";
                                    if (root.conversation.turnState === CodexConversationController.Running || root.conversation.turnState === CodexConversationController.Starting)
                                        return root.palette.highlight;
                                    return root.palette.mid;
                                }
                            }

                            Label {
                                text: {
                                    if (root.controller.showingArchived)
                                        return qsTr("Archived · Read only");
                                    if (root.conversation.turnState === CodexConversationController.Starting)
                                        return qsTr("Starting…");
                                    if (root.conversation.turnState === CodexConversationController.Running) {
                                        if (root.conversation.waitingOnApproval)
                                            return qsTr("Waiting for approval");
                                        if (root.conversation.waitingOnUserInput)
                                            return qsTr("Waiting for input");
                                        return qsTr("Running");
                                    }
                                    if (root.conversation.turnState === CodexConversationController.Idle)
                                        return qsTr("Live · Idle");
                                    if (root.conversation.turnState === CodexConversationController.SystemError)
                                        return qsTr("Runtime error");
                                    if (root.conversation.turnState === CodexConversationController.Unknown)
                                        return qsTr("Status unknown");
                                    return qsTr("History only");
                                }
                                color: root.conversation.turnState === CodexConversationController.SystemError ? "#B4232A" : root.palette.placeholderText
                                font.pixelSize: 11
                            }
                        }
                    }

                    BusyIndicator {
                        Layout.preferredWidth: 22
                        Layout.preferredHeight: 22
                        running: root.conversation.loading || root.controller.startingThread || root.controller.forkingThread || root.conversation.turnInFlight || root.controller.changingThreadLifecycle
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
                    model: root.conversation.interactions
                    enabled: !root.controller.startingThread
                    visible: count > 0 && !root.controller.showingArchived
                    ScrollBar.vertical: OverlayScrollBar {}

                    delegate: CodexInteractionCard {
                        id: interactionCard

                        width: ListView.view.width
                        onApprovalSubmitted: decision => root.conversation.respondToApproval(interactionId, decision)
                        onUserInputSubmitted: answers => root.conversation.respondToUserInput(interactionId, answers)
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
                    visible: root.conversation.threadId.length > 0 && root.conversation.activityHistoryPartial
                }

                CodexTimelineView {
                    id: timelineView

                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    controller: root.conversation
                    forkEnabled: historyActionState.canFork
                    showForkActions: root.conversation.threadId.length > 0 && !root.controller.showingArchived
                    onForkRequested: turnId => root.controller.forkSelectedThread(turnId)
                }

                CodexComposer {
                    Layout.fillWidth: true
                    controller: root.conversation
                    readOnly: root.controller.showingArchived
                    startingThread: root.controller.startingThread
                    enabled: !root.controller.startingThread && !root.controller.forkingThread
                    visible: historyActionState.composerVisible
                    onTurnSubmitted: timelineView.followLatest()
                }
            }
        }
    }
}
