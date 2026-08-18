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

        Pages.CodexConversationRenameState {}
    }

    SignalSpy {
        id: renameSpy

        signalName: "renameRequested"
    }

    TestCase {
        name: "CodexConversationRenameState"

        function init() {
            suite.state = stateComponent.createObject(suite);
            verify(suite.state !== null);
            renameSpy.target = suite.state;
            renameSpy.clear();
        }

        function cleanup() {
            renameSpy.target = null;
            suite.state.destroy();
            suite.state = null;
        }

        function test_resetUsesTheCurrentConversationName() {
            suite.state.reset("Existing name");

            compare(suite.state.currentName, "Existing name");
            compare(suite.state.draft, "Existing name");
            verify(!suite.state.canSubmit);
        }

        function test_submissionRequiresAChangedNonemptyName() {
            suite.state.reset("Existing name");
            suite.state.draft = "   ";
            verify(!suite.state.canSubmit);
            verify(!suite.state.submit());

            suite.state.draft = "  Existing name  ";
            verify(!suite.state.canSubmit);
            verify(!suite.state.submit());

            suite.state.draft = "  Focused work  ";
            verify(suite.state.canSubmit);
            verify(suite.state.submit());
            compare(renameSpy.count, 1);
            compare(renameSpy.signalArguments[0][0], "Focused work");
        }

        function test_unavailableConversationPreventsSubmission() {
            suite.state.reset("Existing name");
            suite.state.draft = "Focused work";
            suite.state.renameAllowed = false;

            verify(!suite.state.canSubmit);
            verify(!suite.state.submit());
            compare(renameSpy.count, 0);
        }
    }
}
