// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Effects
import QtQuick.Layouts
import Craftward.Codex
import Craftward.Components
import Craftward.Design

Pane {
    id: root

    required property CodexConversationController controller
    required property bool readOnly
    required property bool startingThread
    readonly property bool attachmentIntakeEnabled: turnControlState.attachmentInputEnabled && !root.readOnly
    readonly property string pastedImageName: /*% "Pasted image.png" */ qsTrId("craftward.clipboard_image.default_filename")
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
            if (candidates.length > 0) {
                //% "Only local files can be attached."
                root.showAttachmentNotice(qsTrId("craftward.codex.attachment.local_only"));
            }
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
        const attachments = composerState.attachmentUrls();
        const guidance = root.controller.turnRunning;
        const submittedText = promptEditor.text;
        const accepted = guidance ? root.controller.steerTurn(submittedText, attachments) : root.controller.startTurn(submittedText, attachments);
        if (accepted && guidance)
            composerState.trackGuidanceSubmission(submittedText);
        if (accepted)
            root.turnSubmitted();
    }

    function submitContinuation() {
        if (root.controller.continueTurn())
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

        title: /*% "Attach local files" */ qsTrId("craftward.codex.composer.file_dialog.title")
        fileMode: FileDialog.OpenFiles
        nameFilters: [/*% "All files (*)" */ qsTrId("craftward.file_filter.all")]
        onAccepted: root.addLocalAttachments(selectedFiles)
    }

    padding: 10

    background: Item {
        RectangularShadow {
            anchors.fill: composerSurface
            radius: composerSurface.radius
            blur: 14
            cached: true
            color: Theme.composerAmbientShadow
            offset: Qt.vector2d(0, 3)
        }

        Rectangle {
            id: composerSurface

            anchors.fill: parent
            radius: 16
            color: Theme.composerSurface
            border.color: attachmentDropArea.containsDrag ? root.palette.highlight : Theme.composerBorder
            border.width: attachmentDropArea.containsDrag ? 2 : 0.5
            border.pixelAligned: false
        }
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
                    text: /*% "Model" */ qsTrId("craftward.codex.composer.model.label")
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
                    text: /*% "Reasoning" */ qsTrId("craftward.codex.composer.reasoning.label")
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
                    text: /*% "Mode" */ qsTrId("craftward.codex.composer.mode.label")
                    color: root.palette.placeholderText
                    font.pixelSize: 11
                }

                MenuComboBox {
                    id: turnModeSelector

                    enabled: !root.controller.turnInFlight
                    textRole: "text"
                    valueRole: "value"
                    model: [
                        {
                            //% "Default"
                            text: qsTrId("craftward.codex.composer.mode.default"),
                            value: CodexConversationController.DefaultMode
                        },
                        {
                            //% "Plan"
                            text: qsTrId("craftward.codex.composer.mode.plan"),
                            value: CodexConversationController.PlanMode
                        }
                    ]
                    currentIndex: indexOfValue(root.controller.turnMode)
                    onActivated: root.controller.turnMode = currentValue

                    ToolTip.delay: 500
                    ToolTip.text: currentValue === CodexConversationController.PlanMode ? /*% "Plan mode can pause to ask structured questions before acting." */ qsTrId("craftward.codex.composer.mode.plan.description") : /*% "Default mode is optimized for carrying out the requested work." */ qsTrId("craftward.codex.composer.mode.default.description")
                    ToolTip.visible: hovered
                }

                Label {
                    text: /*% "Permissions" */ qsTrId("craftward.codex.composer.permissions.label")
                    color: root.palette.placeholderText
                    font.pixelSize: 11
                }

                MenuComboBox {
                    id: permissionSelector

                    enabled: !root.controller.turnInFlight
                    textRole: "text"
                    valueRole: "value"
                    model: [
                        {
                            //% "Current"
                            text: qsTrId("craftward.codex.composer.permissions.current"),
                            value: CodexConversationController.InheritPermissions
                        },
                        {
                            //% "Ask"
                            text: qsTrId("craftward.codex.composer.permissions.ask"),
                            value: CodexConversationController.RequestApproval
                        },
                        {
                            //% "Read only"
                            text: qsTrId("craftward.codex.composer.permissions.read_only"),
                            value: CodexConversationController.ReadOnlyPermissions
                        }
                    ]
                    currentIndex: indexOfValue(root.controller.permissionPreset)
                    onActivated: root.controller.permissionPreset = currentValue

                    ToolTip.delay: 500
                    ToolTip.text: {
                        if (currentValue === CodexConversationController.RequestApproval)
                            return /*% "Allow workspace edits and ask before network access or sandbox escalation." */ qsTrId("craftward.codex.composer.permissions.ask.description");
                        if (currentValue === CodexConversationController.ReadOnlyPermissions)
                            return /*% "Keep the turn read-only and ask before an escalation." */ qsTrId("craftward.codex.composer.permissions.read_only.description");
                        return /*% "Keep the permission settings already associated with this conversation." */ qsTrId("craftward.codex.composer.permissions.current.description");
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
                    text: /*% "Attach" */ qsTrId("craftward.codex.composer.attachment.action")
                    enabled: root.attachmentIntakeEnabled
                    onClicked: {
                        root.controller.acquireWriteAccess();
                        attachmentDialog.open();
                    }

                    ToolTip.delay: 500
                    ToolTip.text: /*% "Attach local files." */ qsTrId("craftward.codex.composer.attachment.tooltip")
                    ToolTip.visible: hovered
                }

                TextArea {
                    id: promptEditor

                    objectName: "codexComposerPromptEditor"

                    Layout.fillWidth: true
                    Layout.preferredHeight: Math.min(Math.max(contentHeight + topPadding + bottomPadding, 44), 120)
                    placeholderText: root.controller.turnRunning ? /*% "Guide Codex while it works…" */ qsTrId("craftward.codex.composer.placeholder.guide") : /*% "Ask Codex to continue this conversation…" */ qsTrId("craftward.codex.composer.placeholder.continue")
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

                CodexComposerActionButton {
                    objectName: "codexComposerPrimaryAction"
                    Layout.alignment: Qt.AlignBottom
                    composerAction: turnControlState.primaryAction
                    enabled: turnControlState.primaryEnabled
                    statusText: turnControlState.primaryToolTip
                    onClicked: turnControlState.activatePrimaryAction()
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
                            text: attachmentDelegate.modelData.nameKind === "pastedImage" ? root.pastedImageName : attachmentDelegate.modelData.name
                            elide: Text.ElideMiddle
                        }

                        ToolButton {
                            text: "×"
                            enabled: root.attachmentIntakeEnabled
                            onClicked: composerState.removeAttachment(attachmentDelegate.index)

                            Accessible.name: /*% "Remove attachment" */ qsTrId("craftward.codex.composer.attachment.remove")
                            ToolTip.text: /*% "Remove attachment" */ qsTrId("craftward.codex.composer.attachment.remove")
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
                            return /*% "Checking whether this conversation is available for writing…" */ qsTrId("craftward.codex.composer.write_state.checking");
                        if (root.controller.writeAvailabilityMessage.length > 0)
                            return root.controller.writeAvailabilityMessage;
                        if (root.controller.writeAvailability === CodexConversationController.Busy)
                            return /*% "This conversation is open in another Codex client. Your draft is kept here." */ qsTrId("craftward.codex.composer.write_state.busy");
                        return /*% "Writing is currently unavailable for this conversation." */ qsTrId("craftward.codex.composer.write_state.unavailable");
                    }
                    color: root.controller.writeAvailability === CodexConversationController.Busy ? root.palette.placeholderText : root.palette.text
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                }

                Button {
                    text: /*% "Retry" */ qsTrId("craftward.action.retry")
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
                text: /*% "Drop files to attach" */ qsTrId("craftward.codex.composer.drop_files")
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
        continuationRequestable: !root.readOnly && !root.startingThread && (root.controller.writeAvailability === CodexConversationController.NotRequested || writable)
        promptReady: promptEditor.text.trim().length > 0
        attachmentReady: composerState.attachments.length > 0
        continuationAvailable: root.controller.hasInterruptedLatestTurn
        onSendRequested: root.submitPrompt()
        onStopRequested: root.controller.interruptTurn()
        onContinueRequested: root.submitContinuation()
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
            composerState.confirmGuidanceSubmission();
        }
    }
}
