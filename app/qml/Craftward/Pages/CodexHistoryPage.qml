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
    property alias sidebarExpanded: layoutState.sidebarExpanded
    readonly property bool historyPollingEnabled: root.visible && root.ApplicationWindow.window !== null && root.ApplicationWindow.window.visible && root.ApplicationWindow.window.visibility !== Window.Minimized
    readonly property bool fullScreen: root.ApplicationWindow.window !== null && root.ApplicationWindow.window.visibility === Window.FullScreen
    readonly property bool trafficLightsVisible: Qt.platform.os === "osx" && !root.fullScreen
    readonly property int titleBarMotionDuration: 160
    readonly property real titleBarHeight: Math.max(28, root.SafeArea.margins.top)
    readonly property real titleBarLeadingInset: root.trafficLightsVisible ? Math.max(78, root.SafeArea.margins.left) : Math.max(0, root.SafeArea.margins.left)
    property real animatedTitleBarLeadingInset: root.titleBarLeadingInset
    readonly property bool titleBarLeadingInsetAnimating: Math.abs(root.animatedTitleBarLeadingInset - root.titleBarLeadingInset) > 0.01
    readonly property string runtimeStatusText: {
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
    readonly property color runtimeIndicatorColor: {
        if (root.conversation.turnState === CodexConversationController.SystemError)
            return Theme.dangerForeground;
        if (root.conversation.turnState === CodexConversationController.Running || root.conversation.turnState === CodexConversationController.Starting)
            return root.palette.highlight;
        return root.palette.mid;
    }

    Behavior on animatedTitleBarLeadingInset {
        NumberAnimation {
            duration: root.titleBarMotionDuration
            easing.type: Easing.OutCubic
        }
    }

    Binding {
        target: root.controller
        property: "pollingEnabled"
        value: root.historyPollingEnabled
    }

    CodexHistoryLayoutState {
        id: layoutState

        titleBarLeadingInset: root.animatedTitleBarLeadingInset
    }

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

    SplitView {
        id: contentSplit

        anchors {
            top: parent.top
            left: parent.left
            right: parent.right
            bottom: statusBar.top
        }
        orientation: Qt.Horizontal
        handle: Rectangle {
            implicitWidth: 1
            color: root.palette.windowText
            opacity: 0.14
        }

        Rectangle {
            id: sidebarPane

            objectName: "codexSidebar"
            SplitView.minimumWidth: layoutState.sidebarExpanded ? layoutState.minimumSidebarWidth : 0
            SplitView.preferredWidth: layoutState.bodySidebarWidth
            SplitView.maximumWidth: layoutState.sidebarExpanded ? layoutState.maximumSidebarWidth : 0
            visible: layoutState.sidebarExpanded
            color: Theme.sidebarSurface
            onWidthChanged: if (visible)
                layoutState.rememberSidebarWidth(width)

            ColumnLayout {
                anchors {
                    fill: parent
                    topMargin: root.titleBarHeight + 10
                    leftMargin: 14
                    rightMargin: 14
                    bottomMargin: 14
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
            id: mainPane

            SplitView.minimumWidth: 360
            SplitView.fillWidth: true

            ColumnLayout {
                anchors {
                    fill: parent
                    topMargin: root.titleBarHeight + 14
                    leftMargin: 22
                    rightMargin: Math.max(22, root.SafeArea.margins.right)
                    bottomMargin: 14
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
                                color: root.runtimeIndicatorColor
                            }

                            Label {
                                text: root.runtimeStatusText
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
                            icon.source: "qrc:///icons/fluent/dismiss-circle-20-regular.svg"
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

    Item {
        id: titleBar

        objectName: "codexTitleBar"
        anchors {
            top: parent.top
            left: parent.left
            right: parent.right
        }
        height: root.titleBarHeight
        z: 10

        WindowMoveHandler {
            targetWindow: root.ApplicationWindow.window
        }

        Rectangle {
            id: navigationChrome

            anchors {
                top: parent.top
                bottom: parent.bottom
                left: parent.left
            }
            width: Math.min(parent.width, layoutState.navigationChromeWidth)
            color: layoutState.sidebarExpanded ? Theme.sidebarSurface : root.palette.window

            Behavior on width {
                enabled: !root.titleBarLeadingInsetAnimating && !contentSplit.resizing

                NumberAnimation {
                    duration: root.titleBarMotionDuration
                    easing.type: Easing.OutCubic
                }
            }
        }

        Rectangle {
            id: titleBarSidebarDivider

            x: navigationChrome.width
            anchors {
                top: parent.top
                bottom: parent.bottom
            }
            width: 1
            color: root.palette.windowText
            opacity: layoutState.sidebarExpanded ? 0.14 : 0
            z: 2
        }

        IconButton {
            id: sidebarToggle

            objectName: "codexSidebarToggle"
            x: layoutState.sidebarExpanded ? navigationChrome.width - width : layoutState.collapsedSidebarToggleX
            anchors {
                verticalCenter: parent.verticalCenter
            }
            activeFocusOnTab: true
            padding: 6
            backgroundInset: 2
            icon.source: "qrc:///icons/fluent/panel-left-tall-20-regular-emphasized.svg"
            icon.width: 16
            icon.height: 16
            toolTipText: layoutState.sidebarExpanded ? /*% "Hide Sidebar" */ qsTrId("craftward.navigation.sidebar.hide") : /*% "Show Sidebar" */ qsTrId("craftward.navigation.sidebar.show")
            onClicked: layoutState.toggleSidebar()

            Behavior on x {
                enabled: !root.titleBarLeadingInsetAnimating && !contentSplit.resizing

                NumberAnimation {
                    duration: root.titleBarMotionDuration
                    easing.type: Easing.OutCubic
                }
            }
        }

        Row {
            id: historyActions

            x: layoutState.leadingActionsX
            anchors.verticalCenter: parent.verticalCenter
            spacing: 0

            Behavior on x {
                enabled: !root.titleBarLeadingInsetAnimating

                NumberAnimation {
                    duration: root.titleBarMotionDuration
                    easing.type: Easing.OutCubic
                }
            }

            IconButton {
                id: newConversationButton

                objectName: "codexNewConversationButton"
                activeFocusOnTab: true
                padding: 6
                backgroundInset: 2
                icon.source: "qrc:///icons/fluent/chat-add-20-regular-emphasized.svg"
                icon.width: 16
                icon.height: 16
                toolTipText: /*% "New…" */ qsTrId("craftward.codex.history.new.action")
                visible: !root.controller.showingArchived
                enabled: historyActionState.canStartThread
                onClicked: workingDirectoryDialog.open()
            }

            IconButton {
                id: refreshButton

                objectName: "codexRefreshButton"
                activeFocusOnTab: true
                padding: 6
                backgroundInset: 2
                icon.source: "qrc:///icons/fluent/arrow-sync-20-regular-emphasized.svg"
                icon.width: 16
                icon.height: 16
                toolTipText: /*% "Refresh" */ qsTrId("craftward.action.refresh")
                enabled: !historyActionState.busy
                onClicked: root.controller.refresh()
            }
        }

        Item {
            id: tabStrip

            objectName: "codexTabStrip"
            anchors {
                top: parent.top
                bottom: parent.bottom
                left: navigationChrome.right
                right: parent.right
                rightMargin: Math.max(0, root.SafeArea.margins.right)
            }
            clip: true
            z: 1

            Rectangle {
                id: conversationTab

                anchors {
                    top: parent.top
                    bottom: parent.bottom
                    left: parent.left
                }
                width: Math.min(240, Math.max(112, conversationTabLabel.implicitWidth + 24))
                color: Theme.navigationSelectionBackground

                Label {
                    id: conversationTabLabel

                    anchors {
                        fill: parent
                        leftMargin: 12
                        rightMargin: 12
                    }
                    text: /*% "Conversation" */ qsTrId("craftward.codex.history.conversation.title")
                    font.pixelSize: 13
                    font.weight: Font.DemiBold
                    verticalAlignment: Text.AlignVCenter
                    elide: Text.ElideRight
                }

                Rectangle {
                    anchors {
                        left: parent.left
                        right: parent.right
                        bottom: parent.bottom
                    }
                    height: 2
                    color: Theme.navigationSelectionForeground
                    z: 1
                }
            }
        }

        Rectangle {
            anchors {
                left: layoutState.sidebarExpanded ? navigationChrome.right : parent.left
                right: parent.right
                bottom: parent.bottom
            }
            height: 1
            color: root.palette.windowText
            opacity: 0.14
        }
    }

    Rectangle {
        id: statusBar

        objectName: "codexStatusBar"
        anchors {
            left: parent.left
            right: parent.right
            bottom: parent.bottom
        }
        height: 24 + Math.max(0, root.SafeArea.margins.bottom)
        color: root.palette.window
        z: 9

        Rectangle {
            anchors {
                top: parent.top
                left: parent.left
                right: parent.right
            }
            height: 1
            color: root.palette.windowText
            opacity: 0.14
        }

        RowLayout {
            anchors {
                top: parent.top
                left: parent.left
                right: parent.right
                leftMargin: 10
                rightMargin: 10
            }
            height: 24
            spacing: 7

            Label {
                text: /*% "Codex" */ qsTrId("craftward.codex.name")
                color: root.palette.placeholderText
                font.pixelSize: 11
            }

            Item {
                Layout.fillWidth: true
            }

            Rectangle {
                Layout.preferredWidth: 6
                Layout.preferredHeight: 6
                radius: width / 2
                color: root.runtimeIndicatorColor
                visible: root.conversation.threadId.length > 0
            }

            Label {
                text: root.runtimeStatusText
                color: root.conversation.turnState === CodexConversationController.SystemError ? Theme.dangerForeground : root.palette.placeholderText
                font.pixelSize: 11
                visible: root.conversation.threadId.length > 0
            }
        }
    }
}
