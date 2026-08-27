// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import Craftward.Codex
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 720
    height: 480
    property var composer
    property bool readOnly: false

    CodexConversationController {
        id: fakeController

        threadId: "thread-1"
        writeAvailability: CodexConversationController.NotRequested
    }

    Component {
        id: composerComponent

        Pages.CodexComposer {
            width: 680
            controller: fakeController
            readOnly: suite.readOnly
            startingThread: false
        }
    }

    SignalSpy {
        id: submittedSpy

        target: suite.composer
        signalName: "turnSubmitted"
    }

    TestCase {
        name: "CodexComposerIntegration"
        when: windowShown

        function init() {
            fakeController.continueTurnCalls = 0;
            fakeController.acquireWriteAccessCalls = 0;
            fakeController.writeAvailability = CodexConversationController.NotRequested;
            fakeController.hasInterruptedLatestTurn = false;
            fakeController.turnInFlight = false;
            fakeController.turnRunning = false;
            fakeController.steeringTurn = false;
            fakeController.interruptRequested = false;
            suite.readOnly = false;
            suite.composer = createTemporaryObject(composerComponent, suite);
            verify(suite.composer !== null);
            submittedSpy.clear();
        }

        function cleanup() {
            suite.composer = null;
        }

        function test_mapsInterruptedHistoryToTheContinueAction() {
            const action = findChild(suite.composer, "codexComposerPrimaryAction");
            verify(action !== null);
            compare(action.enabled, false);

            fakeController.hasInterruptedLatestTurn = true;

            tryCompare(action, "composerAction", Pages.CodexComposerAction.ContinueAction);
            tryCompare(action, "displayedAction", Pages.CodexComposerAction.ContinueAction, 300);
            verify(action.enabled);
            mouseClick(action, action.width / 2, action.height / 2);
            compare(fakeController.continueTurnCalls, 1);
            compare(fakeController.acquireWriteAccessCalls, 1);
            compare(fakeController.writeAvailability, CodexConversationController.Checking);
            compare(submittedSpy.count, 1);

            const editor = findChild(suite.composer, "codexComposerPromptEditor");
            verify(editor !== null);
            editor.text = "Use a different approach";
            tryCompare(action, "composerAction", Pages.CodexComposerAction.SendAction);
        }

        function test_archivedInterruptedHistoryKeepsContinueDisabled() {
            const action = findChild(suite.composer, "codexComposerPrimaryAction");
            verify(action !== null);

            suite.readOnly = true;
            fakeController.hasInterruptedLatestTurn = true;

            tryCompare(action, "composerAction", Pages.CodexComposerAction.ContinueAction);
            verify(!action.enabled);
        }

        function test_writableInterruptedHistoryContinuesImmediately() {
            const action = findChild(suite.composer, "codexComposerPrimaryAction");
            verify(action !== null);
            fakeController.writeAvailability = CodexConversationController.Writable;
            fakeController.hasInterruptedLatestTurn = true;

            tryCompare(action, "composerAction", Pages.CodexComposerAction.ContinueAction);
            verify(action.enabled);
            mouseClick(action, action.width / 2, action.height / 2);

            compare(fakeController.continueTurnCalls, 1);
            compare(fakeController.acquireWriteAccessCalls, 0);
            compare(submittedSpy.count, 1);
        }
    }
}
