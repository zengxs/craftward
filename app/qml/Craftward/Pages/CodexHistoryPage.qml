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
import Craftward.Terminal

Page {
    id: root

    required property CodexHistoryController controller
    property TerminalController terminalController: null
    signal fileLocationRequested(string file, int start, int end)
    property bool timelineMotionDiagnosticsEnabled: false
    property bool timelineRenderBenchmarkEnabled: false
    property string timelineRenderBenchmarkThreadId: ""
    readonly property CodexConversationController conversation: controller.conversation
    readonly property string timelineMotionDiagnosticsText: conversationView.timelineMotionDiagnosticsText
    property alias sidebarExpanded: layoutState.sidebarExpanded
    property alias filesExpanded: workspaceState.filesExpanded
    readonly property bool historyPollingEnabled: visible && ApplicationWindow.window !== null && ApplicationWindow.window.visible && ApplicationWindow.window.visibility !== Window.Minimized
    readonly property bool fullScreen: ApplicationWindow.window !== null && ApplicationWindow.window.visibility === Window.FullScreen
    readonly property bool trafficLightsVisible: Qt.platform.os === "osx" && !fullScreen
    readonly property real titleBarHeight: Math.max(28, SafeArea.margins.top)
    readonly property real titleBarLeadingInset: trafficLightsVisible ? Math.max(78, SafeArea.margins.left) : Math.max(12, SafeArea.margins.left)

    function openFile(file, start = 0, end = 0) {
        const tab = ApplicationFiles.readTextFile(file, controller.workingDirectory);
        tab.startLine = start;
        tab.endLine = end;
        workspaceState.openTab(tab);
    }

    function chooseFile() {
        fileDialog.open();
    }
    function closeActiveTab() {
        workspaceState.closeTab(workspaceState.activeIndex);
    }
    function toggleSidebar() {
        layoutState.toggleSidebar();
    }
    function toggleFiles() {
        if (!layoutState.filesVisible) {
            workspaceState.filesExpanded = true;
            if (!layoutState.filesVisible)
                layoutState.sidebarExpanded = false;
        } else {
            workspaceState.filesExpanded = false;
        }
    }

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

    Binding {
        target: root.controller
        property: "pollingEnabled"
        value: root.historyPollingEnabled
    }

    ThreadWorkspaceState {
        id: workspaceState
    }
    Connections {
        target: root.conversation
        function onSelectionChanged() {
            workspaceState.selectThread(root.conversation.threadId);
        }
    }
    Component.onCompleted: workspaceState.selectThread(root.conversation.threadId)

    CodexHistoryLayoutState {
        id: layoutState
        availableWidth: root.width
        filesExpanded: workspaceState.filesExpanded
    }

    FileDialog {
        id: fileDialog
        title: /*% "Open File…" */ qsTrId("craftward.file.open")
        fileMode: FileDialog.OpenFile
        onAccepted: root.openFile(selectedFile.toString())
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

    Rectangle {
        id: titleBar
        objectName: "codexTitleBar"
        anchors {
            top: parent.top
            left: parent.left
            right: parent.right
        }
        height: root.titleBarHeight
        color: TailwindColors.zinc200
        WindowMoveHandler {
            targetWindow: root.ApplicationWindow.window
        }
        Label {
            anchors {
                fill: parent
                leftMargin: root.titleBarLeadingInset + 12
                rightMargin: root.titleBarLeadingInset + 12
            }
            text: root.conversation.title || /*% "Craftward" */ qsTrId("craftward.app.name")
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            font.pixelSize: 13
            font.weight: Font.DemiBold
            color: TailwindColors.zinc800
            elide: Text.ElideRight
        }
        Rectangle {
            anchors {
                left: parent.left
                right: parent.right
                bottom: parent.bottom
            }
            height: 1
            color: root.palette.windowText
            opacity: 0.12
        }
    }

    SplitView {
        id: contentSplit
        anchors {
            top: titleBar.bottom
            left: parent.left
            right: parent.right
            bottom: statusBar.top
        }
        orientation: Qt.Horizontal
        handle: Rectangle {
            id: columnDivider
            implicitWidth: 1
            color: root.palette.windowText
            opacity: SplitHandle.pressed ? 0.4 : 0.12
            containmentMask: Item {
                x: (columnDivider.width - width) / 2
                width: 8
                height: columnDivider.height
            }
        }
        CollapsibleSplitPane {
            id: sidebarPane
            objectName: "codexSidebar"
            minimumExpandedWidth: layoutState.minimumSidebarWidth
            expandedWidth: layoutState.sidebarWidth
            maximumExpandedWidth: layoutState.maximumSidebarWidth
            expanded: layoutState.sidebarExpanded
            resizing: contentSplit.resizing
            onResized: width => layoutState.rememberSidebarWidth(width)

            Rectangle {
                anchors.fill: parent
                color: Theme.sidebarSurface
            }

            ColumnLayout {
                anchors {
                    top: parent.top
                    bottom: parent.bottom
                    right: parent.right
                }
                width: Math.max(sidebarPane.width, sidebarPane.minimumExpandedWidth)
                spacing: 0

                RowLayout {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 32
                    Layout.leftMargin: 12
                    Layout.rightMargin: 4
                    spacing: 0
                    Label {
                        Layout.fillWidth: true
                        text: /*% "Conversations" */ qsTrId("craftward.navigation.conversations")
                        font.pixelSize: 12
                        font.weight: Font.DemiBold
                    }
                    IconButton {
                        objectName: "codexNewConversationButton"
                        icon.source: "qrc:///icons/hugeicons/chat-add.svg"
                        toolTipText: /*% "New…" */ qsTrId("craftward.codex.history.new.action")
                        visible: !root.controller.showingArchived
                        enabled: historyActionState.canStartThread
                        onClicked: workingDirectoryDialog.open()
                    }
                    IconButton {
                        icon.source: "qrc:///icons/hugeicons/refresh-01.svg"
                        toolTipText: /*% "Refresh" */ qsTrId("craftward.action.refresh")
                        enabled: !historyActionState.busy
                        onClicked: root.controller.refresh()
                    }
                }

                TextField {
                    id: conversationSearch
                    Layout.fillWidth: true
                    Layout.margins: 8
                    Layout.preferredHeight: 28
                    placeholderText: /*% "Search conversations" */ qsTrId("craftward.navigation.search")
                    font.pixelSize: 12
                    selectByMouse: true
                }

                RowLayout {
                    Layout.fillWidth: true
                    Layout.leftMargin: 8
                    Layout.rightMargin: 8
                    Layout.bottomMargin: 8
                    spacing: 6
                    ButtonGroup {
                        id: scopeGroup
                    }
                    Button {
                        Layout.fillWidth: true
                        text: /*% "Active" */ qsTrId("craftward.codex.history.scope.active")
                        checkable: true
                        checked: !root.controller.showingArchived
                        enabled: historyActionState.canSwitchScope
                        ButtonGroup.group: scopeGroup
                        onClicked: root.controller.showArchivedThreads(false)
                    }
                    Button {
                        Layout.fillWidth: true
                        text: /*% "Archived" */ qsTrId("craftward.codex.history.scope.archived")
                        checkable: true
                        checked: root.controller.showingArchived
                        enabled: historyActionState.canSwitchScope
                        ButtonGroup.group: scopeGroup
                        onClicked: root.controller.showArchivedThreads(true)
                    }
                }

                CodexThreadList {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    Layout.leftMargin: 8
                    Layout.rightMargin: 8
                    Layout.bottomMargin: 8
                    threads: root.controller.threads
                    searchText: conversationSearch.text
                    selectedThreadId: root.conversation.threadId
                    busy: historyActionState.busy
                    emptyText: {
                        if (root.controller.loadingThreads)
                            return /*% "Loading conversations…" */ qsTrId("craftward.codex.history.loading");
                        if (conversationSearch.text.length > 0)
                            return /*% "No matching conversations were found." */ qsTrId("craftward.codex.history.empty.search");
                        return root.controller.showingArchived ? /*% "No archived conversations were found." */ qsTrId("craftward.codex.history.empty.archived") : /*% "No active conversations were found." */ qsTrId("craftward.codex.history.empty.active");
                    }
                    onThreadRequested: (threadId, title) => root.controller.selectThread(threadId, title)
                }
            }
        }

        Item {
            id: mainPane
            SplitView.minimumWidth: layoutState.minimumContentWidth
            SplitView.fillWidth: true

            WorkbenchContentMenu {
                id: contentMenu
                fileActive: workspaceState.activeTab !== null
                hasConversation: root.conversation.threadId.length > 0
                archived: root.controller.showingArchived
                renameAllowed: renameDialog.renameAllowed
                archiveAllowed: historyActionState.canArchive
                restoreAllowed: historyActionState.canRestore
                onOpenFileRequested: root.chooseFile()
                onOpenExternalRequested: root.fileLocationRequested(workspaceState.activeTab.path, 0, 0)
                onRefreshRequested: root.openFile(workspaceState.activeTab.path)
                onRenameRequested: renameDialog.begin()
                onArchiveRequested: archiveDialog.open()
                onRestoreRequested: root.controller.restoreSelectedThread()
            }

            WorkbenchTabs {
                id: tabStrip
                objectName: "codexTabStrip"
                anchors {
                    top: parent.top
                    left: parent.left
                    right: parent.right
                }
                height: layoutState.tabBarHeight
                workspace: workspaceState
                onConversationActionsRequested: anchor => contentMenu.popup(anchor, 0, anchor.height)
            }

            ContentHeader {
                id: contentHeader
                objectName: "workbenchContentHeader"
                anchors {
                    top: tabStrip.bottom
                    left: parent.left
                    right: parent.right
                }
                height: layoutState.contentHeaderHeight
                locationText: workspaceState.activeTab ? workspaceState.activeTab.location : (root.conversation.title || /*% "Untitled conversation" */ qsTrId("craftward.codex.history.untitled"))
                statusText: workspaceState.activeTab ? (workspaceState.activeTab.external ? /*% "External · Read only" */ qsTrId("craftward.file.external_read_only") : /*% "Read only" */ qsTrId("craftward.file.read_only")) : ""

                IconButton {
                    icon.source: "qrc:///icons/hugeicons/folder-02.svg"
                    toolTipText: /*% "Show in File List" */ qsTrId("craftward.file.reveal")
                    visible: workspaceState.activeTab !== null && !workspaceState.activeTab.external
                    onClicked: {
                        workspaceState.filesExpanded = true;
                        if (!layoutState.filesVisible)
                            layoutState.sidebarExpanded = false;
                        filesPane.reveal(workspaceState.activeTab.path);
                    }
                }
                IconButton {
                    id: contentMenuButton
                    icon.source: "qrc:///icons/hugeicons/more-horizontal-circle-02.svg"
                    toolTipText: /*% "More Actions" */ qsTrId("craftward.actions.more")
                    visible: workspaceState.activeTab !== null
                    onClicked: contentMenu.popup(contentMenuButton, 0, contentMenuButton.height)
                }
            }

            SplitView {
                id: conversationSplit
                anchors {
                    top: contentHeader.bottom
                    left: parent.left
                    right: parent.right
                    bottom: parent.bottom
                }
                orientation: Qt.Vertical
                handle: Rectangle {
                    id: terminalDivider
                    implicitHeight: 1
                    color: root.palette.windowText
                    opacity: SplitHandle.pressed ? 0.4 : 0.12
                    containmentMask: Item {
                        y: (terminalDivider.height - height) / 2
                        width: terminalDivider.width
                        height: 8
                    }
                }
                onResizingChanged: {
                    if (!resizing && root.terminalController && terminalPanel.visible)
                        root.terminalController.panelHeight = Math.round(terminalPanel.height);
                }
                Item {
                    SplitView.minimumHeight: 160
                    SplitView.fillHeight: true
                    CodexConversationView {
                        id: conversationView
                        anchors.fill: parent
                        visible: workspaceState.activeIndex === 0
                        controller: root.controller
                        actionState: historyActionState
                        timelineMotionDiagnosticsEnabled: root.timelineMotionDiagnosticsEnabled
                        timelineRenderBenchmarkEnabled: root.timelineRenderBenchmarkEnabled
                        timelineRenderBenchmarkThreadId: root.timelineRenderBenchmarkThreadId
                        onFileLocationRequested: (file, start, end) => root.openFile(file, start, end)
                    }
                    Loader {
                        anchors.fill: parent
                        active: workspaceState.activeTab !== null
                        visible: active
                        sourceComponent: FileContentView {
                            file: workspaceState.activeTab
                            onOpenExternallyRequested: path => root.fileLocationRequested(path, 0, 0)
                        }
                    }
                }
                Loader {
                    id: terminalPanel
                    active: visible
                    visible: root.terminalController ? root.terminalController.panelVisible : false
                    SplitView.minimumHeight: 120
                    SplitView.preferredHeight: root.terminalController ? root.terminalController.panelHeight : 240
                    sourceComponent: TerminalPanel {
                        controller: root.terminalController
                    }
                }
            }
        }

        CollapsibleSplitPane {
            id: filesSidebar
            objectName: "projectFilesSidebar"
            minimumExpandedWidth: layoutState.minimumFilesWidth
            expandedWidth: layoutState.filesWidth
            maximumExpandedWidth: layoutState.maximumFilesWidth
            expanded: layoutState.filesVisible
            resizing: contentSplit.resizing
            onResized: width => layoutState.rememberFilesWidth(width)

            ProjectFilesPane {
                id: filesPane
                objectName: "projectFilesPane"
                anchors {
                    top: parent.top
                    bottom: parent.bottom
                    left: parent.left
                }
                width: Math.max(filesSidebar.width, filesSidebar.minimumExpandedWidth)
                directory: root.controller.workingDirectory
                selectedPath: workspaceState.activeTab ? workspaceState.activeTab.path : ""
                onFileRequested: path => root.openFile(path)
                onOpenFileRequested: root.chooseFile()
            }
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
        height: layoutState.statusBarHeight + Math.max(0, root.SafeArea.margins.bottom)
        color: root.palette.window
        Rectangle {
            anchors {
                left: parent.left
                right: parent.right
                top: parent.top
            }
            height: 1
            color: root.palette.windowText
            opacity: 0.12
        }
        RowLayout {
            anchors {
                top: parent.top
                left: parent.left
                right: parent.right
                leftMargin: 4
                rightMargin: 4
            }
            height: layoutState.statusBarHeight
            spacing: 6
            PanelActionButton {
                objectName: "codexSidebarToggle"
                icon.source: "qrc:///icons/hugeicons/sidebar-left.svg"
                toolTipText: layoutState.sidebarExpanded ? /*% "Hide Sidebar" */ qsTrId("craftward.navigation.sidebar.hide") : /*% "Show Sidebar" */ qsTrId("craftward.navigation.sidebar.show")
                onClicked: root.toggleSidebar()
            }
            Label {
                text: /*% "Local" */ qsTrId("craftward.execution.local")
                font.pixelSize: 11
                color: root.palette.placeholderText
            }
            Item {
                Layout.fillWidth: true
            }
            Rectangle {
                Layout.preferredWidth: 6
                Layout.preferredHeight: 6
                radius: 3
                color: root.runtimeIndicatorColor
                visible: root.conversation.threadId.length > 0
            }
            Label {
                text: root.runtimeStatusText
                font.pixelSize: 11
                color: root.runtimeIndicatorColor
                visible: root.conversation.threadId.length > 0
            }
            PanelActionButton {
                objectName: "toggleTerminalButton"
                icon.source: "qrc:///icons/hugeicons/sidebar-bottom.svg"
                enabled: root.terminalController ? root.terminalController.available || root.terminalController.tabs !== null : false
                toolTipText: /*% "Terminal" */ qsTrId("craftward.terminal.title") + " (⌘J)"
                onClicked: root.terminalController.togglePanel()
            }
            PanelActionButton {
                objectName: "toggleFilesButton"
                icon.source: "qrc:///icons/hugeicons/sidebar-right.svg"
                toolTipText: layoutState.filesVisible ? /*% "Hide Files" */ qsTrId("craftward.files.hide") : /*% "Show Files" */ qsTrId("craftward.files.show")
                onClicked: root.toggleFiles()
            }
        }
    }
}
