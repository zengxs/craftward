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

    TestCase {
        name: "CodexTurnControlState"

        function init() {
            suite.state = stateComponent.createObject(suite);
            verify(suite.state !== null);
            sendSpy.target = suite.state;
            sendSpy.clear();
        }

        function cleanup() {
            sendSpy.target = null;
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
            suite.state.turnInFlight = true;
            verify(!suite.state.enabled);
        }
    }
}
