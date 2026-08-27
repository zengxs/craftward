// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    enum TurnMode {
        DefaultMode,
        PlanMode
    }

    enum PermissionPreset {
        InheritPermissions,
        RequestApproval,
        ReadOnlyPermissions
    }

    enum WriteAvailability {
        NotRequested,
        Checking,
        Writable,
        Busy,
        Unavailable
    }

    property var modelCatalog: null
    property var timeline
    property string threadId
    property string model
    property string reasoningEffort
    property var reasoningEfforts: []
    property bool loading: false
    property bool loadingModelCatalog: false
    property string modelCatalogErrorMessage
    property bool hasRunningEvidence: false
    property bool hasInterruptedLatestTurn: false
    property bool turnInFlight: false
    property bool turnRunning: false
    property bool steeringTurn: false
    property bool interruptRequested: false
    property bool waitingOnApproval: false
    property bool waitingOnUserInput: false
    property int turnMode: CodexConversationController.DefaultMode
    property int permissionPreset: CodexConversationController.InheritPermissions
    property int writeAvailability: CodexConversationController.NotRequested
    property string writeAvailabilityMessage
    property int continueTurnCalls: 0
    property int acquireWriteAccessCalls: 0

    signal selectionChanged
    signal turnStateChanged
    signal turnStarted
    signal turnSteered

    function acquireWriteAccess() {
        ++acquireWriteAccessCalls;
        writeAvailability = CodexConversationController.Checking;
    }

    function releaseWriteAccess() {
    }

    function describeAttachments(attachments) {
        return [];
    }

    function attachmentsFromClipboard() {
        return [];
    }

    function selectModel(model) {
        return true;
    }

    function selectReasoningEffort(effort) {
        return true;
    }

    function startTurn(prompt, attachments) {
        return true;
    }

    function steerTurn(prompt, attachments) {
        return true;
    }

    function interruptTurn() {
        return true;
    }

    function continueTurn() {
        ++continueTurnCalls;
        if (writeAvailability === CodexConversationController.NotRequested)
            acquireWriteAccess();
        if (writeAvailability !== CodexConversationController.Checking && writeAvailability !== CodexConversationController.Writable)
            return false;
        return true;
    }
}
