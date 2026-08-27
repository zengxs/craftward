// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 120
    height: 80
    property var button

    Component {
        id: buttonComponent

        Pages.CodexComposerActionButton {}
    }

    TestCase {
        name: "CodexComposerActionButton"
        when: windowShown

        function init() {
            suite.button = createTemporaryObject(buttonComponent, suite);
            verify(suite.button !== null);
        }

        function cleanup() {
            suite.button = null;
        }

        function test_usesStableCircularGeometryAcrossActions() {
            compare(suite.button.implicitWidth, 32);
            compare(suite.button.implicitHeight, 32);
            compare(suite.button.displayedAction, Pages.CodexComposerAction.SendAction);
            compare(suite.button.toolTipText, "Send");
            verify(String(suite.button.displayedIconSource).endsWith("arrow-up-20-filled.svg"));

            suite.button.composerAction = Pages.CodexComposerAction.StopAction;
            tryCompare(suite.button, "displayedAction", Pages.CodexComposerAction.StopAction, 300);
            verify(String(suite.button.displayedIconSource).endsWith("stop-20-filled.svg"));
            compare(suite.button.toolTipText, "Stop");
            compare(suite.button.implicitWidth, 32);
            compare(suite.button.implicitHeight, 32);

            suite.button.composerAction = Pages.CodexComposerAction.ContinueAction;
            tryCompare(suite.button, "displayedAction", Pages.CodexComposerAction.ContinueAction, 300);
            verify(String(suite.button.displayedIconSource).endsWith("play-20-filled.svg"));
            compare(suite.button.toolTipText, "Continue");
            compare(suite.button.implicitWidth, 32);
            compare(suite.button.implicitHeight, 32);
        }
    }
}
