// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtTest
import Craftward.TestSupport
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite
    width: 720
    height: 540
    property int openCount: 0
    property int closeCount: 0

    NativePointer {
        id: pointer
    }
    AbstractButton {
        id: chip
        x: 80
        y: 35
        width: 130
        height: 32
        hoverEnabled: true
        text: "1 annotation"
        contentItem: Text {
            text: chip.text
        }
        background: Rectangle {
            color: "white"
            border.color: "gray"
            radius: 16
        }
        onHoveredChanged: popup.handle("user", hovered ? "hover" : "leave", chip, {
            index: 0,
            key: "collection",
            rect: Qt.rect(0, 0, width, height)
        })
    }
    Pages.CodexAnnotationPopup {
        id: popup
        resolveAnnotations: () => [
                {
                    index: 1,
                    inputNumber: 1,
                    text: "Selected response text",
                    comment: "Change this",
                    sourceEntryId: "user"
                }
            ]
    }
    Connections {
        target: popup
        function onOpened() {
            suite.openCount++;
        }
        function onClosed() {
            suite.closeCount++;
        }
    }
    Timer {
        id: stationaryPointer
        interval: 30
        repeat: true
        onTriggered: pointer.moveTo(chip, Qt.point(30, 10))
    }
    TestCase {
        name: "CodexAnnotationNativeHover"
        when: windowShown

        function test_stationaryPointerAndCrossWindowTransferKeepOnePopup() {
            if (!pointer.available)
                skip("This regression requires native macOS pointer and window events.");
            const previous = pointer.position();
            try {
                suite.Window.window.requestActivate();
                pointer.moveTo(suite, Qt.point(1, suite.height - 1));
                wait(150);
                popup.dismiss();
                suite.openCount = 0;
                suite.closeCount = 0;
                pointer.moveTo(chip, Qt.point(30, 10));
                // Keep the OS pointer fixed while showing the popup clears the source hover state.
                stationaryPointer.start();
                tryCompare(popup, "opened", true);
                wait(2000);
                compare(suite.openCount, 1, "A stationary pointer must open the popup exactly once");
                compare(suite.closeCount, 0, "Showing the native window must not dismiss the popup");
                stationaryPointer.stop();

                pointer.moveTo(chip, Qt.point(30, chip.height + 4));
                wait(popup.leaveDelay / 2);
                verify(popup.visible, "Crossing the gap must retain the full leave grace period");
                pointer.moveTo(popup.contentItem, Qt.point(15, 15));
                wait(popup.leaveDelay + 100);
                verify(popup.visible);
                pointer.moveTo(chip, Qt.point(30, 10));
                wait(popup.leaveDelay + 100);
                verify(popup.visible);
                // The panel padding is part of the interactive popup, too.
                pointer.moveTo(popup.contentItem, Qt.point(1 - popup.leftPadding, 15));
                wait(popup.leaveDelay + 100);
                verify(popup.visible);
                pointer.moveTo(suite, Qt.point(1, suite.height - 1));
                tryCompare(popup, "visible", false);
                compare(suite.openCount, 1);
                compare(suite.closeCount, 1);
            } finally {
                stationaryPointer.stop();
                popup.dismiss();
                pointer.restore(previous);
            }
        }
    }
}
