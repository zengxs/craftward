// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 320
    height: 180

    property var state

    Component {
        id: stateComponent

        Pages.CodexTurnControlState {}
    }

    SignalSpy {
        id: sendSpy

        signalName: "sendRequested"
    }

    SignalSpy {
        id: stopSpy

        signalName: "stopRequested"
    }

    SignalSpy {
        id: continueSpy

        signalName: "continueRequested"
    }

    TestCase {
        name: "CodexTurnControlState"

        function init() {
            suite.state = stateComponent.createObject(suite);
            verify(suite.state !== null);
            sendSpy.target = suite.state;
            stopSpy.target = suite.state;
            continueSpy.target = suite.state;
            sendSpy.clear();
            stopSpy.clear();
            continueSpy.clear();
        }

        function cleanup() {
            sendSpy.target = null;
            stopSpy.target = null;
            continueSpy.target = null;
            suite.state.destroy();
            suite.state = null;
        }

        function test_sendRequiresWritableNonemptyPrompt() {
            compare(suite.state.sendLabel, "Send");
            compare(suite.state.primaryAction, Pages.CodexComposerAction.SendAction);
            verify(!suite.state.sendEnabled);
            verify(!suite.state.primaryEnabled);

            suite.state.writable = true;
            suite.state.promptReady = true;
            verify(suite.state.sendEnabled);
            verify(suite.state.primaryEnabled);
            suite.state.activatePrimaryAction();
            compare(sendSpy.count, 1);
            compare(stopSpy.count, 0);
        }

        function test_attachmentCanStartAndGuideATurnWithoutText() {
            suite.state.writable = true;
            suite.state.attachmentReady = true;
            verify(suite.state.sendEnabled);

            suite.state.turnInFlight = true;
            verify(!suite.state.sendEnabled);

            suite.state.turnRunning = true;
            verify(suite.state.sendEnabled);
            compare(suite.state.primaryAction, Pages.CodexComposerAction.SendAction);
            suite.state.activatePrimaryAction();
            compare(sendSpy.count, 1);
        }

        function test_inputFollowsTheSubmissionLifecycle() {
            verify(suite.state.inputEnabled);

            suite.state.turnInFlight = true;
            verify(!suite.state.inputEnabled);

            suite.state.turnRunning = true;
            verify(suite.state.inputEnabled);

            suite.state.steerPending = true;
            verify(!suite.state.inputEnabled);

            suite.state.steerPending = false;
            suite.state.interruptPending = true;
            verify(!suite.state.inputEnabled);
        }

        function test_attachmentInputIgnoresPendingTurnControls() {
            verify(suite.state.attachmentInputEnabled);

            suite.state.turnInFlight = true;
            verify(!suite.state.attachmentInputEnabled);

            suite.state.turnRunning = true;
            verify(suite.state.attachmentInputEnabled);

            suite.state.steerPending = true;
            verify(suite.state.attachmentInputEnabled);

            suite.state.interruptPending = true;
            verify(suite.state.attachmentInputEnabled);
        }

        function test_runningTurnUsesSendWhenGuidanceIsReady() {
            suite.state.turnInFlight = true;
            suite.state.turnRunning = true;
            suite.state.writable = true;
            suite.state.promptReady = true;
            compare(suite.state.sendLabel, "Guide");
            verify(suite.state.sendEnabled);
            verify(suite.state.stopEnabled);
            compare(suite.state.primaryAction, Pages.CodexComposerAction.SendAction);
            compare(suite.state.primaryToolTip, "Guide");

            suite.state.activatePrimaryAction();
            compare(sendSpy.count, 1);
            compare(stopSpy.count, 0);
        }

        function test_runningTurnCanStopWithoutGuidance() {
            suite.state.turnInFlight = true;
            suite.state.turnRunning = true;
            suite.state.writable = true;

            verify(!suite.state.sendEnabled);
            verify(suite.state.stopEnabled);
            compare(suite.state.primaryAction, Pages.CodexComposerAction.StopAction);
            compare(suite.state.primaryToolTip, "Stop");
            suite.state.activatePrimaryAction();
            compare(stopSpy.count, 1);
        }

        function test_interruptedTurnCanContinueWithoutNewInput() {
            suite.state.continuationRequestable = true;
            suite.state.continuationAvailable = true;

            compare(suite.state.primaryAction, Pages.CodexComposerAction.ContinueAction);
            compare(suite.state.primaryToolTip, "Continue");
            verify(suite.state.primaryEnabled);

            suite.state.activatePrimaryAction();
            compare(continueSpy.count, 1);
            compare(sendSpy.count, 0);
            compare(stopSpy.count, 0);

            suite.state.promptReady = true;
            compare(suite.state.primaryAction, Pages.CodexComposerAction.SendAction);
        }

        function test_startingTurnCanStopWhileGuidanceStaysDisabled() {
            suite.state.turnInFlight = true;
            suite.state.writable = true;
            suite.state.promptReady = true;

            compare(suite.state.sendLabel, "Starting…");
            verify(!suite.state.sendEnabled);
            verify(suite.state.stopEnabled);
            compare(suite.state.primaryAction, Pages.CodexComposerAction.StopAction);

            suite.state.activatePrimaryAction();
            compare(sendSpy.count, 0);
            compare(stopSpy.count, 1);
        }

        function test_pendingGuidancePreventsDuplicateSendButKeepsStop() {
            suite.state.turnInFlight = true;
            suite.state.turnRunning = true;
            suite.state.writable = true;
            suite.state.promptReady = true;
            suite.state.steerPending = true;
            compare(suite.state.sendLabel, "Guiding…");
            verify(!suite.state.sendEnabled);
            verify(suite.state.stopEnabled);
            compare(suite.state.primaryAction, Pages.CodexComposerAction.StopAction);

            suite.state.activatePrimaryAction();
            compare(sendSpy.count, 0);
            compare(stopSpy.count, 1);
        }

        function test_pendingInterruptDisablesGuidanceAndRepeatedStop() {
            suite.state.turnInFlight = true;
            suite.state.turnRunning = true;
            suite.state.writable = true;
            suite.state.promptReady = true;
            suite.state.interruptPending = true;
            compare(suite.state.stopLabel, "Stopping…");
            verify(!suite.state.sendEnabled);
            verify(!suite.state.stopEnabled);
            compare(suite.state.primaryAction, Pages.CodexComposerAction.StopAction);
            verify(!suite.state.primaryEnabled);

            suite.state.activatePrimaryAction();
            compare(sendSpy.count, 0);
            compare(stopSpy.count, 0);
        }
    }
}
