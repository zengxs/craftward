// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Window
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 800
    height: 600
    property var viewport
    property var scrollViewport
    property var frameReferenceAnchor
    property bool frameSampling: false
    property real maximumFrameAnchorExcursion: 0
    property int sampledFrameCount: 0
    property var materializationStartedRows: []
    property int movementExpandedRow: -1

    Connections {
        target: suite.Window.window

        function onAfterAnimating() {
            if (!suite.frameSampling || !suite.viewport || !suite.scrollViewport || !suite.frameReferenceAnchor)
                return;
            const row = suite.viewport.indexOfEntryId(suite.frameReferenceAnchor.entryId);
            const slot = row >= 0 ? suite.scrollViewport.itemAtIndex(row) : null;
            if (!slot)
                return;
            const top = slot.mapToItem(suite.scrollViewport.contentItem, 0, 0).y;
            const offset = top - suite.scrollViewport.contentY;
            ++suite.sampledFrameCount;
            suite.maximumFrameAnchorExcursion = Math.max(suite.maximumFrameAnchorExcursion, Math.abs(offset - suite.frameReferenceAnchor.offset));
        }
    }

    QtObject {
        id: fakeTimelineModel

        property int totalRowCount: 0
        property int revision: 0
        property var rowIds: []

        function entryIdAt(sourceRow) {
            if (sourceRow < 0 || sourceRow >= totalRowCount)
                return "";
            return rowIds.length === totalRowCount ? String(rowIds[sourceRow]) : "entry:" + sourceRow;
        }

        function valueAt(sourceRow, roleName) {
            return roleName === "entryId" ? entryIdAt(sourceRow) : undefined;
        }
    }

    Component {
        id: rowComponent

        Item {
            property int sourceRow: -1
            property int dataRevision: -1
            property bool interactionState: false
            readonly property string entryId: {
                const currentRevision = dataRevision;
                return currentRevision >= 0 ? fakeTimelineModel.entryIdAt(sourceRow) : "";
            }

            width: parent ? parent.width : 0
            implicitHeight: width >= 400 ? 80 : 160
            height: implicitHeight
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
            estimatedRowHeight: 100
        }
    }

    Component {
        id: delayedRowComponent

        Item {
            property int sourceRow: -1
            property int dataRevision: -1
            property bool contentReady: false

            implicitHeight: contentReady ? 160 : 72

            Timer {
                interval: 50
                running: true
                onTriggered: parent.contentReady = true
            }
        }
    }

    Component {
        id: delayedViewportComponent

        Pages.CodexTimelineViewport {
            width: 600
            height: 400
            timelineModel: fakeTimelineModel
            rowDelegate: delayedRowComponent
            bottomContentInset: 0
            estimatedRowHeight: 72
        }
    }

    Component {
        id: materializingRowComponent

        Item {
            property int sourceRow: -1
            property int dataRevision: -1
            property bool contentMaterializationAllowed: false
            readonly property bool contentMaterializationRequested: true
            property bool contentMaterializationReady: false
            readonly property bool contentMeasurementReady: contentMaterializationReady

            implicitHeight: contentMaterializationReady ? 160 : 72
            onContentMaterializationAllowedChanged: {
                if (!contentMaterializationAllowed || contentMaterializationReady)
                    return;
                suite.materializationStartedRows = suite.materializationStartedRows.concat([sourceRow]);
                materializationTimer.restart();
            }

            Timer {
                id: materializationTimer

                interval: 100
                onTriggered: parent.contentMaterializationReady = true
            }
        }
    }

    Component {
        id: materializingViewportComponent

        Pages.CodexTimelineViewport {
            width: 600
            height: 400
            timelineModel: fakeTimelineModel
            rowDelegate: materializingRowComponent
            bottomContentInset: 0
            estimatedRowHeight: 72
            followLiveTail: false
            contentMaterializationMargin: 0
            maximumConcurrentContentMaterializations: 1
        }
    }

    Component {
        id: movementRowComponent

        Item {
            property int sourceRow: -1
            property int dataRevision: -1

            implicitHeight: sourceRow === suite.movementExpandedRow ? 160 : 72
        }
    }

    Component {
        id: movementViewportComponent

        Pages.CodexTimelineViewport {
            width: 600
            height: 400
            timelineModel: fakeTimelineModel
            rowDelegate: movementRowComponent
            bottomContentInset: 0
            estimatedRowHeight: 72
            followLiveTail: false
        }
    }

    TestCase {
        name: "CodexTimelineViewport"
        when: windowShown

        function createViewport(rowCount) {
            fakeTimelineModel.totalRowCount = rowCount;
            ++fakeTimelineModel.revision;
            suite.viewport = createTemporaryObject(viewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0 || rowCount === 0);
            wait(0);
        }

        function cleanup() {
            suite.frameSampling = false;
            suite.frameReferenceAnchor = null;
            suite.scrollViewport = null;
            suite.viewport = null;
            suite.materializationStartedRows = [];
            suite.movementExpandedRow = -1;
            fakeTimelineModel.totalRowCount = 0;
            fakeTimelineModel.rowIds = [];
            ++fakeTimelineModel.revision;
        }

        function test_materializesOnlyTheViewportNeighborhood() {
            createViewport(1000);

            verify(suite.viewport.scrollContentHeight > suite.viewport.viewportHeight);
            verify(suite.viewport.activeRowSlotCount < 80, "The viewport retained " + suite.viewport.activeRowSlotCount + " row shells");
        }

        function test_positionsWithoutMaterializingTheSkippedRows() {
            createViewport(1000);
            suite.viewport.followLiveTail = false;

            suite.viewport.positionAtContentY(10000);

            tryVerify(() => Math.abs(suite.viewport.contentY - 10000) <= 1);
            verify(suite.viewport.activeRowSlotCount < 80, "The viewport retained " + suite.viewport.activeRowSlotCount + " row shells");
            verify(suite.viewport.captureVisibleAnchor() !== null);
        }

        function test_recreatesADelegateWhenItsEntryMappingChanges() {
            fakeTimelineModel.rowIds = ["entry:a", "entry:b", "entry:c"];
            createViewport(3);
            tryVerify(() => suite.viewport.delegateForEntry("entry:b") !== null);
            const previousDelegate = suite.viewport.delegateForEntry("entry:b");
            previousDelegate.interactionState = true;

            fakeTimelineModel.rowIds = ["entry:a", "entry:inserted", "entry:b", "entry:c"];
            fakeTimelineModel.totalRowCount = 4;
            ++fakeTimelineModel.revision;

            tryVerify(() => {
                const insertedDelegate = suite.viewport.delegateForEntry("entry:inserted");
                return insertedDelegate !== null && insertedDelegate !== previousDelegate;
            });
            verify(!suite.viewport.delegateForEntry("entry:inserted").interactionState);
            tryVerify(() => suite.viewport.delegateForEntry("entry:b") !== null);
        }

        function test_preservesTheVisibleAnchorWhenRowsAreInsertedBeforeIt() {
            const rowIds = [];
            for (let row = 0; row < 100; ++row)
                rowIds.push("entry:" + row);
            fakeTimelineModel.rowIds = rowIds;
            createViewport(rowIds.length);
            suite.viewport.followLiveTail = false;
            suite.viewport.positionAtContentY(2000);
            tryVerify(() => suite.viewport.captureVisibleAnchor() !== null);
            const anchorBeforeInsertion = suite.viewport.captureVisibleAnchor();

            fakeTimelineModel.rowIds = ["entry:inserted"].concat(rowIds);
            fakeTimelineModel.totalRowCount = rowIds.length + 1;
            ++fakeTimelineModel.revision;
            suite.viewport.scheduleAnchorRestore(anchorBeforeInsertion);

            tryVerify(() => {
                const anchor = suite.viewport.captureVisibleAnchor();
                return anchor !== null && anchor.entryId === anchorBeforeInsertion.entryId && Math.abs(anchor.offset - anchorBeforeInsertion.offset) <= 1;
            });
        }

        function test_reusedShellDoesNotLeakDelegateInteractionState() {
            createViewport(1000);
            suite.viewport.followLiveTail = false;
            suite.viewport.positionAtContentY(0);
            tryVerify(() => suite.viewport.delegateForEntry("entry:0") !== null);
            suite.viewport.delegateForEntry("entry:0").interactionState = true;

            suite.viewport.positionAtContentY(10000);
            tryVerify(() => suite.viewport.captureVisibleAnchor() !== null);
            const distantEntryId = suite.viewport.captureVisibleAnchor().entryId;
            verify(distantEntryId !== "entry:0");
            tryVerify(() => suite.viewport.delegateForEntry(distantEntryId) !== null);
            verify(!suite.viewport.delegateForEntry(distantEntryId).interactionState);
        }

        function test_preservesTheVisibleAnchorAcrossBidirectionalReflow() {
            createViewport(100);
            suite.viewport.followLiveTail = false;
            suite.viewport.positionAtContentY(2000);
            tryVerify(() => suite.viewport.captureVisibleAnchor() !== null);

            const wideAnchor = suite.viewport.captureVisibleAnchor();
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

        function test_preservesTheVisibleAnchorWhileRowsFinishLoading() {
            fakeTimelineModel.totalRowCount = 6;
            ++fakeTimelineModel.revision;
            suite.viewport = createTemporaryObject(delayedViewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0);

            suite.viewport.followLiveTail = false;
            suite.viewport.positionAtContentY(100);
            tryVerify(() => suite.viewport.captureVisibleAnchor() !== null);
            const anchorBeforeLoading = suite.viewport.captureVisibleAnchor();

            wait(300);

            const anchorAfterLoading = suite.viewport.captureVisibleAnchor();
            verify(anchorAfterLoading !== null);
            compare(anchorAfterLoading.entryId, anchorBeforeLoading.entryId);
            verify(Math.abs(anchorAfterLoading.offset - anchorBeforeLoading.offset) <= 1, "Visible anchor moved from " + anchorBeforeLoading.offset + " to " + anchorAfterLoading.offset);
        }

        function test_neverRendersAStaleAnchorWhileRowsFinishLoading() {
            fakeTimelineModel.totalRowCount = 6;
            ++fakeTimelineModel.revision;
            suite.viewport = createTemporaryObject(delayedViewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0);
            suite.scrollViewport = findChild(suite.viewport, "codexTimelineScrollViewport");
            verify(suite.scrollViewport !== null);

            suite.viewport.followLiveTail = false;
            suite.viewport.positionAtContentY(100);
            tryVerify(() => suite.viewport.captureVisibleAnchor() !== null);
            suite.frameReferenceAnchor = suite.viewport.captureVisibleAnchor();
            suite.maximumFrameAnchorExcursion = 0;
            suite.sampledFrameCount = 0;
            suite.frameSampling = true;

            wait(300);
            suite.frameSampling = false;

            verify(suite.sampledFrameCount > 0);
            verify(suite.maximumFrameAnchorExcursion <= 1, "A rendered frame moved the visible anchor by " + suite.maximumFrameAnchorExcursion + " px");
        }

        function test_heightChangesDuringADragDoNotMoveTheVisibleAnchor() {
            fakeTimelineModel.totalRowCount = 400;
            ++fakeTimelineModel.revision;
            suite.viewport = createTemporaryObject(movementViewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0);
            suite.scrollViewport = findChild(suite.viewport, "codexTimelineScrollViewport");
            verify(suite.scrollViewport !== null);
            suite.viewport.positionAtContentY(6000);
            tryVerify(() => suite.viewport.captureVisibleAnchor() !== null);
            const initialAnchor = suite.viewport.captureVisibleAnchor();
            const expandedRow = suite.viewport.indexOfEntryId(initialAnchor.entryId);
            verify(expandedRow >= 0);
            tryVerify(() => {
                const candidateSlot = suite.scrollViewport.itemAtIndex(expandedRow);
                return candidateSlot !== null && candidateSlot.loadedItem !== null;
            });

            mousePress(suite.viewport, 300, 260);
            mouseMove(suite.viewport, 300, 220, 20, Qt.LeftButton);
            mouseMove(suite.viewport, 300, 140, 20, Qt.LeftButton);
            mouseMove(suite.viewport, 300, 80, 20, Qt.LeftButton);
            tryVerify(() => suite.scrollViewport.dragging);
            const anchor = suite.viewport.captureVisibleAnchor();
            const row = suite.viewport.indexOfEntryId(anchor.entryId);
            const slot = suite.scrollViewport.itemAtIndex(row);
            verify(slot !== null);
            verify(expandedRow < row);
            const offsetBefore = slot.mapToItem(suite.scrollViewport.contentItem, 0, 0).y - suite.scrollViewport.contentY;

            suite.movementExpandedRow = expandedRow;
            wait(50);

            const offsetAfter = slot.mapToItem(suite.scrollViewport.contentItem, 0, 0).y - suite.scrollViewport.contentY;
            mouseRelease(suite.viewport, 300, 80);
            verify(Math.abs(offsetAfter - offsetBefore) <= 1, "A height change moved the flicking anchor by " + (offsetAfter - offsetBefore) + " px");
            tryVerify(() => !suite.scrollViewport.moving, 5000);
            verify(suite.viewport.pendingAnchor === null, "Movement end scheduled a corrective anchor pass from the already-settled position");
            verify(!suite.viewport.anchorRestoreRunning, "Movement end left a corrective anchor animation running");
        }

        function test_materializesVisibleDeferredContentOneRowAtATime() {
            fakeTimelineModel.totalRowCount = 100;
            ++fakeTimelineModel.revision;
            suite.materializationStartedRows = [];
            suite.viewport = createTemporaryObject(materializingViewportComponent, suite);
            verify(suite.viewport !== null);

            tryVerify(() => suite.materializationStartedRows.length > 0, 1000);
            verify(suite.materializationStartedRows[0] < 8, "Materialization started outside the visible rows at " + suite.materializationStartedRows[0]);
            verify(suite.viewport.activeContentMaterializationCount <= 1, "The viewport started " + suite.viewport.activeContentMaterializationCount + " materializations concurrently");
            tryVerify(() => suite.materializationStartedRows.length > 1, 1000);
            verify(suite.viewport.activeContentMaterializationCount <= 1, "The viewport started " + suite.viewport.activeContentMaterializationCount + " materializations concurrently");
            wait(700);
            verify(suite.materializationStartedRows.every(row => row < 8), "Materialization escaped the visible neighborhood: " + JSON.stringify(suite.materializationStartedRows));
        }

        function test_followsTheLatestRowOnDemand() {
            createViewport(100);
            suite.viewport.followLiveTail = false;
            suite.viewport.positionAtContentY(0);
            tryVerify(() => suite.viewport.contentY <= 1);

            suite.viewport.followLatest();

            tryVerify(() => suite.viewport.contentY >= suite.viewport.scrollContentHeight - suite.viewport.viewportHeight - 1);
        }

        function test_reservesTheComposerInset() {
            createViewport(1);
            tryVerify(() => suite.viewport.delegateForEntry("entry:0") !== null);
            wait(0);
            const originalHeight = suite.viewport.scrollContentHeight;

            suite.viewport.bottomContentInset = 120;

            tryCompare(suite.viewport, "scrollContentHeight", originalHeight + 120);
        }
    }
}
