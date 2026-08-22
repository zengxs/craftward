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

        Pages.CodexHistoryLayoutState {}
    }

    TestCase {
        name: "CodexHistoryLayoutState"

        function init() {
            suite.state = stateComponent.createObject(suite);
            verify(suite.state !== null);
        }

        function cleanup() {
            suite.state.destroy();
            suite.state = null;
        }

        function test_expandedSidebarDefinesNavigationAndBodyWidth() {
            compare(suite.state.navigationChromeWidth, 310);
            compare(suite.state.bodySidebarWidth, 310);

            suite.state.rememberSidebarWidth(360);
            compare(suite.state.navigationChromeWidth, 360);
            compare(suite.state.bodySidebarWidth, 360);
        }

        function test_collapsedSidebarKeepsOnlyTitleBarNavigationChrome() {
            suite.state.titleBarLeadingInset = 80;
            suite.state.sidebarExpanded = false;

            compare(suite.state.navigationChromeWidth, 164);
            compare(suite.state.bodySidebarWidth, 0);
            compare(suite.state.sidebarWidth, 310);
            compare(suite.state.collapsedSidebarToggleX, 80);
            compare(suite.state.leadingActionsX, 108);
        }

        function test_expandedTitleBarActionsStartAfterLeadingInset() {
            suite.state.titleBarLeadingInset = 78;

            compare(suite.state.leadingActionsX, 78);
        }

        function test_sidebarWidthIsClampedAndRememberedAcrossToggles() {
            suite.state.rememberSidebarWidth(100);
            compare(suite.state.sidebarWidth, 240);

            suite.state.rememberSidebarWidth(500);
            compare(suite.state.sidebarWidth, 420);

            suite.state.toggleSidebar();
            compare(suite.state.bodySidebarWidth, 0);
            suite.state.toggleSidebar();
            compare(suite.state.bodySidebarWidth, 420);
        }
    }
}
