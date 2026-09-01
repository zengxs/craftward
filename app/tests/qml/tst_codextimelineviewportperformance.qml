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
    property bool tallRows: false
    readonly property var conversationRowGroups: [683, 614, 245, 166, 362, 123]

    QtObject {
        id: fakeTimelineModel

        property int revision: 1
        readonly property int totalRowCount: suite.conversationRowGroups.reduce((sum, rows) => sum + rows, 0)

        function entryIdAt(sourceRow) {
            return "entry:" + sourceRow;
        }

        function valueAt(sourceRow, roleName) {
            const turn = Math.floor(sourceRow / 100);
            switch (roleName) {
            case "entryId":
                return entryIdAt(sourceRow);
            case "turnId":
                return "turn:" + turn;
            case "turnForkable":
            case "fromUser":
            case "finalAnswer":
                return false;
            case "latestTurn":
                return sourceRow === totalRowCount - 1;
            case "activityGroup":
            case "detailRow":
                return false;
            case "firstDetailInTurn":
                return false;
            case "detailCountInTurn":
                return 0;
            case "standaloneActivity":
                return true;
            case "activityPresentationKind":
                return "activity";
            case "activityLabel":
                return "Activity";
            case "failed":
            case "running":
                return false;
            case "markupDocument":
                return null;
            default:
                return undefined;
            }
        }
    }

    Component {
        id: rowComponent

        Pages.CodexTimelineRow {
            width: parent ? parent.width : 0
            timelineModel: fakeTimelineModel
            turnExpanded: false
            hasRunningEvidence: false
            activityShimmerEnabled: false
            forkEnabled: false
            showForkActions: false
            wallClockUnixMilliseconds: 0
        }
    }

    Component {
        id: viewportComponent

        Pages.CodexTimelineViewport {
            width: 600
            height: 400
            timelineModel: fakeTimelineModel
            rowDelegate: rowComponent
            bottomContentInset: 0
        }
    }

    Component {
        id: resizingRowComponent

        Item {
            property int sourceRow: -1
            property int dataRevision: -1

            implicitHeight: suite.tallRows ? 160 : 72
        }
    }

    Component {
        id: resizingViewportComponent

        Pages.CodexTimelineViewport {
            width: 600
            height: 400
            timelineModel: fakeTimelineModel
            rowDelegate: resizingRowComponent
            bottomContentInset: 0
            estimatedRowHeight: 72
            followLiveTail: false
        }
    }

    TestCase {
        name: "CodexTimelineViewportPerformance"
        when: windowShown

        function cleanup() {
            if (suite.viewport)
                suite.viewport.destroy();
            suite.viewport = null;
            suite.tallRows = false;
            wait(0);
        }

        function test_batchHeightSettlementStaysWithinFrameBudget() {
            suite.viewport = createTemporaryObject(resizingViewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 10, 10000);
            suite.viewport.positionAtContentY(10000);
            tryVerify(() => suite.viewport.captureVisibleAnchor() !== null, 10000);
            wait(100);
            const anchor = suite.viewport.captureVisibleAnchor();
            const startedAt = Date.now();

            suite.tallRows = true;

            const elapsedMilliseconds = Date.now() - startedAt;
            verify(elapsedMilliseconds < Math.ceil(suite.viewport.frameBudgetMilliseconds), "Settling active row heights blocked for " + elapsedMilliseconds + " ms");
            tryVerify(() => {
                const currentAnchor = suite.viewport.captureVisibleAnchor();
                return currentAnchor !== null && currentAnchor.entryId === anchor.entryId && Math.abs(currentAnchor.offset - anchor.offset) <= 1;
            });
        }

        function test_smallScrollDoesNotMaterializeDistantRowsSynchronously() {
            suite.viewport = createTemporaryObject(viewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0, 10000);
            verify(suite.viewport.activeRowSlotCount < 100, "The viewport instantiated " + suite.viewport.activeRowSlotCount + " row shells");
            tryVerify(() => suite.viewport.contentY > 20, 10000);

            suite.viewport.followLiveTail = false;
            const initialY = suite.viewport.contentY;
            const targetY = initialY - 20;
            const startedAt = Date.now();

            suite.viewport.positionAtContentY(targetY);

            const elapsedMilliseconds = Date.now() - startedAt;
            verify(elapsedMilliseconds < Math.ceil(suite.viewport.frameBudgetMilliseconds), "The scroll callback blocked for " + elapsedMilliseconds + " ms");
            verify(Math.abs(suite.viewport.contentY - targetY) <= 1, "The requested 20 px movement ended at " + (suite.viewport.contentY - initialY) + " px");
        }
    }
}
