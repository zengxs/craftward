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
    readonly property real stressFlickVelocity: 32000
    readonly property real stressFlickDeceleration: 16000
    readonly property real minimumStressObservedVelocity: 30000
    readonly property real minimumStressTravel: 24000
    readonly property int minimumStressRowTravel: 100
    property var viewport: null
    property var scrollViewport: null
    property var frameReferenceAnchor
    property bool frameSampling: false
    property real maximumFrameAnchorExcursion: 0
    property int sampledFrameCount: 0
    property var anchorExcursionTrace: []
    property var anchorCorrectionTrace: []
    property var rowGeometryTrace: []
    property int rowGeometryChangeCount: 0
    property bool finalDecelerationSampling: false
    property int finalDecelerationContentDirection: 1
    property int finalDecelerationChangedRowOffset: -1
    property bool finalDecelerationHeightChangeTriggered: false
    property var finalDecelerationAnchor: null
    property real finalDecelerationPreviousContentY: Number.NaN
    property real finalDecelerationPreviousOffset: Number.NaN
    property int finalDecelerationInitialRowHeightRevision: -1
    property real maximumOppositeDirectionMovement: 0
    property int finalDecelerationMissingFrameCount: 0
    property var finalDecelerationTrace: []
    property bool delegateContinuitySampling: false
    property var delegateContinuityEntryIds: []
    property int delegateContinuityFrameCount: 0
    property int minimumLoadedDelegateCount: 0
    property var delegateContinuityTrace: []
    property var materializationStartedRows: []
    property int movementExpandedRow: -1
    property int postFlickChangedRow: -1
    property int postFlickChangedRowCount: 1
    property real postFlickChangedHeight: 220
    property bool postFlickSettlementArmed: false
    property bool postFlickSettlementStarted: false
    property bool deferredMeasurementSampling: false
    property int maximumDeferredRowMeasurementCount: 0
    property int maximumActiveRowSlotCount: 0

    function beginPostFlickHeightSettlement() {
        suite.frameReferenceAnchor = suite.viewport.captureVisibleAnchor();
        suite.maximumFrameAnchorExcursion = 0;
        suite.sampledFrameCount = 0;
        suite.anchorExcursionTrace = [];
        suite.anchorCorrectionTrace = [];
        suite.rowGeometryTrace = [];
        suite.frameSampling = suite.frameReferenceAnchor !== null;
        suite.postFlickChangedRow = suite.frameReferenceAnchor ? suite.viewport.indexOfEntryId(suite.frameReferenceAnchor.entryId) - 1 : -1;
        suite.postFlickSettlementStarted = true;
    }

    Connections {
        target: suite.Window.window

        function onAfterAnimating() {
            if (suite.deferredMeasurementSampling && suite.viewport) {
                suite.maximumDeferredRowMeasurementCount = Math.max(suite.maximumDeferredRowMeasurementCount, suite.viewport.deferredRowMeasurementCount);
                suite.maximumActiveRowSlotCount = Math.max(suite.maximumActiveRowSlotCount, suite.viewport.activeRowSlotCount);
            }
            if (suite.frameSampling && suite.viewport && suite.scrollViewport && suite.frameReferenceAnchor) {
                const row = suite.viewport.indexOfEntryId(suite.frameReferenceAnchor.entryId);
                const slot = row >= 0 ? suite.scrollViewport.itemAtIndex(row) : null;
                if (slot) {
                    const top = slot.mapToItem(suite.scrollViewport.contentItem, 0, 0).y;
                    const offset = top - suite.scrollViewport.contentY;
                    ++suite.sampledFrameCount;
                    suite.maximumFrameAnchorExcursion = Math.max(suite.maximumFrameAnchorExcursion, Math.abs(offset - suite.frameReferenceAnchor.offset));
                    if (suite.anchorExcursionTrace.length < 8) {
                        suite.anchorExcursionTrace = suite.anchorExcursionTrace.concat([
                            {
                                frame: suite.sampledFrameCount,
                                contentY: suite.viewport.contentY,
                                anchorOffset: offset,
                                rowHeightRevision: suite.viewport.rowHeightRevision,
                                anchorSettlementSuppressed: suite.viewport.anchorSettlementSuppressed,
                                anchorRestoreRunning: suite.viewport.anchorRestoreRunning,
                                pendingAnchorEntryId: suite.viewport.pendingAnchor ? suite.viewport.pendingAnchor.entryId : ""
                            }
                        ]);
                    }
                }
            }
            if (suite.delegateContinuitySampling && suite.viewport) {
                const rowStates = suite.delegateContinuityEntryIds.map(entryId => {
                    const row = suite.viewport.indexOfEntryId(entryId);
                    const slot = row >= 0 && suite.scrollViewport ? suite.scrollViewport.itemAtIndex(row) : null;
                    return {
                        entryId: entryId,
                        row: row,
                        slotEntryId: slot ? slot.entryId : "",
                        loadedEntryId: slot ? slot.loadedEntryId : "",
                        loaderGeneration: slot ? slot.loaderGeneration : -1,
                        loaded: slot !== null && slot.loadedItem !== null
                    };
                });
                const loadedEntryIds = rowStates.filter(state => state.loaded).map(state => state.entryId);
                ++suite.delegateContinuityFrameCount;
                suite.minimumLoadedDelegateCount = Math.min(suite.minimumLoadedDelegateCount, loadedEntryIds.length);
                if (suite.delegateContinuityTrace.length < 3) {
                    suite.delegateContinuityTrace = suite.delegateContinuityTrace.concat([
                        {
                            frame: suite.delegateContinuityFrameCount,
                            contentY: suite.viewport.contentY,
                            modelRevision: fakeTimelineModel.revision,
                            modelRowCount: fakeTimelineModel.totalRowCount,
                            rowHeightRevision: suite.viewport.rowHeightRevision,
                            pendingAnchorEntryId: suite.viewport.pendingAnchor ? suite.viewport.pendingAnchor.entryId : "",
                            rowStates: rowStates.map(state => state.entryId + "@" + state.row + ":" + state.slotEntryId + "/" + state.loadedEntryId + "#" + state.loaderGeneration + ":" + state.loaded)
                        }
                    ]);
                }
            }
        }

        function onFrameSwapped() {
            if (suite.finalDecelerationSampling && suite.viewport && suite.scrollViewport) {
                const moving = suite.scrollViewport.moving;
                const velocity = Math.abs(Number(suite.viewport.verticalVelocity));
                if (!suite.finalDecelerationHeightChangeTriggered && moving && velocity > 0 && velocity <= 4000) {
                    const visibleAnchor = suite.viewport.captureVisibleAnchor();
                    const visibleAnchorRow = visibleAnchor ? suite.viewport.indexOfEntryId(visibleAnchor.entryId) : -1;
                    const trackedRow = visibleAnchorRow;
                    const changedRow = visibleAnchorRow + suite.finalDecelerationChangedRowOffset;
                    const trackedEntryId = trackedRow >= 0 ? suite.viewport.entryIdAt(trackedRow) : "";
                    const changedEntryId = changedRow >= 0 ? suite.viewport.entryIdAt(changedRow) : "";
                    const trackedOffset = trackedEntryId.length > 0 ? Number(suite.viewport.anchorOffsetForBenchmark({
                        entryId: trackedEntryId,
                        row: trackedRow
                    })) : Number.NaN;
                    if (Number.isFinite(trackedOffset) && suite.viewport.delegateForEntry(changedEntryId)) {
                        suite.finalDecelerationAnchor = {
                            entryId: trackedEntryId,
                            row: trackedRow
                        };
                        suite.finalDecelerationPreviousContentY = suite.viewport.contentY;
                        suite.finalDecelerationPreviousOffset = trackedOffset;
                        suite.finalDecelerationInitialRowHeightRevision = suite.viewport.rowHeightRevision;
                        suite.finalDecelerationHeightChangeTriggered = true;
                        suite.postFlickChangedRow = changedRow;
                    }
                } else if (suite.finalDecelerationHeightChangeTriggered && suite.finalDecelerationAnchor) {
                    const offset = Number(suite.viewport.anchorOffsetForBenchmark(suite.finalDecelerationAnchor));
                    if (!Number.isFinite(offset)) {
                        ++suite.finalDecelerationMissingFrameCount;
                    } else {
                        const contentY = suite.viewport.contentY;
                        const contentDelta = contentY - suite.finalDecelerationPreviousContentY;
                        const offsetDelta = offset - suite.finalDecelerationPreviousOffset;
                        if (suite.finalDecelerationContentDirection * offsetDelta > 0.1)
                            suite.maximumOppositeDirectionMovement = Math.max(suite.maximumOppositeDirectionMovement, Math.abs(offsetDelta));
                        if (suite.finalDecelerationTrace.length < 32) {
                            suite.finalDecelerationTrace = suite.finalDecelerationTrace.concat([
                                {
                                    contentY: contentY,
                                    contentDelta: contentDelta,
                                    anchorOffset: offset,
                                    offsetDelta: offsetDelta,
                                    moving: moving,
                                    velocity: velocity,
                                    rowHeightRevision: suite.viewport.rowHeightRevision
                                }
                            ]);
                        }
                        suite.finalDecelerationPreviousContentY = contentY;
                        suite.finalDecelerationPreviousOffset = offset;
                    }
                }
            }
        }
    }

    Connections {
        target: suite.viewport
        ignoreUnknownSignals: true

        function onAnchorPositionCorrected(displacement) {
            if (suite.frameSampling)
                suite.anchorCorrectionTrace = suite.anchorCorrectionTrace.concat([displacement]);
        }

        function onRowGeometryChanged(sourceRow, heightDelta) {
            ++suite.rowGeometryChangeCount;
            if (suite.frameSampling && suite.rowGeometryTrace.length < 16)
                suite.rowGeometryTrace = suite.rowGeometryTrace.concat([sourceRow + ":" + heightDelta]);
        }
    }

    Connections {
        target: suite.scrollViewport
        ignoreUnknownSignals: true

        function onMovementEnded() {
            if (!suite.postFlickSettlementArmed)
                return;
            suite.postFlickSettlementArmed = false;
            suite.beginPostFlickHeightSettlement();
        }
    }

    ListModel {
        id: fakeTimelineModel

        property int revision: 0
        readonly property int totalRowCount: count

        function resetRows(rowCount, rowIds) {
            clear();
            const explicitRowIds = rowIds ?? [];
            for (let row = 0; row < rowCount; ++row) {
                append({
                    entryId: explicitRowIds.length === rowCount ? String(explicitRowIds[row]) : "entry:" + row
                });
            }
            ++revision;
        }

        function entryIdAt(sourceRow) {
            if (sourceRow < 0 || sourceRow >= totalRowCount)
                return "";
            return String(get(sourceRow).entryId);
        }

        function valueAt(sourceRow, roleName) {
            return roleName === "entryId" ? entryIdAt(sourceRow) : undefined;
        }

        function indexOfEntryId(entryId) {
            const target = String(entryId);
            for (let row = 0; row < count; ++row) {
                if (entryIdAt(row) === target)
                    return row;
            }
            return -1;
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
        id: variableHeightRowComponent

        Item {
            property int sourceRow: -1
            property int dataRevision: -1

            implicitHeight: [72, 144, 240][Math.abs(sourceRow) % 3]
        }
    }

    Component {
        id: variableHeightViewportComponent

        Pages.CodexTimelineViewport {
            width: 600
            height: 400
            timelineModel: fakeTimelineModel
            rowDelegate: variableHeightRowComponent
            bottomContentInset: 0
            estimatedRowHeight: 72
            followLiveTail: false
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

    Component {
        id: postFlickSettlingRowComponent

        Item {
            property int sourceRow: -1
            property int dataRevision: -1

            implicitHeight: sourceRow <= suite.postFlickChangedRow && sourceRow > suite.postFlickChangedRow - suite.postFlickChangedRowCount ? suite.postFlickChangedHeight : 72
        }
    }

    Component {
        id: postFlickSettlingViewportComponent

        Pages.CodexTimelineViewport {
            width: 600
            height: 400
            timelineModel: fakeTimelineModel
            rowDelegate: postFlickSettlingRowComponent
            bottomContentInset: 0
            estimatedRowHeight: 72
            followLiveTail: false
        }
    }

    TestCase {
        name: "CodexTimelineViewport"
        when: windowShown

        function createViewport(rowCount, rowIds) {
            fakeTimelineModel.resetRows(rowCount, rowIds);
            suite.viewport = createTemporaryObject(viewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0 || rowCount === 0);
            wait(0);
        }

        function cleanup() {
            suite.frameSampling = false;
            suite.frameReferenceAnchor = null;
            suite.anchorExcursionTrace = [];
            suite.anchorCorrectionTrace = [];
            suite.rowGeometryTrace = [];
            suite.rowGeometryChangeCount = 0;
            suite.finalDecelerationSampling = false;
            suite.finalDecelerationContentDirection = 1;
            suite.finalDecelerationChangedRowOffset = -1;
            suite.finalDecelerationHeightChangeTriggered = false;
            suite.finalDecelerationAnchor = null;
            suite.finalDecelerationPreviousContentY = Number.NaN;
            suite.finalDecelerationPreviousOffset = Number.NaN;
            suite.finalDecelerationInitialRowHeightRevision = -1;
            suite.maximumOppositeDirectionMovement = 0;
            suite.finalDecelerationMissingFrameCount = 0;
            suite.finalDecelerationTrace = [];
            suite.delegateContinuitySampling = false;
            suite.delegateContinuityEntryIds = [];
            suite.delegateContinuityFrameCount = 0;
            suite.minimumLoadedDelegateCount = 0;
            suite.delegateContinuityTrace = [];
            suite.scrollViewport = null;
            if (suite.viewport)
                suite.viewport.destroy();
            suite.viewport = null;
            wait(0);
            suite.materializationStartedRows = [];
            suite.movementExpandedRow = -1;
            suite.postFlickChangedRow = -1;
            suite.postFlickChangedRowCount = 1;
            suite.postFlickChangedHeight = 220;
            suite.postFlickSettlementArmed = false;
            suite.postFlickSettlementStarted = false;
            suite.deferredMeasurementSampling = false;
            suite.maximumDeferredRowMeasurementCount = 0;
            suite.maximumActiveRowSlotCount = 0;
            fakeTimelineModel.clear();
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

        function test_restoresAnAnchorSynchronouslyAfterItsShellWasRecycled() {
            createViewport(1000);
            suite.viewport.followLiveTail = false;
            suite.scrollViewport = findChild(suite.viewport, "codexTimelineScrollViewport");
            verify(suite.scrollViewport !== null);
            suite.viewport.positionAtContentY(100);
            const anchor = suite.viewport.captureVisibleAnchor();
            verify(anchor !== null);
            const anchorRow = suite.viewport.indexOfEntryId(anchor.entryId);

            suite.viewport.positionAtContentY(30000);
            tryVerify(() => suite.scrollViewport.itemAtIndex(anchorRow) === null);

            suite.viewport.restoreAnchorAfterLayout(anchor);

            const restoredOffset = Number(suite.viewport.anchorOffsetForBenchmark(anchor));
            verify(Number.isFinite(restoredOffset));
            verify(Math.abs(restoredOffset - anchor.offset) <= 1, "The synchronously reinstantiated anchor moved by " + (restoredOffset - anchor.offset) + " px");
        }

        function test_preservesADelegateWhenRowsAreInsertedBeforeIt() {
            createViewport(3, ["entry:a", "entry:b", "entry:c"]);
            tryVerify(() => suite.viewport.delegateForEntry("entry:b") !== null);
            const previousDelegate = suite.viewport.delegateForEntry("entry:b");
            previousDelegate.interactionState = true;

            fakeTimelineModel.insert(1, {
                entryId: "entry:inserted"
            });
            ++fakeTimelineModel.revision;

            tryVerify(() => {
                const insertedDelegate = suite.viewport.delegateForEntry("entry:inserted");
                return insertedDelegate !== null && insertedDelegate !== previousDelegate;
            });
            verify(!suite.viewport.delegateForEntry("entry:inserted").interactionState);
            compare(suite.viewport.delegateForEntry("entry:b"), previousDelegate);
            verify(suite.viewport.delegateForEntry("entry:b").interactionState);
        }

        function test_preservesTheVisibleAnchorWhenRowsAreInsertedBeforeIt() {
            const rowIds = [];
            for (let row = 0; row < 100; ++row)
                rowIds.push("entry:" + row);
            createViewport(rowIds.length, rowIds);
            suite.viewport.followLiveTail = false;
            suite.viewport.positionAtContentY(2000);
            tryVerify(() => suite.viewport.captureVisibleAnchor() !== null);
            const anchorBeforeInsertion = suite.viewport.captureVisibleAnchor();

            fakeTimelineModel.insert(0, {
                entryId: "entry:inserted"
            });
            ++fakeTimelineModel.revision;
            suite.viewport.scheduleAnchorRestore(anchorBeforeInsertion);

            tryVerify(() => {
                const anchor = suite.viewport.captureVisibleAnchor();
                return anchor !== null && anchor.entryId === anchorBeforeInsertion.entryId && Math.abs(anchor.offset - anchorBeforeInsertion.offset) <= 1;
            });
        }

        function test_keepsVisibleDelegatesLoadedAcrossInsertionBeforeViewport() {
            const rowIds = [];
            for (let row = 0; row < 8; ++row)
                rowIds.push("entry:" + row);
            createViewport(rowIds.length, rowIds);
            suite.viewport.followLiveTail = false;
            suite.viewport.positionAtContentY(250);
            suite.scrollViewport = findChild(suite.viewport, "codexTimelineScrollViewport");
            verify(suite.scrollViewport !== null);
            tryVerify(() => suite.viewport.captureVisibleAnchor() !== null);
            const anchorBeforeInsertion = suite.viewport.captureVisibleAnchor();
            const anchorRow = suite.viewport.indexOfEntryId(anchorBeforeInsertion.entryId);
            const visibleEntryIds = [];
            for (let row = anchorRow; row < Math.min(anchorRow + 4, rowIds.length); ++row)
                visibleEntryIds.push(rowIds[row]);
            tryVerify(() => visibleEntryIds.every(entryId => suite.viewport.delegateForEntry(entryId) !== null));
            const loaderGenerations = {};
            for (const entryId of visibleEntryIds) {
                const row = suite.viewport.indexOfEntryId(entryId);
                loaderGenerations[entryId] = suite.scrollViewport.itemAtIndex(row).loaderGeneration;
            }

            suite.delegateContinuityEntryIds = visibleEntryIds;
            suite.minimumLoadedDelegateCount = visibleEntryIds.length;
            suite.delegateContinuityFrameCount = 0;
            suite.delegateContinuityTrace = [];
            suite.delegateContinuitySampling = true;

            fakeTimelineModel.insert(0, {
                entryId: "entry:inserted"
            });
            ++fakeTimelineModel.revision;

            wait(120);
            suite.delegateContinuitySampling = false;

            verify(suite.delegateContinuityFrameCount > 0);
            compare(suite.minimumLoadedDelegateCount, visibleEntryIds.length, "Visible delegate continuity trace: " + JSON.stringify(suite.delegateContinuityTrace));
            for (const entryId of visibleEntryIds) {
                const row = suite.viewport.indexOfEntryId(entryId);
                compare(suite.scrollViewport.itemAtIndex(row).loaderGeneration, loaderGenerations[entryId], "The unchanged delegate was rebuilt for " + entryId);
            }
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

        function test_reusingWarmRowsDoesNotReportSemanticGeometryChanges() {
            fakeTimelineModel.resetRows(1000);
            suite.viewport = createTemporaryObject(variableHeightViewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0);
            const knownHeights = [72, 144, 240];
            for (let row = 0; row < 1000; ++row)
                suite.viewport.rememberRowHeight("entry:" + row, knownHeights[row % knownHeights.length]);

            suite.viewport.positionAtContentY(20000);
            wait(300);
            suite.viewport.positionAtContentY(0);
            wait(300);

            suite.rowGeometryTrace = [];
            suite.frameSampling = true;
            suite.viewport.positionAtContentY(20000);
            wait(300);
            suite.frameSampling = false;

            compare(suite.rowGeometryTrace.length, 0, "Warm row reuse reported geometry changes: " + JSON.stringify(suite.rowGeometryTrace));
        }

        function test_reportsUncachedEntryMeasurementAsGeometryChange() {
            fakeTimelineModel.resetRows(3);
            suite.rowGeometryTrace = [];
            suite.frameSampling = true;
            suite.viewport = createTemporaryObject(variableHeightViewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0);

            wait(300);
            suite.frameSampling = false;

            verify(suite.rowGeometryTrace.includes("1:72"), "Missing the uncached row measurement: " + JSON.stringify(suite.rowGeometryTrace));
            verify(suite.rowGeometryTrace.includes("2:168"), "Missing the uncached row measurement: " + JSON.stringify(suite.rowGeometryTrace));
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
            fakeTimelineModel.resetRows(6);
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
            fakeTimelineModel.resetRows(6);
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
            suite.anchorExcursionTrace = [];
            suite.anchorCorrectionTrace = [];
            suite.rowGeometryTrace = [];
            suite.frameSampling = true;

            wait(300);
            suite.frameSampling = false;

            verify(suite.sampledFrameCount > 0);
            verify(suite.maximumFrameAnchorExcursion <= 1, "A rendered frame moved the visible anchor by " + suite.maximumFrameAnchorExcursion + " px");
        }

        function test_heightChangesDuringADragDoNotMoveTheVisibleAnchor() {
            fakeTimelineModel.resetRows(400);
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

        function test_doesNotMoveTheVisibleAnchorAfterAHighVelocityLongDistanceFlickEnds() {
            fakeTimelineModel.resetRows(1000);
            suite.viewport = createTemporaryObject(postFlickSettlingViewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0);
            suite.scrollViewport = findChild(suite.viewport, "codexTimelineScrollViewport");
            verify(suite.scrollViewport !== null);
            suite.viewport.positionAtContentY(100);
            const initialContentY = suite.viewport.contentY;
            const initialAnchor = suite.viewport.captureVisibleAnchor();
            verify(initialAnchor !== null);
            suite.postFlickSettlementArmed = true;

            suite.viewport.flickContentForBenchmark(suite.stressFlickVelocity, suite.stressFlickDeceleration);

            tryVerify(() => Math.abs(suite.viewport.verticalVelocity) >= suite.minimumStressObservedVelocity, 200, "The stress flick never reached 30,000 px/s");
            tryVerify(() => suite.scrollViewport.moving);
            tryVerify(() => suite.postFlickSettlementStarted, 5000);
            verify(!suite.scrollViewport.moving);
            verify(Math.abs(suite.viewport.contentY - initialContentY) >= suite.minimumStressTravel, "The stress flick travelled only " + Math.abs(suite.viewport.contentY - initialContentY) + " px");
            verify(suite.frameReferenceAnchor !== null);
            verify(Math.abs(suite.viewport.indexOfEntryId(suite.frameReferenceAnchor.entryId) - suite.viewport.indexOfEntryId(initialAnchor.entryId)) >= suite.minimumStressRowTravel, "The stress flick did not cross enough rows");

            wait(120);
            suite.frameSampling = false;

            verify(suite.sampledFrameCount > 0);
            compare(suite.rowGeometryTrace.length, 1);
            compare(suite.anchorCorrectionTrace.length, 1);
            verify(suite.maximumFrameAnchorExcursion <= 1, "Post-flick trace: " + JSON.stringify({
                maximumAnchorExcursion: suite.maximumFrameAnchorExcursion,
                frames: suite.anchorExcursionTrace,
                anchorCorrections: suite.anchorCorrectionTrace,
                rowGeometryChanges: suite.rowGeometryTrace
            }));
        }

        function test_doesNotPresentReverseRowMotionWhenGeometryChangesDuringFinalDeceleration() {
            fakeTimelineModel.resetRows(1000);
            suite.viewport = createTemporaryObject(postFlickSettlingViewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0);
            suite.scrollViewport = findChild(suite.viewport, "codexTimelineScrollViewport");
            verify(suite.scrollViewport !== null);
            suite.viewport.positionAtContentY(100);
            const initialContentY = suite.viewport.contentY;
            const initialAnchor = suite.viewport.captureVisibleAnchor();
            verify(initialAnchor !== null);
            suite.finalDecelerationSampling = true;

            suite.viewport.flickContentForBenchmark(suite.stressFlickVelocity, suite.stressFlickDeceleration);

            tryVerify(() => Math.abs(suite.viewport.verticalVelocity) >= suite.minimumStressObservedVelocity, 200, "The stress flick never reached 30,000 px/s");
            tryVerify(() => suite.scrollViewport.moving);
            tryVerify(() => suite.finalDecelerationHeightChangeTriggered, 5000);
            tryVerify(() => !suite.scrollViewport.moving, 5000);
            verify(Math.abs(suite.viewport.contentY - initialContentY) >= suite.minimumStressTravel, "The stress flick travelled only " + Math.abs(suite.viewport.contentY - initialContentY) + " px");
            verify(suite.finalDecelerationAnchor !== null);
            verify(Math.abs(suite.viewport.indexOfEntryId(suite.finalDecelerationAnchor.entryId) - suite.viewport.indexOfEntryId(initialAnchor.entryId)) >= suite.minimumStressRowTravel, "The stress flick did not cross enough rows");

            wait(120);
            suite.finalDecelerationSampling = false;

            verify(suite.finalDecelerationTrace.length > 0);
            compare(suite.finalDecelerationMissingFrameCount, 0);
            verify(suite.viewport.rowHeightRevision > suite.finalDecelerationInitialRowHeightRevision);
            verify(suite.maximumOppositeDirectionMovement <= 1, "Final deceleration trace: " + JSON.stringify(suite.finalDecelerationTrace));
        }

        function test_geometryCorrectionDoesNotCancelAHighVelocityLongDistanceFlick() {
            fakeTimelineModel.resetRows(1000);
            suite.viewport = createTemporaryObject(postFlickSettlingViewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0);
            suite.scrollViewport = findChild(suite.viewport, "codexTimelineScrollViewport");
            verify(suite.scrollViewport !== null);
            suite.viewport.positionAtContentY(100);
            const initialContentY = suite.viewport.contentY;
            const initialAnchor = suite.viewport.captureVisibleAnchor();
            verify(initialAnchor !== null);

            suite.deferredMeasurementSampling = true;
            suite.viewport.flickContentForBenchmark(suite.stressFlickVelocity, suite.stressFlickDeceleration);

            tryVerify(() => Math.abs(suite.viewport.verticalVelocity) >= suite.minimumStressObservedVelocity, 200, "The stress flick never reached 30,000 px/s");
            tryVerify(() => suite.viewport.contentY - initialContentY >= 1000 && Math.abs(suite.viewport.verticalVelocity) >= 25000, 500, "The stress flick did not reach an early high-velocity geometry trigger");
            const movingAnchor = suite.viewport.captureVisibleAnchor();
            const movingAnchorRow = movingAnchor ? suite.viewport.indexOfEntryId(movingAnchor.entryId) : -1;
            const changedRow = movingAnchorRow - 1;
            const changedEntryId = changedRow >= 0 ? suite.viewport.entryIdAt(changedRow) : "";
            verify(changedRow >= 0);
            tryVerify(() => suite.viewport.delegateForEntry(changedEntryId) !== null);
            const initialRowHeightRevision = suite.viewport.rowHeightRevision;
            const initialRowGeometryChangeCount = suite.rowGeometryChangeCount;

            suite.postFlickChangedRowCount = 4;
            suite.postFlickChangedRow = changedRow;

            const changedSlot = suite.scrollViewport.itemAtIndex(changedRow);
            verify(changedSlot !== null);
            tryVerify(() => Number(changedSlot.pendingMeasuredHeight) > 0);
            tryVerify(() => suite.viewport.deferredRowMeasurementCount >= suite.postFlickChangedRowCount);
            compare(suite.viewport.rowHeightRevision, initialRowHeightRevision);
            wait(100);
            verify(suite.scrollViewport.moving, "The geometry correction cancelled the remaining kinetic flick");
            tryVerify(() => !suite.scrollViewport.moving, 5000);
            suite.deferredMeasurementSampling = false;
            tryVerify(() => suite.viewport.rowHeightRevision > initialRowHeightRevision);
            verify(suite.rowGeometryChangeCount - initialRowGeometryChangeCount <= suite.maximumActiveRowSlotCount, "The stopped-frame flush scaled with traversed rows instead of active slots");
            const finalAnchor = suite.viewport.captureVisibleAnchor();
            verify(finalAnchor !== null);
            verify(Math.abs(suite.viewport.contentY - initialContentY) >= suite.minimumStressTravel, "The corrected stress flick travelled only " + Math.abs(suite.viewport.contentY - initialContentY) + " px");
            verify(Math.abs(suite.viewport.indexOfEntryId(finalAnchor.entryId) - suite.viewport.indexOfEntryId(initialAnchor.entryId)) >= suite.minimumStressRowTravel, "The corrected stress flick did not cross enough rows");
            verify(suite.maximumDeferredRowMeasurementCount <= suite.maximumActiveRowSlotCount, "Deferred measurements grew beyond the active viewport slots");
        }

        function test_doesNotPresentReverseRowMotionNearTheOldBottomBoundary() {
            fakeTimelineModel.resetRows(1000);
            suite.viewport = createTemporaryObject(postFlickSettlingViewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0);
            suite.scrollViewport = findChild(suite.viewport, "codexTimelineScrollViewport");
            verify(suite.scrollViewport !== null);
            suite.viewport.positionAtContentY(suite.viewport.maximumContentY - 26000);
            const initialContentY = suite.viewport.contentY;
            const initialAnchor = suite.viewport.captureVisibleAnchor();
            verify(initialAnchor !== null);
            suite.postFlickChangedHeight = 1612;
            suite.finalDecelerationContentDirection = 1;
            suite.finalDecelerationSampling = true;

            suite.viewport.flickContentForBenchmark(suite.stressFlickVelocity, suite.stressFlickDeceleration);

            tryVerify(() => Math.abs(suite.viewport.verticalVelocity) >= suite.minimumStressObservedVelocity, 200, "The stress flick never reached 30,000 px/s");
            tryVerify(() => suite.finalDecelerationHeightChangeTriggered, 5000);
            tryVerify(() => !suite.scrollViewport.moving, 5000);
            verify(suite.viewport.contentY - initialContentY >= suite.minimumStressTravel, "The boundary stress flick travelled only " + (suite.viewport.contentY - initialContentY) + " px");
            verify(suite.finalDecelerationAnchor !== null);

            wait(120);
            suite.finalDecelerationSampling = false;

            compare(suite.finalDecelerationMissingFrameCount, 0);
            verify(suite.maximumOppositeDirectionMovement <= 1, "Bottom-boundary final deceleration trace: " + JSON.stringify(suite.finalDecelerationTrace));
        }

        function test_doesNotDoubleCorrectAnAnchorPreservedByListView() {
            fakeTimelineModel.resetRows(1000);
            suite.viewport = createTemporaryObject(postFlickSettlingViewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0);
            suite.scrollViewport = findChild(suite.viewport, "codexTimelineScrollViewport");
            verify(suite.scrollViewport !== null);
            suite.viewport.positionAtContentY(40000);
            const initialContentY = suite.viewport.contentY;
            suite.postFlickChangedHeight = 1214;
            suite.finalDecelerationContentDirection = -1;
            suite.finalDecelerationChangedRowOffset = -6;
            suite.finalDecelerationSampling = true;

            suite.viewport.flickContentForBenchmark(-suite.stressFlickVelocity, suite.stressFlickDeceleration);

            tryVerify(() => Math.abs(suite.viewport.verticalVelocity) >= suite.minimumStressObservedVelocity, 200, "The stress flick never reached 30,000 px/s");
            tryVerify(() => suite.finalDecelerationHeightChangeTriggered, 5000);
            tryVerify(() => !suite.scrollViewport.moving, 5000);
            verify(initialContentY - suite.viewport.contentY >= suite.minimumStressTravel, "The stress flick travelled only " + (initialContentY - suite.viewport.contentY) + " px");
            verify(suite.finalDecelerationAnchor !== null);

            wait(120);
            suite.finalDecelerationSampling = false;

            compare(suite.finalDecelerationMissingFrameCount, 0);
            verify(suite.maximumOppositeDirectionMovement <= 1, "ListView-preserved final deceleration trace: " + JSON.stringify(suite.finalDecelerationTrace));
        }

        function test_repeatedGeometryChangesDoNotShortenAHighVelocityLongDistanceFlick() {
            fakeTimelineModel.resetRows(1000);
            suite.viewport = createTemporaryObject(variableHeightViewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0);
            suite.scrollViewport = findChild(suite.viewport, "codexTimelineScrollViewport");
            verify(suite.scrollViewport !== null);
            suite.viewport.positionAtContentY(100);
            const initialContentY = suite.viewport.contentY;
            const initialAnchor = suite.viewport.captureVisibleAnchor();
            const initialRowHeightRevision = suite.viewport.rowHeightRevision;
            const initialRowGeometryChangeCount = suite.rowGeometryChangeCount;
            verify(initialAnchor !== null);

            suite.deferredMeasurementSampling = true;
            suite.viewport.flickContentForBenchmark(suite.stressFlickVelocity, suite.stressFlickDeceleration);

            tryVerify(() => Math.abs(suite.viewport.verticalVelocity) >= suite.minimumStressObservedVelocity, 200, "The stress flick never reached 30,000 px/s");
            tryVerify(() => suite.scrollViewport.moving);
            tryVerify(() => !suite.scrollViewport.moving, 5000);
            suite.deferredMeasurementSampling = false;
            const finalAnchor = suite.viewport.captureVisibleAnchor();
            verify(finalAnchor !== null);
            verify(suite.viewport.rowHeightRevision > initialRowHeightRevision, "The stress flick did not publish its deferred height batch");
            verify(suite.rowGeometryChangeCount - initialRowGeometryChangeCount >= 10, "The stress flick did not exercise repeated row geometry changes");
            verify(Math.abs(suite.viewport.contentY - initialContentY) >= suite.minimumStressTravel, "Repeated geometry changes shortened the stress flick to " + Math.abs(suite.viewport.contentY - initialContentY) + " px");
            verify(Math.abs(suite.viewport.indexOfEntryId(finalAnchor.entryId) - suite.viewport.indexOfEntryId(initialAnchor.entryId)) >= suite.minimumStressRowTravel, "The stress flick did not cross enough rows");
            verify(suite.maximumDeferredRowMeasurementCount <= suite.maximumActiveRowSlotCount, "Deferred measurements grew beyond the active viewport slots");
            verify(suite.rowGeometryChangeCount - initialRowGeometryChangeCount <= suite.maximumActiveRowSlotCount, "The stopped-frame flush scaled with traversed rows instead of active slots");
        }

        function test_materializesVisibleDeferredContentOneRowAtATime() {
            fakeTimelineModel.resetRows(100);
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
