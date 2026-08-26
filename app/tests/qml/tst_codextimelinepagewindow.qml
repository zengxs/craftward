// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    property var state

    Component {
        id: stateComponent

        Pages.CodexTimelinePageWindow {}
    }

    TestCase {
        name: "CodexTimelinePageWindow"

        function init() {
            suite.state = createTemporaryObject(stateComponent, suite);
            verify(suite.state !== null);
        }

        function cleanup() {
            suite.state = null;
        }

        function test_expandsTowardMotionBeforeAdvancingTheBoundedWindow() {
            suite.state.pageCount = 10;
            compare(suite.state.firstPage, 7);
            compare(suite.state.lastPage, 9);
            compare(suite.state.activePageCount, 3);

            verify(suite.state.expand(-1));
            compare(suite.state.firstPage, 5);
            compare(suite.state.lastPage, 9);
            compare(suite.state.activePageCount, 5);

            verify(suite.state.advance(-1));
            compare(suite.state.firstPage, 4);
            compare(suite.state.lastPage, 8);
            compare(suite.state.activePageCount, 5);
        }

        function test_compactsAroundTheVisiblePageAfterMotion() {
            suite.state.pageCount = 10;
            suite.state.expand(-1);
            suite.state.advance(-1);

            verify(suite.state.compactAround(6));
            compare(suite.state.firstPage, 5);
            compare(suite.state.lastPage, 7);
            compare(suite.state.activePageCount, 3);

            verify(suite.state.expand(1));
            compare(suite.state.firstPage, 5);
            compare(suite.state.lastPage, 9);
            compare(suite.state.activePageCount, 5);
        }

        function test_repositionsTheMaximumWindowForFastCatchUp() {
            suite.state.pageCount = 100;

            verify(suite.state.setWindowAround(10, suite.state.maximumWindowSize));
            compare(suite.state.firstPage, 8);
            compare(suite.state.lastPage, 12);
            compare(suite.state.activePageCount, 5);
        }

        function test_clampsSmallAndShrinkingHistories() {
            suite.state.pageCount = 2;
            compare(suite.state.firstPage, 0);
            compare(suite.state.lastPage, 1);
            compare(suite.state.activePageCount, 2);
            verify(!suite.state.expand(-1));

            suite.state.pageCount = 10;
            suite.state.resetToLatest();
            suite.state.expand(-1);
            suite.state.pageCount = 4;
            compare(suite.state.firstPage, 0);
            compare(suite.state.lastPage, 3);
            compare(suite.state.activePageCount, 4);
        }
    }
}
