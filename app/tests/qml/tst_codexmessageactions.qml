// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 320
    height: 120
    property var actions

    Component {
        id: actionsComponent

        Pages.CodexMessageActions {
            x: 20
            y: 20
            available: true
            revealed: true
            forkVisible: true
            forkEnabled: true
            copyFeedbackDuration: 40
            copyIconSource: ""
            copiedIconSource: ""
            forkIconSource: ""
        }
    }

    SignalSpy {
        id: copySpy

        signalName: "copyRequested"
    }

    SignalSpy {
        id: forkSpy

        signalName: "forkRequested"
    }

    TestCase {
        name: "CodexMessageActions"
        when: windowShown

        function init() {
            suite.actions = actionsComponent.createObject(suite);
            verify(suite.actions !== null);
            copySpy.target = suite.actions;
            forkSpy.target = suite.actions;
            copySpy.clear();
            forkSpy.clear();
        }

        function cleanup() {
            copySpy.target = null;
            forkSpy.target = null;
            suite.actions.destroy();
            suite.actions = null;
        }

        function test_reservesHeightWhileVisuallyHidden() {
            compare(suite.actions.implicitHeight, 24);
            suite.actions.revealed = false;
            tryCompare(suite.actions, "opacity", 0);
            compare(suite.actions.implicitHeight, 24);

            suite.actions.available = false;
            compare(suite.actions.implicitHeight, 0);
            verify(!suite.actions.visible);
        }

        function test_emitsCopyAndShowsTemporaryFeedback() {
            const copyButton = findChild(suite.actions, "codexMessageCopyButton");
            verify(copyButton !== null);

            mouseClick(copyButton);
            compare(copySpy.count, 1);

            suite.actions.confirmCopied();
            tryVerify(function () {
                return suite.actions.copied;
            });
            compare(copyButton.toolTipText, "Copied");
            verify(copyButton.forceToolTipVisible);

            tryCompare(suite.actions, "copied", false, 500);
            compare(copyButton.toolTipText, "Copy");
            verify(!copyButton.forceToolTipVisible);
        }

        function test_exposesForkOnlyWhenRequested() {
            const forkButton = findChild(suite.actions, "codexMessageForkButton");
            verify(forkButton !== null);
            compare(forkButton.contentItem.rotation, -90);

            mouseClick(forkButton);
            compare(forkSpy.count, 1);

            suite.actions.forkVisible = false;
            verify(!forkButton.visible);
        }
    }
}
