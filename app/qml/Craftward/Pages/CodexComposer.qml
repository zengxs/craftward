// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import Craftward.Codex

Pane {
    id: root

    required property CodexConversationController controller
    required property bool readOnly
    required property bool startingThread
    readonly property bool attachmentIntakeEnabled: turnControlState.attachmentInputEnabled && !root.readOnly
    property string attachmentNotice

    signal turnSubmitted

    function showAttachmentNotice(message) {
        root.attachmentNotice = message;
        attachmentNoticeTimer.restart();
    }

    function addDescribedAttachments(attachments) {
        if (attachments.length === 0)
            return false;
        root.attachmentNotice = "";
        attachmentNoticeTimer.stop();
        root.controller.acquireWriteAccess();
        composerState.addAttachments(attachments);
        return true;
    }

    function addLocalAttachments(candidates) {
        const localFiles = [];
        for (let index = 0; index < candidates.length; ++index) {
            const candidate = candidates[index];
            if (String(candidate).startsWith("file:"))
                localFiles.push(candidate);
        }
        if (localFiles.length === 0) {
            if (candidates.length > 0)
                root.showAttachmentNotice(qsTr("Only local files can be attached."));
            return false;
        }
        return root.addDescribedAttachments(root.controller.describeAttachments(localFiles));
    }

    function tryPasteAttachments() {
        if (!root.attachmentIntakeEnabled)
            return false;
        return root.addDescribedAttachments(root.controller.attachmentsFromClipboard());
    }

    function submitPrompt() {
        const accepted = root.controller.turnRunning ? root.controller.steerTurn(promptEditor.text) : root.controller.startTurn(promptEditor.text, composerState.attachmentUrls());
        if (accepted)
            root.turnSubmitted();
    }

    function releaseWriteAccessWhenHidden() {
        const window = root.ApplicationWindow.window;
        if (window && !window.visible && !root.startingThread && !root.controller.turnInFlight && root.controller.writeAvailability === CodexConversationController.Writable)
            root.controller.releaseWriteAccess();
    }

    onStartingThreadChanged: releaseWriteAccessWhenHidden()

    Timer {
        id: attachmentNoticeTimer

        interval: 4000
        onTriggered: root.attachmentNotice = ""
    }

    FileDialog {
        id: attachmentDialog

        title: qsTr("Attach local files")
        fileMode: FileDialog.OpenFiles
        nameFilters: [qsTr("All files (*)")]
        onAccepted: root.addLocalAttachments(selectedFiles)
    }

    padding: 10

    background: Rectangle {
        radius: 10
        color: root.palette.base
        border.color: attachmentDropArea.containsDrag ? root.palette.highlight : root.palette.mid
        border.width: attachmentDropArea.containsDrag ? 2 : 1
    }

    contentItem: Item {
        implicitWidth: composerLayout.implicitWidth
        implicitHeight: composerLayout.implicitHeight

        ColumnLayout {
            id: composerLayout

            anchors.fill: parent

            spacing: 6

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Label {
                    text: qsTr("Model")
                    color: root.palette.placeholderText
                    font.pixelSize: 11
                }

                CodexModelSelector {
                    Layout.preferredWidth: 220
                    catalogModel: root.controller.modelCatalog
                    selectedModel: root.controller.model
                    loading: root.controller.loadingModelCatalog
                    errorMessage: root.controller.modelCatalogErrorMessage
                    selectionEnabled: !root.controller.turnInFlight && !root.readOnly
                    onModelSelected: model => root.controller.selectModel(model)
                }

                Item {
                    Layout.fillWidth: true
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Label {
                    text: qsTr("Reasoning")
                    color: root.palette.placeholderText
                    font.pixelSize: 11
                }

                CodexReasoningEffortSelector {
                    Layout.preferredWidth: 160
                    efforts: root.controller.reasoningEfforts
                    selectedEffort: root.controller.reasoningEffort
                    selectionEnabled: !root.controller.turnInFlight && !root.readOnly
                    onEffortSelected: effort => root.controller.selectReasoningEffort(effort)
                }

                Item {
                    Layout.fillWidth: true
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 6

                Label {
                    text: qsTr("Mode")
                    color: root.palette.placeholderText
                    font.pixelSize: 11
                }

                ComboBox {
                    id: turnModeSelector

                    enabled: !root.controller.turnInFlight
                    textRole: "text"
                    valueRole: "value"
                    model: [
                        {
                            text: qsTr("Default"),
                            value: CodexConversationController.DefaultMode
                        },
                        {
                            text: qsTr("Plan"),
                            value: CodexConversationController.PlanMode
                        }
                    ]
                    currentIndex: indexOfValue(root.controller.turnMode)
                    onActivated: root.controller.turnMode = currentValue

                    ToolTip.delay: 500
                    ToolTip.text: currentValue === CodexConversationController.PlanMode ? qsTr("Plan mode can pause to ask structured questions before acting.") : qsTr("Default mode is optimized for carrying out the requested work.")
                    ToolTip.visible: hovered
                }

                Label {
                    text: qsTr("Permissions")
                    color: root.palette.placeholderText
                    font.pixelSize: 11
                }

                ComboBox {
                    id: permissionSelector

                    enabled: !root.controller.turnInFlight
                    textRole: "text"
                    valueRole: "value"
                    model: [
                        {
                            text: qsTr("Current"),
                            value: CodexConversationController.InheritPermissions
                        },
                        {
                            text: qsTr("Ask"),
                            value: CodexConversationController.RequestApproval
                        },
                        {
                            text: qsTr("Read only"),
                            value: CodexConversationController.ReadOnlyPermissions
                        }
                    ]
                    currentIndex: indexOfValue(root.controller.permissionPreset)
                    onActivated: root.controller.permissionPreset = currentValue

                    ToolTip.delay: 500
                    ToolTip.text: {
                        if (currentValue === CodexConversationController.RequestApproval)
                            return qsTr("Allow workspace edits and ask before network access or sandbox escalation.");
                        if (currentValue === CodexConversationController.ReadOnlyPermissions)
                            return qsTr("Keep the turn read-only and ask before an escalation.");
                        return qsTr("Keep the permission settings already associated with this conversation.");
                    }
                    ToolTip.visible: hovered
                }

                Item {
                    Layout.fillWidth: true
                }
            }

            RowLayout {
                Layout.fillWidth: true
                spacing: 8

                Button {
                    text: qsTr("Attach")
                    enabled: root.attachmentIntakeEnabled
                    onClicked: {
                        root.controller.acquireWriteAccess();
                        attachmentDialog.open();
                    }

                    ToolTip.delay: 500
                    ToolTip.text: qsTr("Attach local files to the next turn.")
                    ToolTip.visible: hovered
                }

                TextArea {
                    id: promptEditor

                    Layout.fillWidth: true
                    Layout.preferredHeight: Math.min(Math.max(contentHeight + topPadding + bottomPadding, 44), 120)
                    placeholderText: root.controller.turnRunning ? qsTr("Guide Codex while it works…") : qsTr("Ask Codex to continue this conversation…")
                    enabled: turnControlState.inputEnabled
                    wrapMode: TextEdit.Wrap
                    selectByMouse: true
                    onTextChanged: composerState.saveDraft(text)
                    onActiveFocusChanged: {
                        if (activeFocus)
                            root.controller.acquireWriteAccess();
                    }
                    Keys.onPressed: event => {
                        const commandModifier = event.modifiers & (Qt.ControlModifier | Qt.MetaModifier);
                        if (commandModifier && event.key === Qt.Key_V && root.tryPasteAttachments()) {
                            event.accepted = true;
                        } else if (commandModifier && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) {
                            root.submitPrompt();
                            event.accepted = true;
                        }
                    }
                }

                Button {
                    text: turnControlState.sendLabel
                    enabled: turnControlState.sendEnabled
                    onClicked: turnControlState.send()
                }

                Button {
                    text: turnControlState.stopLabel
                    enabled: turnControlState.stopEnabled
                    visible: turnControlState.stopVisible
                    onClicked: turnControlState.stop()
                }
            }

            ListView {
                id: attachmentList

                Layout.fillWidth: true
                Layout.preferredHeight: visible ? 38 : 0
                visible: count > 0
                orientation: ListView.Horizontal
                spacing: 6
                clip: true
                model: composerState.attachments

                delegate: Rectangle {
                    id: attachmentDelegate

                    required property int index
                    required property var modelData

                    width: attachmentContent.implicitWidth + 6
                    height: 36
                    radius: 5
                    color: root.palette.alternateBase
                    border.color: root.palette.mid

                    RowLayout {
                        id: attachmentContent

                        anchors.fill: parent
                        anchors.margins: 3

                        spacing: 4

                        Image {
                            Layout.preferredWidth: 28
                            Layout.preferredHeight: 28
                            visible: attachmentDelegate.modelData.kind === "localImage"
                            source: attachmentDelegate.modelData.url
                            sourceSize.width: 56
                            sourceSize.height: 56
                            fillMode: Image.PreserveAspectCrop
                            asynchronous: true
                        }

                        Label {
                            Layout.preferredWidth: 28
                            horizontalAlignment: Text.AlignHCenter
                            visible: attachmentDelegate.modelData.kind !== "localImage"
                            text: attachmentDelegate.modelData.kind === "localAudio" ? "♪" : "▤"
                            font.pixelSize: 20
                        }

                        Label {
                            Layout.maximumWidth: 180
                            text: attachmentDelegate.modelData.name
                            elide: Text.ElideMiddle
                        }

                        ToolButton {
                            text: "×"
                            enabled: root.attachmentIntakeEnabled
                            onClicked: composerState.removeAttachment(attachmentDelegate.index)

                            Accessible.name: qsTr("Remove attachment")
                            ToolTip.text: qsTr("Remove attachment")
                            ToolTip.visible: hovered
                        }
                    }
                }
            }

            Label {
                Layout.fillWidth: true
                visible: root.attachmentNotice.length > 0
                text: root.attachmentNotice
                color: root.palette.placeholderText
                font.pixelSize: 11
                wrapMode: Text.WordWrap
            }

            RowLayout {
                Layout.fillWidth: true
                visible: root.controller.writeAvailability === CodexConversationController.Checking || root.controller.writeAvailability === CodexConversationController.Busy || root.controller.writeAvailability === CodexConversationController.Unavailable
                spacing: 6

                BusyIndicator {
                    Layout.preferredWidth: 16
                    Layout.preferredHeight: 16
                    running: root.controller.writeAvailability === CodexConversationController.Checking
                    visible: running
                }

                Label {
                    Layout.fillWidth: true
                    text: {
                        if (root.controller.writeAvailability === CodexConversationController.Checking)
                            return qsTr("Checking whether this conversation is available for writing…");
                        if (root.controller.writeAvailabilityMessage.length > 0)
                            return root.controller.writeAvailabilityMessage;
                        if (root.controller.writeAvailability === CodexConversationController.Busy)
                            return qsTr("This conversation is open in another Codex client. Your draft is kept here.");
                        return qsTr("Writing is currently unavailable for this conversation.");
                    }
                    color: root.controller.writeAvailability === CodexConversationController.Busy ? root.palette.placeholderText : root.palette.text
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                }

                Button {
                    text: qsTr("Retry")
                    visible: root.controller.writeAvailability === CodexConversationController.Busy || root.controller.writeAvailability === CodexConversationController.Unavailable
                    onClicked: root.controller.acquireWriteAccess()
                }
            }
        }

        DropArea {
            id: attachmentDropArea

            anchors.fill: parent
            enabled: root.attachmentIntakeEnabled
            onEntered: drag => drag.accepted = drag.hasUrls
            onDropped: drop => {
                if (drop.hasUrls && root.addLocalAttachments(drop.urls))
                    drop.acceptProposedAction();
            }

            Label {
                anchors.centerIn: parent
                visible: attachmentDropArea.containsDrag
                text: qsTr("Drop files to attach")
                color: root.palette.highlight
                font.bold: true
            }
        }
    }

    CodexComposerState {
        id: composerState

        threadId: root.controller.threadId
        onDraftChanged: {
            if (promptEditor.text !== draft)
                promptEditor.text = draft;
        }
        onEditorShouldLoseFocus: promptEditor.focus = false
    }

    CodexTurnControlState {
        id: turnControlState

        turnInFlight: root.controller.turnInFlight
        turnRunning: root.controller.turnRunning
        steerPending: root.controller.steeringTurn
        interruptPending: root.controller.interruptRequested
        writable: root.controller.writeAvailability === CodexConversationController.Writable
        promptReady: promptEditor.text.trim().length > 0
        attachmentReady: composerState.attachments.length > 0
        onSendRequested: root.submitPrompt()
        onStopRequested: root.controller.interruptTurn()
    }

    Connections {
        target: root.ApplicationWindow.window

        function onVisibleChanged() {
            root.releaseWriteAccessWhenHidden();
        }
    }

    Connections {
        target: root.controller

        function onTurnStateChanged() {
            root.releaseWriteAccessWhenHidden();
        }

        function onTurnStarted() {
            composerState.confirmSubmission();
        }

        function onTurnSteered() {
            composerState.confirmTextSubmission();
        }
    }
}
