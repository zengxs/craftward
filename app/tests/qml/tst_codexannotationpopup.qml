// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite
    width: 720
    height: 540
    property var entries: []
    property bool automaticHover: false
    Item {
        id: anchorContainer
        width: suite.width
        height: suite.height
        Item {
            id: anchorRow
            width: parent.width
            height: parent.height
            Rectangle {
                id: anchor
                x: 80
                y: 35
                width: 100
                height: 28
                color: "#dddddd"
                HoverHandler {
                    enabled: suite.automaticHover
                    onHoveredChanged: popup.handle("agent", hovered ? "hover" : "leave", anchor, {
                        index: 1,
                        key: "reference",
                        rect: Qt.rect(0, 0, anchor.width, anchor.height)
                    })
                }
            }
        }
    }
    Pages.CodexAnnotationPopup {
        id: popup
        hoverDelay: 40
        leaveDelay: 60
        resolveAnnotations: (entryId, index) => suite.entries
    }
    TestCase {
        name: "CodexAnnotationPopup"
        when: windowShown

        function hit(index) {
            return {
                index: index,
                key: "reference",
                rect: Qt.rect(0, 0, anchor.width, anchor.height)
            };
        }
        function candidate(inputNumber, text) {
            return {
                index: 1,
                text: text ?? "**Literal selection** :codex-annotation{index=\"7\"}",
                comment: "Please change it",
                sourceEntryId: "user-" + inputNumber,
                inputNumber: inputNumber
            };
        }
        function init() {
            suite.automaticHover = false;
            popup.dismiss();
            anchor.parent = anchorRow;
            anchorRow.parent = anchorContainer;
            anchorContainer.x = 0;
            anchorContainer.y = 0;
            anchorRow.x = 0;
            anchorRow.y = 0;
            anchor.visible = true;
            anchor.x = 80;
            anchor.y = 35;
            anchor.width = 100;
            suite.entries = [candidate(1)];
            mouseMove(suite, 1, suite.height - 1);
            // Allow native window moves and closures from the previous case to settle.
            wait(50);
        }
        function cleanup() {
            popup.dismiss();
        }

        function test_collectionAndAmbiguousReferencesShareLiteralDetails() {
            suite.entries = [candidate(3), candidate(1)];
            popup.handle("agent", "activate", anchor, hit(1));
            tryCompare(popup, "opened", true);
            compare(popup.heading, "Annotation 1 · 2 possible sources");
            compare(popup.candidates[0].inputNumber, 3);
            const selected = findChild(popup.contentItem, "codexAnnotationSelection");
            verify(selected !== null);
            compare(selected.textFormat, TextEdit.PlainText);
            selected.selectAll();
            compare(selected.selectedText, suite.entries[0].text);
            suite.entries = [candidate(3), candidate(1)];
            popup.refresh();
            wait(0);
            compare(findChild(popup.contentItem, "codexAnnotationSelection"), selected);
            compare(selected.selectedText, suite.entries[0].text);
            keyClick(Qt.Key_Escape);
            tryCompare(popup, "visible", false);
            popup.handle("user-1", "activate", anchor, hit(0));
            tryCompare(popup, "opened", true);
            compare(popup.heading, "2 annotations");
        }

        function test_popupUsesASeparateWindowAndCanExtendPastItsOwner() {
            const owner = suite.Window.window;
            const previousWidth = owner.width;
            const previousHeight = owner.height;
            try {
                owner.width = 300;
                owner.height = 220;
                anchor.x = 200;
                anchor.y = 150;
                suite.entries = [candidate(1, "A long selection that extends beyond the narrow owner window. ".repeat(2))];
                popup.handle("agent", "activate", anchor, hit(1));
                tryCompare(popup, "opened", true);
                verify(waitForRendering(popup.contentItem));
                compare(popup.popupType, Popup.Window);
                verify(popup.contentItem.Window.window !== owner);
                verify(!(popup.contentItem.Window.window.flags & Qt.NoDropShadowWindowHint));
                verify(popup.width > owner.width);
                const actual = popup.contentItem.parent.mapToGlobal(0, 0);
                const ownerPosition = owner.contentItem.mapToGlobal(0, 0);
                verify(actual.x < ownerPosition.x || actual.x + popup.width > ownerPosition.x + owner.width);
                popup.dismiss();
            } finally {
                owner.width = previousWidth;
                owner.height = previousHeight;
            }
        }

        function test_widthAdaptsToSelectionAndComment_data() {
            return [
                {
                    tag: "selection",
                    field: "text",
                    objectName: "codexAnnotationSelection"
                },
                {
                    tag: "comment",
                    field: "comment",
                    objectName: "codexAnnotationComment"
                }
            ];
        }

        function test_widthAdaptsToSelectionAndComment(data) {
            const shortEntry = candidate(1, "Short selection.");
            shortEntry.comment = "OK";
            suite.entries = [shortEntry];
            popup.handle("agent", "activate", anchor, hit(1));
            tryCompare(popup, "opened", true);
            verify(popup.contentItem.Window.window.width < 300, "The native window should open at the short content width");
            verify(waitForRendering(popup.contentItem));
            const narrowWidth = popup.width;
            const narrowHeight = popup.height;
            verify(narrowWidth < 300, "Short content should not fill a wide panel");
            const shortDetail = findChild(popup.contentItem, data.objectName);
            const lineHeight = shortDetail.height;
            verify(shortDetail.contentWidth <= shortDetail.width + 1);

            const longEntry = candidate(1, shortEntry.text);
            longEntry.comment = shortEntry.comment;
            longEntry[data.field] = "A long passage with 中文内容 and emoji 👨‍👩‍👧‍👦. ".repeat(50);
            suite.entries = [longEntry];
            popup.refresh();
            tryVerify(() => popup.width > narrowWidth);
            verify(popup.width <= 520);
            const longDetail = findChild(popup.contentItem, data.objectName);
            tryVerify(() => longDetail.height > lineHeight);
            verify(longDetail.contentWidth <= longDetail.width + 1, "Long text must wrap without horizontal clipping");
            verify(popup.height <= 480);

            suite.entries = [shortEntry];
            popup.refresh();
            tryCompare(popup, "width", narrowWidth);
            tryCompare(popup, "height", narrowHeight);
        }

        function test_explicitLineBreaksKeepShortPassagesNarrow() {
            suite.entries = [candidate(1, "Short selection.")];
            popup.handle("agent", "activate", anchor, hit(1));
            tryCompare(popup, "opened", true);
            verify(waitForRendering(popup.contentItem));
            const narrowWidth = popup.width;
            const shortHeight = popup.height;

            suite.entries = [candidate(1, "Short selection.\n".repeat(70))];
            popup.refresh();
            tryVerify(() => popup.height > shortHeight);
            compare(popup.width, narrowWidth);
            verify(popup.height <= 480);
            const selected = findChild(popup.contentItem, "codexAnnotationSelection");
            verify(selected.height > popup.height);
        }

        function test_ownerMovementDismissesThePopup() {
            const owner = suite.Window.window;
            const previousX = owner.x;
            popup.handle("agent", "activate", anchor, hit(1));
            tryCompare(popup, "opened", true);
            owner.x += 10;
            tryCompare(popup, "visible", false);
            owner.x = previousX;
        }

        function test_anchorMovementDismissesThePopup_data() {
            return [
                {
                    tag: "anchor-x",
                    item: anchor,
                    axis: "x"
                },
                {
                    tag: "anchor-y",
                    item: anchor,
                    axis: "y"
                },
                {
                    tag: "parent-y",
                    item: anchorRow,
                    axis: "y"
                },
                {
                    tag: "ancestor-x",
                    item: anchorContainer,
                    axis: "x"
                }
            ];
        }

        function test_anchorMovementDismissesThePopup(data) {
            popup.handle("agent", "activate", anchor, hit(1));
            tryCompare(popup, "opened", true);
            // A row can move during layout without changing the anchor's size or scroll offset.
            data.item[data.axis] += 40;
            tryCompare(popup, "visible", false);
        }

        function test_anchorMovementCancelsPendingHover() {
            popup.handle("agent", "hover", anchor, hit(1));
            verify(!popup.visible);
            anchorRow.y += 40;
            wait(popup.hoverDelay + 100);
            verify(!popup.visible);
        }

        function test_reparentingAnAncestorDismissesThePopup() {
            popup.handle("agent", "activate", anchor, hit(1));
            tryCompare(popup, "opened", true);
            anchorRow.parent = suite;
            tryCompare(popup, "visible", false);
        }

        function test_switchingAnchorsStopsWatchingThePreviousAnchor() {
            popup.handle("agent", "activate", anchor, hit(1));
            tryCompare(popup, "opened", true);
            popup.handle("other-agent", "activate", anchorRow, hit(1));
            tryCompare(popup, "opened", true);
            anchor.y += 40;
            wait(50);
            verify(popup.visible);
            anchorRow.y += 40;
            tryCompare(popup, "visible", false);
        }

        function test_reopeningTracksTheCurrentAncestors() {
            popup.handle("agent", "activate", anchor, hit(1));
            tryCompare(popup, "opened", true);
            popup.close();
            tryCompare(popup, "visible", false);
            anchor.parent = anchorContainer;
            popup.handle("agent", "hover", anchor, hit(1));
            tryCompare(popup, "opened", true);
            anchorRow.y += 40;
            wait(50);
            verify(popup.visible);
            anchorContainer.x += 40;
            tryCompare(popup, "visible", false);
        }

        function test_detailsCanBeSelectedWithTheMouseAndClosedOutside() {
            suite.entries = [candidate(1, "Select this annotation detail text.")];
            popup.handle("agent", "activate", anchor, hit(1));
            tryCompare(popup, "opened", true);
            wait(0);
            const selected = findChild(popup.contentItem, "codexAnnotationSelection");
            verify(selected.Window.window !== null);
            mousePress(selected, 2, selected.height / 2);
            verify(popup.visible, "The text press must keep the popup open");
            mouseMove(selected, 102, selected.height / 2, 20, Qt.LeftButton);
            verify(popup.visible, "Dragging text must keep the popup open");
            mouseRelease(selected, 102, selected.height / 2);
            verify(selected.selectedText.length > 0);
            verify(popup.visible);
            mouseClick(popup.contentItem, 1, popup.height + 20);
            tryCompare(popup, "visible", false);
        }

        function test_longDetailsOpenAboveAnAnchorNearTheScreenBottom() {
            const owner = suite.Window.window;
            const previousY = owner.y;
            try {
                owner.y = suite.Screen.virtualY + suite.Screen.height - owner.height - 80;
                wait(50);
                anchor.y = suite.height - anchor.height;
                suite.entries = [candidate(1, "A long selected passage.\n".repeat(70))];
                popup.handle("agent", "activate", anchor, hit(1));
                tryCompare(popup, "opened", true);
                const popupPosition = popup.contentItem.parent.mapToGlobal(0, 0);
                const anchorPosition = anchor.mapToGlobal(0, 0);
                verify(popupPosition.y + popup.height <= anchorPosition.y - 7);
                verify(popupPosition.y >= suite.Screen.virtualY);
            } finally {
                popup.dismiss();
                owner.y = previousY;
            }
        }

        function test_missingReferenceHasAnExplicitFallback() {
            suite.entries = [];
            popup.handle("agent", "activate", anchor, hit(19));
            tryCompare(popup, "opened", true);
            compare(popup.heading, "Annotation 19");
            verify(findChild(popup.contentItem, "codexAnnotationMissing").visible);
        }

        function test_hoverStaysOpenWhileThePointerRemainsOnTheAnchor() {
            suite.automaticHover = true;
            mouseMove(anchor, 30, 10);
            tryCompare(popup, "opened", true);
            wait(popup.leaveDelay + 100);
            verify(popup.visible);
            mouseMove(popup.contentItem, 15, 15);
            wait(popup.leaveDelay + 100);
            verify(popup.visible);
            mouseMove(popup.contentItem, 1, popup.height + 20);
            tryCompare(popup, "visible", false);
        }

        function test_hoverDelayAndGraceAllowEnteringThePopup() {
            popup.handle("agent", "hover", anchor, hit(1));
            verify(!popup.visible);
            tryCompare(popup, "opened", true);
            mouseMove(popup.contentItem, 12, 12);
            tryCompare(findChild(popup.contentItem, "codexAnnotationPopupHover"), "hovered", true);
            popup.handle("agent", "leave", anchor, ({}));
            wait(popup.leaveDelay + 40);
            verify(popup.visible);
            mouseMove(popup.contentItem, 1, popup.height + 20);
            tryCompare(popup, "visible", false);
        }

        function test_longDetailsStayOnScreenAndCloseWhenAnchorIsRecycled() {
            suite.entries = [candidate(3, "A long selected passage.\n".repeat(70)), candidate(1)];
            anchor.x = suite.width - anchor.width;
            anchor.y = suite.height - anchor.height;
            popup.handle("agent", "activate", anchor, hit(1));
            tryCompare(popup, "opened", true);
            const position = popup.contentItem.parent.mapToGlobal(0, 0);
            const screen = anchor.Screen;
            verify(position.x >= screen.virtualX && position.x + popup.width <= screen.virtualX + screen.width);
            verify(position.y >= screen.virtualY && position.y + popup.height <= screen.virtualY + screen.height);
            verify(popup.height <= 480);
            popup.handle("", "invalidate", suite, ({}));
            tryCompare(popup, "visible", false);
            popup.handle("agent", "hover", anchor, hit(1));
            anchor.visible = false;
            wait(popup.hoverDelay + 20);
            verify(!popup.visible);
        }
    }
}
