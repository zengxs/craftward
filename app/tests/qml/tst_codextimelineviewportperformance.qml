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
    property int createdRowCount: 0
    property int rowGeometryChangeCount: 0
    readonly property var conversationRowGroups: [683, 614, 245, 166, 362, 123]

    Connections {
        target: suite.viewport ?? null

        function onRowGeometryChanged() {
            ++suite.rowGeometryChangeCount;
        }
    }

    ListModel {
        id: fakeTimelineModel

        property int revision: 1
        readonly property int totalRowCount: count

        function resetRows() {
            clear();
            const rowCount = suite.conversationRowGroups.reduce((sum, rows) => sum + rows, 0);
            for (let row = 0; row < rowCount; ++row) {
                append({
                    entryId: "entry:" + row
                });
            }
            ++revision;
        }

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

        function indexOfEntryId(entryId) {
            const prefix = "entry:";
            const value = String(entryId);
            if (!value.startsWith(prefix))
                return -1;
            const row = Number(value.slice(prefix.length));
            return Number.isInteger(row) && row >= 0 && row < count ? row : -1;
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
            Component.onCompleted: ++suite.createdRowCount
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
            Component.onCompleted: ++suite.createdRowCount
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
            fakeTimelineModel.clear();
            ++fakeTimelineModel.revision;
            wait(0);
            suite.createdRowCount = 0;
            suite.rowGeometryChangeCount = 0;
        }

        function test_heightReflowKeepsWorkBoundedAndPreservesAnchor() {
            fakeTimelineModel.resetRows();
            suite.viewport = createTemporaryObject(resizingViewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 10, 10000);
            suite.viewport.positionAtContentY(10000);
            tryVerify(() => suite.viewport.captureVisibleAnchor() !== null, 10000);
            verify(waitForRendering(suite.viewport));
            const anchor = suite.viewport.captureVisibleAnchor();
            const initialActiveRowCount = suite.viewport.activeRowSlotCount;
            suite.rowGeometryChangeCount = 0;

            suite.tallRows = true;

            // Count synchronous work here; TimelineRenderBenchmark measures frame time.
            verify(suite.rowGeometryChangeCount > 0, "The reflow must update materialized rows");
            verify(suite.rowGeometryChangeCount <= initialActiveRowCount, "The reflow updated more rows than the active neighborhood");
            verify(suite.createdRowCount < 100, "The reflow created " + suite.createdRowCount + " row delegates");
            tryVerify(() => {
                const currentAnchor = suite.viewport.captureVisibleAnchor();
                return currentAnchor !== null && currentAnchor.entryId === anchor.entryId && Math.abs(currentAnchor.offset - anchor.offset) <= 1;
            });
            const visibleRows = suite.viewport.visibleRowOffsetsForBenchmark();
            verify(visibleRows.length > 0);
            verify(visibleRows.every(row => row.height === 160), "Visible rows must use the new height");
        }

        function test_smallScrollKeepsDelegateCreationBounded() {
            fakeTimelineModel.resetRows();
            suite.viewport = createTemporaryObject(viewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0, 10000);
            verify(suite.viewport.activeRowSlotCount < 100, "The viewport instantiated " + suite.viewport.activeRowSlotCount + " row shells");
            tryVerify(() => suite.viewport.contentY > 20, 10000);
            verify(waitForRendering(suite.viewport));

            suite.viewport.followLiveTail = false;
            const initialY = suite.viewport.contentY;
            const targetY = initialY - 20;
            const initialCreatedRowCount = suite.createdRowCount;
            const initialActiveRowCount = suite.viewport.activeRowSlotCount;

            suite.viewport.positionAtContentY(targetY);

            const createdRowCount = suite.createdRowCount - initialCreatedRowCount;
            verify(createdRowCount <= initialActiveRowCount, "The small scroll created " + createdRowCount + " row delegates");
            verify(Math.abs(suite.viewport.contentY - targetY) <= 1, "The requested 20 px movement ended at " + (suite.viewport.contentY - initialY) + " px");
        }
    }
}
