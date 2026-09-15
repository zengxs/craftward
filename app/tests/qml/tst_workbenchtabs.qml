// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite
    width: 800
    height: 100
    Component {
        id: workspaceFactory
        Pages.ThreadWorkspaceState {}
    }
    Component {
        id: tabsFactory
        Pages.WorkbenchTabs {
            width: 480
        }
    }
    TestCase {
        name: "WorkbenchTabs"
        when: windowShown
        property var workspace
        property var strip
        function init() {
            workspace = createTemporaryObject(workspaceFactory, suite);
            strip = createTemporaryObject(tabsFactory, suite, {
                workspace: workspace
            });
            verify(strip !== null);
            failOnWarning(/TypeError|ReferenceError|Binding loop/);
        }
        function test_conversationRemainsVisibleWhenOtherTabsOverflow() {
            for (let i = 0; i < 12; ++i)
                workspace.openTab({
                    id: "file-" + i,
                    title: "File " + i,
                    path: "/project/" + i
                });
            const fixed = findChild(strip, "conversationTab");
            const additional = findChild(strip, "additionalTabs");
            tryVerify(() => additional.contentX > 0);
            compare(fixed.x, 0);
            compare(fixed.width, 150);
            verify(fixed.visible);
            mouseClick(fixed, fixed.width / 2, fixed.height / 2);
            compare(workspace.activeIndex, 0);
        }

        function test_draggingDocumentTabsKeepsConversationFixed() {
            strip.width = 800;
            for (const name of ["a", "b", "c"])
                workspace.openTab({
                    id: name,
                    title: name,
                    path: "/project/" + name
                });
            const additional = findChild(strip, "additionalTabs");
            tryVerify(() => additional.itemAtIndex(0) !== null);
            const first = additional.itemAtIndex(0);
            mouseDrag(first, first.width / 2, 16, first.width, 0);
            compare(workspace.tabs[0].id, "b");
            compare(workspace.tabs[1].id, "a");
            compare(workspace.activeTab.id, "c");
            compare(findChild(strip, "conversationTab").x, 0);
        }

        function test_modelChangesKeepSelectedOverflowTabVisible_data() {
            return [
                {
                    tag: "move-inactive",
                    operation: "move",
                    selectedIndex: 12,
                    neighborIndex: 11
                },
                {
                    tag: "close-inactive",
                    operation: "close",
                    selectedIndex: 11,
                    neighborIndex: 12
                },
                {
                    tag: "refresh-selected",
                    operation: "refresh",
                    selectedIndex: 12,
                    neighborIndex: 11
                }
            ];
        }

        function test_modelChangesKeepSelectedOverflowTabVisible(data) {
            strip.width = 700;
            for (let i = 0; i < 12; ++i)
                workspace.openTab({
                    id: "file-" + i,
                    title: "File " + i,
                    path: "/project/" + i
                });
            workspace.activeIndex = data.selectedIndex;
            const selectedId = workspace.activeTab.id;
            const additional = findChild(strip, "additionalTabs");
            function tabIsVisible(index) {
                const item = additional.itemAtIndex(index - 1);
                return item !== null && item.x >= additional.contentX - 0.5 && item.x + item.width <= additional.contentX + additional.width + 0.5;
            }
            verify(waitForRendering(strip));
            additional.positionViewAtEnd();
            verify(waitForRendering(strip));
            tryVerify(() => tabIsVisible(data.selectedIndex) && tabIsVisible(data.neighborIndex));
            verify(additional.contentX > 0);

            if (data.operation === "move")
                workspace.moveTab(data.neighborIndex, data.neighborIndex - 1);
            else if (data.operation === "close")
                workspace.closeTab(data.neighborIndex);
            else
                workspace.openTab({
                    id: selectedId,
                    title: "Refreshed file",
                    path: "/project/" + (data.selectedIndex - 1)
                });

            verify(waitForRendering(strip));
            tryVerify(() => tabIsVisible(data.selectedIndex), 1500, "Replacing the tab model must keep the selected tab visible");
            compare(workspace.activeTab.id, selectedId);
            compare(additional.currentIndex, data.selectedIndex - 1);
        }

        function test_closeButtonDoesNotAlsoActivateAnotherTab() {
            workspace.openTab({
                id: "a",
                title: "a",
                path: "/project/a"
            });
            workspace.openTab({
                id: "b",
                title: "b",
                path: "/project/b"
            });
            workspace.activeIndex = 1;
            const additional = findChild(strip, "additionalTabs");
            tryVerify(() => additional.itemAtIndex(0) !== null);
            const close = findChild(additional.itemAtIndex(0), "closePanelTab");
            mouseClick(close);
            compare(workspace.tabs.length, 1);
            compare(workspace.tabs[0].id, "b");
            compare(workspace.activeIndex, 0);
        }
    }
}
