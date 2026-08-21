// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import Craftward.Codex
import Craftward.Components
import Craftward.Design

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

        title: /*% "Choose a working directory for the new conversation" */ qsTrId("craftward.codex.history.new.working_directory_dialog.title")
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

        title: /*% "Archive conversation?" */ qsTrId("craftward.codex.history.archive_confirmation.title")
        message: /*% "This conversation will move out of Active history. You can restore it later from Archived." */ qsTrId("craftward.codex.history.archive_confirmation.message")
        acceptText: /*% "Archive" */ qsTrId("craftward.action.archive")
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
                        text: /*% "Codex" */ qsTrId("craftward.codex.name")
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
                        text: /*% "New…" */ qsTrId("craftward.codex.history.new.action")
                        visible: !root.controller.showingArchived
                        enabled: historyActionState.canStartThread
                        onClicked: workingDirectoryDialog.open()
                    }

                    Button {
                        text: /*% "Refresh" */ qsTrId("craftward.action.refresh")
                        enabled: !historyActionState.busy
                        onClicked: root.controller.refresh()
                    }
                }

                Label {
                    Layout.fillWidth: true
                    text: /*% "Continue persisted conversations through the local Codex app-server." */ qsTrId("craftward.codex.history.description")
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
                        text: /*% "Active" */ qsTrId("craftward.codex.history.scope.active")
                        checkable: true
                        checked: !root.controller.showingArchived
                        enabled: historyActionState.canSwitchScope
                        ButtonGroup.group: historyScopeGroup
                        onClicked: root.controller.showArchivedThreads(false)
                    }

                    Button {
                        Layout.fillWidth: true
                        text: /*% "Archived" */ qsTrId("craftward.codex.history.scope.archived")
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
                                text: threadDelegate.title || /*% "Untitled conversation" */ qsTrId("craftward.codex.history.untitled")
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
                        text: root.controller.startingThread ? /*% "Starting a new conversation…" */ qsTrId("craftward.codex.history.new.starting") : (root.controller.loadingThreads ? /*% "Loading conversations…" */ qsTrId("craftward.codex.history.loading") : (root.controller.showingArchived ? /*% "No archived conversations were found." */ qsTrId("craftward.codex.history.empty.archived") : /*% "No active conversations were found." */ qsTrId("craftward.codex.history.empty.active")))
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
                        text: root.conversation.title || /*% "Conversation" */ qsTrId("craftward.codex.history.conversation.title")
                        font.pixelSize: 24
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }

                    Button {
                        text: /*% "Rename…" */ qsTrId("craftward.action.rename_ellipsis")
                        visible: root.conversation.threadId.length > 0 && !root.controller.showingArchived
                        enabled: renameDialog.renameAllowed
                        onClicked: renameDialog.begin()
                    }

                    Button {
                        text: root.controller.showingArchived ? /*% "Restore" */ qsTrId("craftward.action.restore") : /*% "Archive…" */ qsTrId("craftward.action.archive_ellipsis")
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
                        color: root.conversation.turnState === CodexConversationController.SystemError ? Theme.dangerSurface : root.palette.alternateBase
                        border.color: root.conversation.turnState === CodexConversationController.SystemError ? Theme.dangerBorder : root.palette.mid
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
                                        return Theme.dangerForeground;
                                    if (root.conversation.turnState === CodexConversationController.Running || root.conversation.turnState === CodexConversationController.Starting)
                                        return root.palette.highlight;
                                    return root.palette.mid;
                                }
                            }

                            Label {
                                text: {
                                    if (root.controller.showingArchived)
                                        return /*% "Archived · Read only" */ qsTrId("craftward.codex.runtime.archived_read_only");
                                    if (root.conversation.turnState === CodexConversationController.Starting)
                                        return /*% "Starting…" */ qsTrId("craftward.codex.runtime.starting");
                                    if (root.conversation.turnState === CodexConversationController.Running) {
                                        if (root.conversation.waitingOnApproval)
                                            return /*% "Waiting for approval" */ qsTrId("craftward.codex.runtime.waiting_for_approval");
                                        if (root.conversation.waitingOnUserInput)
                                            return /*% "Waiting for input" */ qsTrId("craftward.codex.runtime.waiting_for_input");
                                        return /*% "Running" */ qsTrId("craftward.codex.runtime.running");
                                    }
                                    if (root.conversation.turnState === CodexConversationController.Idle)
                                        return /*% "Live · Idle" */ qsTrId("craftward.codex.runtime.live_idle");
                                    if (root.conversation.turnState === CodexConversationController.SystemError)
                                        return /*% "Runtime error" */ qsTrId("craftward.codex.runtime.error");
                                    if (root.conversation.turnState === CodexConversationController.Unknown)
                                        return /*% "Status unknown" */ qsTrId("craftward.codex.runtime.unknown");
                                    return /*% "History only" */ qsTrId("craftward.codex.runtime.history_only");
                                }
                                color: root.conversation.turnState === CodexConversationController.SystemError ? Theme.dangerForeground : root.palette.placeholderText
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
                    color: Theme.dangerSurface
                    border.color: Theme.dangerBorder
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
                            color: Theme.dangerForeground
                            wrapMode: Text.WordWrap
                        }

                        IconButton {
                            icon.source: "qrc:///icons/phosphor/x-circle.svg"
                            toolTipText: /*% "Dismiss error" */ qsTrId("craftward.error.dismiss")
                            onClicked: root.controller.clearError()
                        }
                    }
                }

                Label {
                    Layout.fillWidth: true
                    text: /*% "Some runtime activity may be unavailable in persisted history." */ qsTrId("craftward.codex.history.runtime_activity_notice")
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
