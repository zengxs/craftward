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

    TestCase {
        name: "CodexTurnControlState"

        function init() {
            suite.state = stateComponent.createObject(suite);
            verify(suite.state !== null);
            sendSpy.target = suite.state;
            stopSpy.target = suite.state;
            sendSpy.clear();
            stopSpy.clear();
        }

        function cleanup() {
            sendSpy.target = null;
            stopSpy.target = null;
            suite.state.destroy();
            suite.state = null;
        }

        function test_sendRequiresWritableNonemptyPrompt() {
            compare(suite.state.label, "Send");
            verify(!suite.state.enabled);

            suite.state.writable = true;
            suite.state.promptReady = true;
            verify(suite.state.enabled);
            suite.state.activate();
            compare(sendSpy.count, 1);
            compare(stopSpy.count, 0);
        }

        function test_runningTurnOffersStopEvenWithoutPrompt() {
            suite.state.turnInFlight = true;
            compare(suite.state.label, "Stop");
            verify(suite.state.enabled);

            suite.state.activate();
            compare(sendSpy.count, 0);
            compare(stopSpy.count, 1);
        }

        function test_pendingInterruptDisablesRepeatedStop() {
            suite.state.turnInFlight = true;
            suite.state.interruptPending = true;
            compare(suite.state.label, "Stopping…");
            verify(!suite.state.enabled);

            suite.state.activate();
            compare(stopSpy.count, 0);
        }
    }
}
