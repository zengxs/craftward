// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

TestCase {
    name: "CodexHistoryLayoutState"
    property var state
    Component {
        id: factory
        Pages.CodexHistoryLayoutState {}
    }

    function init() {
        state = createTemporaryObject(factory, this);
    }

    function test_sidebarWidthSurvivesCollapse() {
        state.rememberSidebarWidth(350);
        state.toggleSidebar();
        compare(state.bodySidebarWidth, 0);
        state.toggleSidebar();
        compare(state.bodySidebarWidth, 350);
    }

    function test_widthsAreClamped() {
        state.rememberSidebarWidth(10);
        compare(state.sidebarWidth, state.minimumSidebarWidth);
        state.rememberSidebarWidth(999);
        compare(state.sidebarWidth, state.maximumSidebarWidth);
        state.rememberFilesWidth(10);
        compare(state.filesWidth, state.minimumFilesWidth);
        state.rememberFilesWidth(999);
        compare(state.filesWidth, state.maximumFilesWidth);
        state.rememberFilesWidth(NaN);
        compare(state.filesWidth, state.maximumFilesWidth);
    }

    function test_narrowWindowHidesFilesWithoutLosingPreference() {
        state.availableWidth = 960;
        verify(state.filesVisible);
        state.availableWidth = 640;
        verify(!state.filesVisible);
        verify(state.filesExpanded);
        state.availableWidth = 960;
        verify(state.filesVisible);
    }

    function test_filesCanReplaceSidebarInNarrowWindow() {
        state.availableWidth = 640;
        state.sidebarExpanded = false;
        verify(state.filesVisible);
        state.filesExpanded = false;
        verify(!state.filesVisible);
    }
}
