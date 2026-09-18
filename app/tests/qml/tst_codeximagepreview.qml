// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtTest
import Craftward.Components
import Craftward.TestSupport
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite
    width: 720
    height: 540
    readonly property var images: [
        {
            image: true,
            url: Qt.resolvedUrl("fixtures/attachment-landscape.png")
        },
        {
            image: true,
            url: Qt.resolvedUrl("fixtures/attachment-portrait.png")
        }
    ]

    Button {
        id: anchor
        x: 80
        y: 35
        width: 90
        height: 90
        onClicked: preview.handle("open", anchor, suite.images)
    }
    Button {
        id: secondAnchor
        x: 190
        y: 35
        width: 90
        height: 90
        onClicked: preview.handle("open", secondAnchor, suite.images, 1)
    }
    Pages.CodexImagePreview {
        id: preview
    }
    NativePointer {
        id: pointer
    }
    TestCase {
        name: "CodexImagePreview"
        when: windowShown

        function init() {
            preview.dismiss();
            anchor.x = 80;
            anchor.y = 35;
            mouseMove(suite, 1, suite.height - 1);
            wait(50);
        }
        function cleanup() {
            preview.dismiss();
        }
        function test_secondClickTogglesTheAnchor_data() {
            return [
                {
                    tag: "owner-window",
                    throughPopup: false
                },
                {
                    tag: "native-popup-grab",
                    throughPopup: true
                }
            ];
        }
        function test_secondClickTogglesTheAnchor(data) {
            suite.Window.window.requestActivate();
            tryCompare(suite.Window.window, "active", true);
            mouseClick(anchor);
            tryCompare(preview, "opened", true);
            verify(waitForRendering(preview.contentItem));

            if (data.throughPopup) {
                const global = anchor.mapToGlobal(anchor.width / 2, anchor.height / 2);
                const point = preview.contentItem.mapFromGlobal(global.x, global.y);
                // QtTest bypasses the platform replay of an unaccepted outside press.
                if (!pointer.pressAccepted(preview.contentItem, point))
                    mousePress(anchor);
                mouseRelease(anchor);
            } else {
                mouseClick(anchor);
            }
            wait(100);
            verify(!preview.visible, "Clicking the open preview's thumbnail again must leave it closed.");

            mouseClick(anchor);
            tryCompare(preview, "opened", true);
        }

        function test_anotherThumbnailStillReceivesTheReplayedClick() {
            suite.Window.window.requestActivate();
            tryCompare(suite.Window.window, "active", true);
            mouseClick(anchor);
            tryCompare(preview, "opened", true);
            verify(waitForRendering(preview.contentItem));

            const global = secondAnchor.mapToGlobal(secondAnchor.width / 2, secondAnchor.height / 2);
            const point = preview.contentItem.mapFromGlobal(global.x, global.y);
            const accepted = pointer.pressAccepted(preview.contentItem, point);
            verify(!accepted, "Only the current anchor's press may be consumed.");
            mousePress(secondAnchor);
            mouseRelease(secondAnchor);

            tryCompare(preview, "opened", true);
            compare(preview.anchorItem, secondAnchor);
            compare(preview.currentIndex, 1);
            mouseClick(secondAnchor);
            tryCompare(preview, "visible", false);
            suite.Window.window.requestActivate();
            tryVerify(() => secondAnchor.activeFocus);
            keyClick(Qt.Key_Space);
            tryCompare(preview, "opened", true);
            keyClick(Qt.Key_Escape);
            tryCompare(preview, "visible", false);
        }
        function test_repeatedOpenKeepsNativeGeometry_data() {
            return [
                {
                    tag: "top-left",
                    x: 35,
                    y: 35
                },
                {
                    tag: "bottom-right",
                    x: 610,
                    y: 420
                }
            ];
        }
        function test_repeatedOpenKeepsNativeGeometry(data) {
            anchor.x = data.x;
            anchor.y = data.y;
            wait(50);
            for (let attempt = 0; attempt < 10; ++attempt) {
                // Synthetic Qt clicks do not activate the native owner like physical clicks do.
                suite.Window.window.requestActivate();
                tryCompare(suite.Window.window, "active", true);
                mouseClick(anchor);
                tryCompare(preview, "opened", true);
                tryCompare(findChild(preview, "codexAttachmentFullImage"), "status", Image.Ready);
                verify(waitForRendering(preview.contentItem));
                const expected = PopupPositioner.place(anchor, Qt.rect(0, 0, anchor.width, anchor.height), preview.parent, Qt.size(480, 400));
                const expectedGlobal = preview.parent.mapToGlobal(expected.x, expected.y);
                tryVerify(() => {
                    const actual = preview.contentItem.parent.mapToGlobal(0, 0);
                    return Math.abs(actual.x - expectedGlobal.x) <= 1 && Math.abs(actual.y - expectedGlobal.y) <= 1;
                }, 1000, "The native preview must appear at its requested anchor position.");
                compare(preview.width, expected.width);
                compare(preview.height, expected.height);
                mouseClick(findChild(preview, "imagePreviewNext"));
                tryCompare(findChild(preview, "codexAttachmentFullImage"), "status", Image.Ready);
                compare(preview.width, expected.width);
                compare(preview.height, expected.height);
                keyClick(Qt.Key_Escape);
                tryCompare(preview, "visible", false);
                wait(50);
            }
        }

        function test_toolbarHintsDoNotDismissPreview() {
            mouseClick(anchor);
            tryCompare(preview, "opened", true);
            const button = findChild(preview, "imagePreviewOpenTab");
            mouseMove(button, button.width / 2, button.height / 2);
            tryCompare(button, "hovered", true);
            wait(600);
            verify(preview.opened);
            compare(button.Accessible.name, "Open in Tab");
            mouseMove(preview.contentItem, 200, 150);
            wait(50);
            verify(preview.opened);
        }

        function test_arrowKeysNavigateImmediatelyAfterOpening_data() {
            return [
                {
                    tag: "mouse",
                    keyboard: false
                },
                {
                    tag: "keyboard",
                    keyboard: true
                }
            ];
        }
        function test_arrowKeysNavigateImmediatelyAfterOpening(data) {
            suite.Window.window.requestActivate();
            tryCompare(suite.Window.window, "active", true);
            if (data.keyboard) {
                anchor.forceActiveFocus(Qt.TabFocusReason);
                tryVerify(() => anchor.activeFocus);
                keyClick(Qt.Key_Space);
            } else {
                mouseClick(anchor);
            }
            tryCompare(preview, "opened", true);
            compare(preview.currentIndex, 0);
            keyClick(Qt.Key_Right);
            tryCompare(preview, "currentIndex", 1);
            keyClick(Qt.Key_Right);
            compare(preview.currentIndex, 1);
            keyClick(Qt.Key_Left);
            tryCompare(preview, "currentIndex", 0);
            keyClick(Qt.Key_Left);
            compare(preview.currentIndex, 0);
            keyClick(Qt.Key_Escape);
            tryCompare(preview, "visible", false);
        }
    }
}
