// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 800
    height: 600
    property var viewport
    property bool hideAllRows: false

    QtObject {
        id: fakePageModel

        property int pageCount: 0
        property int revision: 0

        function pageId(page) {
            return "page:" + page;
        }

        function pageFirstRow(page) {
            return page;
        }

        function pageRowCount(page) {
            return page >= 0 && page < pageCount ? 1 : 0;
        }
    }

    Component {
        id: rowComponent

        Item {
            property int sourceRow: -1
            property int dataRevision: -1
            readonly property string entryId: "entry:" + sourceRow
            readonly property bool presentationVisible: !suite.hideAllRows

            width: parent ? parent.width : 0
            implicitHeight: width >= 400 ? 80 : 160
            height: implicitHeight
            visible: presentationVisible
        }
    }

    Component {
        id: viewportComponent

        Pages.CodexTimelineViewport {
            width: 600
            height: 400
            pageModel: fakePageModel
            rowDelegate: rowComponent
            bottomContentInset: 0
            estimatedPageHeight: 300
        }
    }

    TestCase {
        name: "CodexTimelineViewport"
        when: windowShown

        function createViewport(pageCount) {
            fakePageModel.pageCount = pageCount;
            ++fakePageModel.revision;
            suite.viewport = createTemporaryObject(viewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.loadedPageCount > 0 || pageCount === 0);
            wait(0);
        }

        function cleanup() {
            suite.viewport = null;
            suite.hideAllRows = false;
            fakePageModel.pageCount = 0;
            ++fakePageModel.revision;
        }

        function test_reservesReachableSpaceForUnmeasuredPages() {
            createViewport(4);

            compare(suite.viewport.activeFirstPage, 1);
            verify(suite.viewport.leadingPlaceholderHeight > 0);
            verify(suite.viewport.scrollContentHeight > suite.viewport.viewportHeight, "content=" + suite.viewport.scrollContentHeight + ", viewport=" + suite.viewport.viewportHeight);

            suite.viewport.positionAtContentY(0);
            tryCompare(suite.viewport, "activeFirstPage", 0);
        }

        function test_catchesUpWhenFastScrollingIntoAPlaceholder() {
            createViewport(100);
            compare(suite.viewport.loadedPageCount, 3);
            verify(suite.viewport.leadingPlaceholderHeight > 0);

            suite.viewport.positionAtContentY(10 * suite.viewport.estimatedPageHeight);
            tryVerify(() => suite.viewport.activeFirstPage <= 10 && suite.viewport.activeLastPage >= 10);
            tryVerify(() => suite.viewport.captureVisibleAnchor() !== null);
            verify(suite.viewport.loadedPageCount <= 5);
        }

        function test_invalidatesMeasuredPageHeightsAfterWidthChanges() {
            createViewport(6);
            suite.viewport.positionAtContentY(0);
            tryCompare(suite.viewport, "activeFirstPage", 0);
            suite.viewport.followLatest();
            tryCompare(suite.viewport, "activeFirstPage", 3);
            tryVerify(() => suite.viewport.leadingPlaceholderHeight < 900);

            suite.viewport.width = 300;
            tryCompare(suite.viewport, "leadingPlaceholderHeight", 900);
        }

        function test_preservesTheVisibleAnchorAcrossBidirectionalReflow() {
            createViewport(20);
            suite.viewport.positionAtContentY(8 * suite.viewport.estimatedPageHeight);
            tryVerify(() => suite.viewport.activeFirstPage <= 8 && suite.viewport.activeLastPage >= 8);
            tryVerify(() => suite.viewport.captureVisibleAnchor() !== null);

            const wideAnchor = suite.viewport.captureVisibleAnchor();
            verify(wideAnchor !== null, JSON.stringify({
                contentY: suite.viewport.contentY,
                contentHeight: suite.viewport.scrollContentHeight,
                firstPage: suite.viewport.activeFirstPage,
                lastPage: suite.viewport.activeLastPage,
                leadingHeight: suite.viewport.leadingPlaceholderHeight,
                trailingHeight: suite.viewport.trailingPlaceholderHeight
            }));
            suite.viewport.width = 300;
            tryVerify(() => {
                const anchor = suite.viewport.captureVisibleAnchor();
                return anchor !== null && anchor.entryId === wideAnchor.entryId && Math.abs(anchor.offset - wideAnchor.offset) <= 1;
            });

            const narrowAnchor = suite.viewport.captureVisibleAnchor();
            suite.viewport.width = 600;
            tryVerify(() => {
                const anchor = suite.viewport.captureVisibleAnchor();
                return anchor !== null && anchor.entryId === narrowAnchor.entryId && Math.abs(anchor.offset - narrowAnchor.offset) <= 1;
            });
        }

        function test_keepsLoadedPagesBoundedAndReservesComposerInset() {
            createViewport(100);
            verify(suite.viewport.loadedPageCount <= 5);
            const originalHeight = suite.viewport.scrollContentHeight;

            suite.viewport.bottomContentInset = 120;
            tryCompare(suite.viewport, "scrollContentHeight", originalHeight + 120);
        }

        function test_collapsedRowsDoNotLeavePageSpacing() {
            createViewport(1);
            tryCompare(suite.viewport, "scrollContentHeight", 120);

            suite.hideAllRows = true;
            tryCompare(suite.viewport, "scrollContentHeight", 30);

            suite.hideAllRows = false;
            tryCompare(suite.viewport, "scrollContentHeight", 120);
        }
    }
}
