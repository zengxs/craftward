// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Codex

Pane {
    id: root

    required property CodexConversationController controller
    required property bool readOnly
    required property bool startingThread

    signal turnSubmitted

    function submitPrompt() {
        const accepted = root.controller.turnRunning ? root.controller.steerTurn(promptEditor.text) : root.controller.startTurn(promptEditor.text);
        if (accepted)
            root.turnSubmitted();
    }

    function releaseWriteAccessWhenHidden() {
        const window = root.ApplicationWindow.window;
        if (window && !window.visible && !root.startingThread && !root.controller.turnInFlight && root.controller.writeAvailability === CodexConversationController.Writable)
            root.controller.releaseWriteAccess();
    }

    onStartingThreadChanged: releaseWriteAccessWhenHidden()

    padding: 10

    background: Rectangle {
        radius: 10
        color: root.palette.base
        border.color: root.palette.mid
    }

    contentItem: ColumnLayout {
        id: composerLayout

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
                    const submitModifier = event.modifiers & (Qt.ControlModifier | Qt.MetaModifier);
                    if (submitModifier && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter)) {
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
            composerState.confirmSubmission();
        }
    }
}
