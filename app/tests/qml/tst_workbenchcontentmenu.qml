// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite
    width: 640
    height: 400

    Item {
        id: tabAnchor
        x: 100
        y: 32
        width: 26
        height: 26
    }
    Item {
        id: headerAnchor
        x: 580
        y: 68
        width: 26
        height: 26
    }
    Item {
        id: edgeAnchor
        x: 100
        y: suite.height - height
        width: 26
        height: 26
    }
    Component {
        id: menuFactory
        Pages.WorkbenchContentMenu {
            hasConversation: true
            renameAllowed: true
            archiveAllowed: true
            restoreAllowed: true
        }
    }
    Component {
        id: spyFactory
        SignalSpy {}
    }

    TestCase {
        name: "WorkbenchContentMenu"
        when: windowShown
        property var menu

        function init() {
            failOnWarning(/TypeError|ReferenceError|Binding loop/);
            menu = createTemporaryObject(menuFactory, tabAnchor);
            verify(menu !== null);
        }

        function cleanup() {
            menu.close();
            tryVerify(() => !menu.visible);
        }

        function openAt(anchor) {
            menu.popup(anchor, 0, anchor.height);
            tryVerify(() => menu.opened);
            menu.contentItem.forceLayout();
        }

        function test_noBlankRows_data() {
            return [
                {
                    tag: "conversation",
                    fileActive: false,
                    archived: false,
                    hasConversation: true,
                    count: 3
                },
                {
                    tag: "file",
                    fileActive: true,
                    archived: false,
                    hasConversation: true,
                    count: 3
                },
                {
                    tag: "archived-conversation",
                    fileActive: false,
                    archived: true,
                    hasConversation: true,
                    count: 2
                },
                {
                    tag: "no-conversation",
                    fileActive: false,
                    archived: false,
                    hasConversation: false,
                    count: 2
                }
            ];
        }

        function test_noBlankRows(data) {
            menu.fileActive = data.fileActive;
            menu.archived = data.archived;
            menu.hasConversation = data.hasConversation;
            // Keep layout assertions available on platforms without popup windows.
            menu.popupType = Popup.Item;
            openAt(tabAnchor);

            const rows = [];
            for (let i = 0; i < menu.count; ++i) {
                if (menu.itemAt(i).visible)
                    rows.push(menu.itemAt(i));
            }
            compare(rows.length, data.count);
            compare(rows[0].y, 0);
            for (let i = 1; i < rows.length; ++i)
                compare(rows[i].y, rows[i - 1].y + rows[i - 1].height + menu.contentItem.spacing, "Unexpected blank space before " + rows[i].text);
            const last = rows[rows.length - 1];
            compare(menu.contentItem.contentHeight, last.y + last.height, "Unexpected blank space below the last action");
            compare(menu.count, data.count);
        }

        function test_usesSeparateWindowForBothAnchors() {
            compare(menu.popupType, Popup.Window);
            for (const anchor of [tabAnchor, headerAnchor]) {
                openAt(anchor);
                compare(menu.parent, anchor);
                const popupWindow = menu.contentItem.Window.window;
                verify(popupWindow !== null);
                verify(popupWindow !== suite.Window.window);
                menu.close();
                tryVerify(() => !menu.visible);
            }
        }

        function test_canExtendBelowTheHostWindow() {
            openAt(edgeAnchor);
            const bottom = menu.contentItem.mapToGlobal(0, menu.contentItem.height).y;
            verify(bottom > suite.mapToGlobal(0, suite.height).y);
        }

        function test_actionsTriggerAndDismiss_data() {
            return [
                {
                    tag: "open-file",
                    fileActive: false,
                    archived: false,
                    index: 0,
                    signal: "openFileRequested"
                },
                {
                    tag: "open-external",
                    fileActive: true,
                    archived: false,
                    index: 1,
                    signal: "openExternalRequested"
                },
                {
                    tag: "refresh",
                    fileActive: true,
                    archived: false,
                    index: 2,
                    signal: "refreshRequested"
                },
                {
                    tag: "rename",
                    fileActive: false,
                    archived: false,
                    index: 1,
                    signal: "renameRequested"
                },
                {
                    tag: "archive",
                    fileActive: false,
                    archived: false,
                    index: 2,
                    signal: "archiveRequested"
                },
                {
                    tag: "restore",
                    fileActive: false,
                    archived: true,
                    index: 1,
                    signal: "restoreRequested"
                }
            ];
        }

        function test_actionsTriggerAndDismiss(data) {
            menu.fileActive = data.fileActive;
            menu.archived = data.archived;
            const spy = createTemporaryObject(spyFactory, suite, {
                target: menu,
                signalName: data.signal
            });
            verify(spy.valid);
            openAt(tabAnchor);
            mouseClick(menu.itemAt(data.index));
            compare(spy.count, 1);
            tryVerify(() => !menu.visible);
        }

        function test_reopeningAfterContextChangesReplacesActions() {
            menu.fileActive = true;
            openAt(headerAnchor);
            compare(menu.count, 3);
            compare(menu.itemAt(1).text, qsTrId("craftward.file.open_external"));
            menu.close();
            tryVerify(() => !menu.visible);

            menu.fileActive = false;
            openAt(tabAnchor);
            compare(menu.count, 3);
            compare(menu.itemAt(1).text, qsTrId("craftward.action.rename_ellipsis"));
            menu.renameAllowed = false;
            compare(menu.itemAt(1).enabled, false);
            menu.close();
            tryVerify(() => !menu.visible);

            menu.archived = true;
            openAt(tabAnchor);
            compare(menu.count, 2);
            compare(menu.itemAt(1).text, qsTrId("craftward.action.restore"));
            menu.restoreAllowed = false;
            compare(menu.itemAt(1).enabled, false);
        }
    }
}
