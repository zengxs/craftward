// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

TestCase {
    name: "ThreadWorkspaceState"
    property var workspace
    Component {
        id: factory
        Pages.ThreadWorkspaceState {}
    }

    function init() {
        workspace = createTemporaryObject(factory, this);
    }
    function file(id, external = false) {
        return {
            id: id,
            kind: "file",
            title: id,
            external: external
        };
    }

    function test_conversationCannotBeClosedOrMoved() {
        workspace.openTab(file("a"));
        workspace.closeTab(0);
        workspace.moveTab(0, 1);
        workspace.moveTab(1, 0);
        compare(workspace.tabs.length, 1);
        compare(workspace.tabs[0].id, "a");
        workspace.activeIndex = 0;
        compare(workspace.activeTab, null);
    }

    function test_switchingThreadsRestoresTheirOwnTabsAndSelection() {
        workspace.selectThread("thread-a");
        workspace.openTab(file("project-file"));
        workspace.openTab(file("external-file", true));
        workspace.filesExpanded = false;
        workspace.selectThread("thread-b");
        compare(workspace.tabs.length, 0);
        compare(workspace.activeIndex, 0);
        verify(workspace.filesExpanded);
        workspace.openTab(file("other-file"));
        workspace.selectThread("thread-a");
        compare(workspace.tabs.length, 2);
        compare(workspace.activeTab.id, "external-file");
        verify(workspace.activeTab.external);
        verify(!workspace.filesExpanded);
        compare(workspace.threadId, "thread-a");
    }

    function test_reopeningFileSelectsExistingTab() {
        workspace.openTab(file("a"));
        workspace.openTab(file("b"));
        workspace.openTab(file("a"));
        compare(workspace.tabs.length, 2);
        compare(workspace.activeIndex, 1);
    }

    function test_movingAndClosingTabsKeepsContentSelection() {
        workspace.openTab(file("a"));
        workspace.openTab(file("b"));
        workspace.openTab(file("c"));
        workspace.moveTab(3, 1);
        compare(workspace.activeTab.id, "c");
        compare(workspace.activeIndex, 1);
        workspace.closeTab(3);
        compare(workspace.activeTab.id, "c");
        workspace.closeTab(1);
        compare(workspace.activeIndex, 0);
        compare(workspace.activeTab, null);
    }

    function test_openingExternalFileDoesNotChangeThread() {
        workspace.selectThread("thread-a");
        workspace.openTab(file("external-file", true));
        compare(workspace.threadId, "thread-a");
        workspace.closeTab(1);
        compare(workspace.threadId, "thread-a");
        compare(workspace.activeIndex, 0);
    }
}
