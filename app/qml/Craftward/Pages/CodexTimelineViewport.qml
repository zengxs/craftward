// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import Craftward.Components
import "../Components/FrameTiming.js" as FrameTiming

Control {
    id: root

    required property var timelineModel
    // The delegate contract exposes sourceRow, dataRevision, entryId, and implicitHeight.
    // Deferred delegates may also expose contentMaterializationRequested, contentMaterializationReady,
    // contentMeasurementReady, contentMaterializationAllowed, and heightCacheKey.
    // Delegates with pending local layouts can expose prepareForLayout() to finish their measurement.
    required property Component rowDelegate
    property real bottomContentInset: 64
    property real contentHorizontalInset: 20
    property real contentMaximumWidth: 920
    property real estimatedRowHeight: 72
    property real rowSpacing: 10
    property string heightCacheNamespace: ""
    property real contentMaterializationMargin: viewportHeight
    property int maximumConcurrentContentMaterializations: 1
    property int activeContentMaterializationCount: 0
    property bool followLiveTail: true
    property var lastVisibleAnchor: null
    property var lastMovementEndedAnchor: null
    property var lastMovementEndedRows: []
    property var pendingAnchor: null
    property int pendingAnchorPasses: 0
    property int pendingAnchorStablePasses: 0
    property int pendingAnchorHeightRevision: -1
    property var rowHeights: ({})
    property int rowHeightRevision: 0
    property bool adjustingAnchor: false
    property bool anchorRestoreRunning: false
    property bool anchorSettlementSuppressed: false
    property bool anchorSettlementResumeScheduled: false
    property int anchorSettlementResumeFramesRemaining: 0
    property int anchorSettlementQuietFrameCount: 3
    property var deferredRowMeasurements: ({})
    property int deferredRowMeasurementCount: 0
    property bool completed: false
    property int activeRowSlotCount: 0
    property var activeRowSlots: []
    property bool viewportUpdateScheduled: false
    property bool liveTailUpdatePending: false
    readonly property int modelRowCount: scrollViewport.count
    readonly property int modelRevision: timelineModel ? Number(timelineModel.revision) : 0
    readonly property real contentColumnWidth: Math.max(0, Math.min(availableWidth - contentHorizontalInset * 2, contentMaximumWidth))
    readonly property real frameBudgetMilliseconds: FrameTiming.budgetMilliseconds(root.Window.window)
    readonly property real contentY: scrollViewport.contentY
    readonly property real scrollContentHeight: scrollViewport.contentHeight
    readonly property real viewportHeight: scrollViewport.height
    readonly property real minimumContentY: scrollViewport.originY
    readonly property real maximumContentY: Math.max(scrollViewport.originY, scrollViewport.originY + scrollViewport.contentHeight - scrollViewport.height)
    readonly property bool moving: scrollViewport.moving
    readonly property real verticalVelocity: scrollViewport.verticalVelocity

    signal anchorPositionCorrected(real displacement)
    signal rowGeometryChanged(int sourceRow, real heightDelta)

    function modelMethod(methodName) {
        return root.timelineModel ? root.timelineModel[methodName] : null;
    }

    function modelValueAt(row, roleName) {
        const method = root.modelMethod("valueAt");
        return typeof method === "function" ? method.call(root.timelineModel, row, roleName) : undefined;
    }

    function entryIdAt(row) {
        const method = root.modelMethod("entryIdAt");
        if (typeof method === "function")
            return String(method.call(root.timelineModel, row) ?? "");
        return String(root.modelValueAt(row, "entryId") ?? "");
    }

    function indexOfEntryId(entryId) {
        const method = root.modelMethod("indexOfEntryId");
        return typeof method === "function" ? Number(method.call(root.timelineModel, entryId)) : -1;
    }

    function rowForAnchor(anchor) {
        if (!anchor)
            return -1;
        const hintedRow = Number(anchor.row);
        if (Number.isInteger(hintedRow) && hintedRow >= 0 && hintedRow < root.modelRowCount && root.entryIdAt(hintedRow) === String(anchor.entryId))
            return hintedRow;
        return root.indexOfEntryId(anchor.entryId);
    }

    function cachedRowHeight(heightCacheKey) {
        const currentRevision = root.rowHeightRevision;
        const cached = currentRevision >= 0 ? Number(root.rowHeights[String(heightCacheKey)]) : 0;
        return Number.isFinite(cached) && cached > 0 ? cached : 0;
    }

    function defaultHeightCacheKey(entryId) {
        const namespace = String(root.heightCacheNamespace);
        const identity = String(entryId);
        return namespace.length > 0 ? namespace + ":" + identity : identity;
    }

    function rowHeightAt(heightCacheKey) {
        const cached = root.cachedRowHeight(heightCacheKey);
        return cached > 0 ? cached : Math.max(1, root.estimatedRowHeight);
    }

    function rememberRowHeight(heightCacheKey, measuredHeight) {
        const key = String(heightCacheKey);
        const height = Math.ceil(Number(measuredHeight));
        if (!key || !Number.isFinite(height) || height <= 0 || root.rowHeights[key] === height)
            return;
        root.rowHeights[key] = height;
        ++root.rowHeightRevision;
    }

    function clearRowHeights() {
        root.rowHeights = {};
        ++root.rowHeightRevision;
    }

    function deferRowMeasurement(entryId, heightCacheKey, sourceRow, previousHeight, measuredHeight) {
        const key = String(entryId);
        if (!key)
            return;
        const existing = root.deferredRowMeasurements[key];
        if (!existing)
            ++root.deferredRowMeasurementCount;
        root.deferredRowMeasurements[key] = {
            entryId: key,
            heightCacheKey: String(heightCacheKey),
            sourceRow: Number(sourceRow),
            previousHeight: existing ? Number(existing.previousHeight) : Number(previousHeight),
            measuredHeight: Number(measuredHeight)
        };
    }

    function discardDeferredRowMeasurement(entryId) {
        const key = String(entryId);
        if (!key || !root.deferredRowMeasurements[key])
            return;
        delete root.deferredRowMeasurements[key];
        root.deferredRowMeasurementCount = Math.max(0, root.deferredRowMeasurementCount - 1);
    }

    function clearDeferredRowMeasurements() {
        root.deferredRowMeasurements = {};
        root.deferredRowMeasurementCount = 0;
    }

    function restoreInstantiatedAnchor(anchor, forceLayout = false) {
        if (!anchor)
            return Number.NaN;
        root.adjustingAnchor = true;
        if (forceLayout)
            scrollViewport.forceLayout();
        const row = root.rowForAnchor(anchor);
        const slot = row >= 0 ? scrollViewport.itemAtIndex(row) : null;
        if (!slot || !Number.isFinite(Number(anchor.offset))) {
            root.adjustingAnchor = false;
            return Number.NaN;
        }
        const previousContentY = scrollViewport.contentY;
        const anchorTop = slot.mapToItem(scrollViewport.contentItem, 0, 0).y;
        const targetContentY = Math.max(root.minimumContentY, Math.min(root.maximumContentY, anchorTop - Number(anchor.offset)));
        if (Math.abs(targetContentY - previousContentY) >= 0.5)
            scrollViewport.contentY = targetContentY;
        const displacement = scrollViewport.contentY - previousContentY;
        root.adjustingAnchor = false;
        if (Math.abs(displacement) >= 0.5)
            root.anchorPositionCorrected(displacement);
        return displacement;
    }

    function restoreAnchorAfterLayout(anchor) {
        let displacement = root.restoreInstantiatedAnchor(anchor, true);
        if (Number.isFinite(displacement))
            return displacement;
        const row = root.rowForAnchor(anchor);
        if (row < 0)
            return 0;
        root.adjustingAnchor = true;
        scrollViewport.positionViewAtIndex(row, ListView.Beginning);
        scrollViewport.forceLayout();
        root.adjustingAnchor = false;
        displacement = root.restoreInstantiatedAnchor(anchor);
        return Number.isFinite(displacement) ? displacement : 0;
    }

    function flushDeferredRowMeasurements(anchorBeforeFlush = null) {
        const measurements = root.deferredRowMeasurements;
        root.clearDeferredRowMeasurements();
        const activeMeasurements = [];
        const selectedEntryIds = {};
        for (const slot of root.activeRowSlots) {
            if (!slot || slot.pooled || slot.loadedEntryId !== slot.entryId)
                continue;
            const entryId = String(slot.entryId);
            const measurement = measurements[entryId];
            if (!measurement || selectedEntryIds[entryId])
                continue;
            selectedEntryIds[entryId] = true;
            activeMeasurements.push(measurement);
        }
        if (activeMeasurements.length === 0)
            return false;
        // A tail settlement preserves the tail edge; other settlements preserve the visual anchor.
        let anchor = anchorBeforeFlush ?? root.captureVisibleAnchor();
        if (root.followLiveTail) {
            anchor = null;
            const tailRow = root.modelRowCount - 1;
            const tailSlot = scrollViewport.itemAtIndex(tailRow);
            if (tailSlot) {
                const tailEntryId = String(tailSlot.entryId);
                const tailMeasurement = selectedEntryIds[tailEntryId] ? measurements[tailEntryId] : null;
                const tailHeight = tailMeasurement ? Number(tailMeasurement.measuredHeight) : tailSlot.height;
                anchor = {
                    entryId: String(tailSlot.entryId),
                    row: tailRow,
                    offset: scrollViewport.height - root.bottomContentInset - tailHeight
                };
            }
        }
        const previousCurrentIndex = scrollViewport.currentIndex;
        const anchorRow = root.rowForAnchor(anchor);
        if (anchorRow >= 0)
            scrollViewport.currentIndex = anchorRow;
        let rowHeightCacheChanged = false;
        for (const measurement of activeMeasurements) {
            const heightCacheKey = String(measurement.heightCacheKey);
            const measuredHeight = Math.ceil(Number(measurement.measuredHeight));
            if (heightCacheKey && Number.isFinite(measuredHeight) && measuredHeight > 0 && root.rowHeights[heightCacheKey] !== measuredHeight) {
                root.rowHeights[heightCacheKey] = measuredHeight;
                rowHeightCacheChanged = true;
            }
            const delta = Number(measurement.measuredHeight) - Number(measurement.previousHeight);
            if (Number.isFinite(delta) && Math.abs(delta) >= 0.5) {
                const currentRow = root.indexOfEntryId(measurement.entryId);
                root.rowGeometryChanged(currentRow >= 0 ? currentRow : Number(measurement.sourceRow), delta);
            }
        }
        if (rowHeightCacheChanged)
            ++root.rowHeightRevision;
        for (const slot of root.activeRowSlots) {
            if (!slot || slot.pooled)
                continue;
            const measurement = selectedEntryIds[String(slot.entryId)] ? measurements[String(slot.entryId)] : null;
            if (measurement && slot.loadedEntryId === slot.entryId)
                slot.applyDeferredMeasuredHeight(measurement.measuredHeight);
        }
        if (anchor)
            root.restoreAnchorAfterLayout(anchor);
        else if (root.followLiveTail)
            root.scrollToBottom();
        else
            scrollViewport.forceLayout();
        scrollViewport.currentIndex = previousCurrentIndex;
        return true;
    }

    function registerRowSlot(slot) {
        root.activeRowSlots = root.activeRowSlots.concat([slot]);
        root.scheduleViewportUpdate();
    }

    function unregisterRowSlot(slot) {
        root.activeRowSlots = root.activeRowSlots.filter(candidate => candidate !== slot);
        root.scheduleViewportUpdate();
    }

    function scheduleViewportUpdate(updateLiveTail = false) {
        if (updateLiveTail)
            root.liveTailUpdatePending = true;
        if (root.viewportUpdateScheduled)
            return;
        root.viewportUpdateScheduled = true;
        Qt.callLater(root.applyScheduledViewportUpdate);
    }

    function applyScheduledViewportUpdate() {
        root.viewportUpdateScheduled = false;
        const updateLiveTail = root.liveTailUpdatePending;
        root.liveTailUpdatePending = false;
        root.updateContentMaterialization();
        if (updateLiveTail && root.followLiveTail)
            root.scrollToBottom();
    }

    function materializationDistance(slot) {
        const viewportStart = scrollViewport.contentY;
        const viewportEnd = viewportStart + scrollViewport.height;
        const slotStart = slot.y;
        const slotEnd = slotStart + slot.height;
        if (slotEnd < viewportStart)
            return viewportStart - slotEnd;
        if (slotStart > viewportEnd)
            return slotStart - viewportEnd;
        return 0;
    }

    function updateContentMaterialization() {
        const candidates = [];
        const margin = Math.max(0, Number(root.contentMaterializationMargin));
        for (const slot of root.activeRowSlots) {
            if (!slot)
                continue;
            if (slot.pooled || !slot.loadedItem) {
                slot.contentMaterializationAllowed = false;
                continue;
            }
            const requested = slot.contentMaterializationRequested;
            if (!requested) {
                slot.contentMaterializationAllowed = false;
                continue;
            }
            if (slot.contentMaterializationReady) {
                slot.contentMaterializationAllowed = true;
                continue;
            }
            const eligible = root.materializationDistance(slot) <= margin;
            if (!eligible) {
                slot.contentMaterializationAllowed = false;
                continue;
            }
            if (scrollViewport.moving || root.anchorSettlementSuppressed) {
                slot.contentMaterializationAllowed = false;
                continue;
            }
            candidates.push(slot);
        }
        candidates.sort((left, right) => {
            const distance = root.materializationDistance(left) - root.materializationDistance(right);
            return distance !== 0 ? distance : left.sourceRow - right.sourceRow;
        });
        const allowedCount = Math.min(candidates.length, Math.max(0, Number(root.maximumConcurrentContentMaterializations)));
        for (let index = 0; index < candidates.length; ++index)
            candidates[index].contentMaterializationAllowed = index < allowedCount;
        root.activeContentMaterializationCount = allowedCount;
    }

    function settleAnchorAfterRowHeightChange(sourceRow, previousHeight, currentHeight, anchorBeforeChange = null) {
        const delta = Number(currentHeight) - Number(previousHeight);
        if (!Number.isFinite(delta) || Math.abs(delta) < 0.5)
            return;
        root.rowGeometryChanged(Number(sourceRow), delta);
        if (root.adjustingAnchor || root.followLiveTail)
            return;
        const anchor = anchorBeforeChange ?? root.pendingAnchor ?? root.lastVisibleAnchor ?? root.captureVisibleAnchor();
        if (!anchor)
            return;
        const anchorRow = root.rowForAnchor(anchor);
        if (anchorRow >= 0 && Number(sourceRow) > anchorRow)
            return;
        if (scrollViewport.moving)
            return;
        root.restoreAnchorAfterLayout(anchor);
        root.scheduleAnchorRestore(anchor);
    }

    function firstInstantiatedIndexAt(contentY) {
        if (root.modelRowCount <= 0)
            return -1;
        const x = Math.max(1, scrollViewport.width / 2);
        const firstY = Math.max(scrollViewport.originY, Number(contentY));
        const probeLimit = Math.max(32, root.rowSpacing + 4);
        for (let offset = 0; offset <= probeLimit; offset += 2) {
            const row = scrollViewport.indexAt(x, firstY + offset);
            if (row >= 0)
                return row;
        }
        const center = scrollViewport.indexAt(x, firstY + scrollViewport.height / 2);
        return center >= 0 ? center : 0;
    }

    function captureVisibleAnchor() {
        const first = root.firstInstantiatedIndexAt(scrollViewport.contentY);
        if (first < 0)
            return null;
        const last = Math.min(root.modelRowCount - 1, first + 64);
        for (let row = first; row <= last; ++row) {
            const slot = scrollViewport.itemAtIndex(row);
            if (!slot || slot.height <= 0)
                continue;
            const top = slot.mapToItem(scrollViewport.contentItem, 0, 0).y;
            if (top + slot.height >= scrollViewport.contentY)
                return {
                    entryId: slot.entryId,
                    offset: top - scrollViewport.contentY,
                    row: row
                };
        }
        return null;
    }

    function delegateForEntry(entryId) {
        const row = root.indexOfEntryId(entryId);
        if (row < 0)
            return null;
        const slot = scrollViewport.itemAtIndex(row);
        return slot ? slot.loadedItem : null;
    }

    function scheduleAnchorRestore(anchor) {
        if (!anchor)
            return;
        root.lastVisibleAnchor = {
            entryId: String(anchor.entryId),
            offset: Number(anchor.offset),
            row: root.rowForAnchor(anchor)
        };
        root.pendingAnchor = root.lastVisibleAnchor;
        root.pendingAnchorPasses = 0;
        root.pendingAnchorStablePasses = 0;
        root.pendingAnchorHeightRevision = root.rowHeightRevision;
        root.anchorRestoreRunning = true;
    }

    function restorePendingAnchor() {
        if (!root.pendingAnchor)
            return false;
        const row = root.rowForAnchor(root.pendingAnchor);
        if (row < 0)
            return false;
        const displacement = root.restoreInstantiatedAnchor(root.pendingAnchor);
        if (Number.isFinite(displacement))
            return true;
        scrollViewport.positionViewAtIndex(row, ListView.Beginning);
        return false;
    }

    function scrollToBottom() {
        if (!root.followLiveTail || root.modelRowCount <= 0)
            return;
        scrollViewport.forceLayout();
        scrollViewport.positionViewAtEnd();
        scrollViewport.forceLayout();
        root.lastVisibleAnchor = root.captureVisibleAnchor();
    }

    function positionAtContentY(contentY) {
        root.anchorRestoreRunning = false;
        root.anchorSettlementSuppressed = false;
        root.anchorSettlementResumeScheduled = false;
        root.anchorSettlementResumeFramesRemaining = 0;
        root.pendingAnchor = null;
        const minimumY = scrollViewport.originY;
        const maximumY = Math.max(minimumY, minimumY + scrollViewport.contentHeight - scrollViewport.height);
        scrollViewport.contentY = Math.max(minimumY, Math.min(maximumY, Number(contentY)));
        root.lastVisibleAnchor = root.captureVisibleAnchor();
        Qt.callLater(() => {
            if (root.followLiveTail || scrollViewport.moving)
                return;
            root.lastVisibleAnchor = root.captureVisibleAnchor();
        });
    }

    function flickContentForBenchmark(verticalVelocity, deceleration = Number.NaN) {
        const requestedVelocity = Number(verticalVelocity);
        const requestedDeceleration = Number(deceleration);
        if (!Number.isFinite(requestedVelocity))
            return;
        scrollViewport.maximumFlickVelocity = Math.max(scrollViewport.maximumFlickVelocity, Math.abs(requestedVelocity));
        if (Number.isFinite(requestedDeceleration) && requestedDeceleration > 0)
            scrollViewport.flickDeceleration = requestedDeceleration;
        scrollViewport.flick(0, -requestedVelocity);
    }

    function cancelFlickForBenchmark() {
        scrollViewport.cancelFlick();
    }

    function anchorOffsetForBenchmark(anchor) {
        const row = root.rowForAnchor(anchor);
        if (row < 0)
            return Number.NaN;
        const slot = scrollViewport.itemAtIndex(row);
        if (!slot)
            return Number.NaN;
        return slot.mapToItem(scrollViewport.contentItem, 0, 0).y - scrollViewport.contentY;
    }

    function movementEndedAnchorForBenchmark() {
        return root.lastMovementEndedAnchor;
    }

    function movementEndedRowsForBenchmark() {
        return root.lastMovementEndedRows;
    }

    function visibleContentMarkersForBenchmark(slot, minimumOffset, maximumOffset) {
        const loadedItem = slot ? slot.loadedItem : null;
        if (!loadedItem)
            return [];
        const contentTop = loadedItem.mapToItem(scrollViewport.contentItem, 0, 0).y - scrollViewport.contentY;
        const contentHeight = Number(loadedItem.implicitHeight);
        if (!Number.isFinite(contentTop) || !Number.isFinite(contentHeight) || contentHeight <= 0)
            return [];
        const contentBottom = contentTop + contentHeight;
        const slotTop = slot.mapToItem(scrollViewport.contentItem, 0, 0).y - scrollViewport.contentY;
        const slotBottom = slotTop + slot.height;
        const clippedContentBottom = slot.clip ? Math.min(contentBottom, slotBottom) : contentBottom;
        const markers = [];
        if (contentTop >= minimumOffset && contentTop <= maximumOffset) {
            markers.push({
                markerId: "content:start",
                offset: contentTop
            });
        }
        if (contentBottom <= clippedContentBottom && contentBottom >= minimumOffset && contentBottom <= maximumOffset) {
            markers.push({
                markerId: "content:end",
                offset: contentBottom
            });
        }
        return markers;
    }

    function visibleRowOffsetsForBenchmark() {
        const first = root.firstInstantiatedIndexAt(scrollViewport.contentY);
        if (first < 0)
            return [];
        const rows = [];
        const viewportEnd = scrollViewport.contentY + scrollViewport.height;
        const last = Math.min(root.modelRowCount - 1, first + 64);
        for (let row = first; row <= last; ++row) {
            const slot = scrollViewport.itemAtIndex(row);
            if (!slot || slot.height <= 0)
                continue;
            const top = slot.mapToItem(scrollViewport.contentItem, 0, 0).y;
            if (top > viewportEnd)
                break;
            if (top + slot.height >= scrollViewport.contentY) {
                rows.push({
                    entryId: String(slot.entryId),
                    row: row,
                    offset: top - scrollViewport.contentY,
                    height: slot.height,
                    pendingMeasuredHeight: slot.pendingMeasuredHeight,
                    visualMarkers: root.visibleContentMarkersForBenchmark(slot, 0, scrollViewport.height)
                });
            }
        }
        return rows;
    }

    function trajectoryRowOffsetsForBenchmark() {
        const rows = [];
        const samplingMargin = Math.max(scrollViewport.height, Number(scrollViewport.cacheBuffer));
        for (const slot of root.activeRowSlots) {
            if (!slot || slot.pooled || slot.loadedEntryId !== slot.entryId || slot.height <= 0)
                continue;
            const offset = slot.mapToItem(scrollViewport.contentItem, 0, 0).y - scrollViewport.contentY;
            if (offset + slot.height < -samplingMargin || offset > scrollViewport.height + samplingMargin)
                continue;
            rows.push({
                entryId: String(slot.entryId),
                row: Number(slot.sourceRow),
                offset: offset,
                height: slot.height,
                pendingMeasuredHeight: slot.pendingMeasuredHeight,
                visualMarkers: root.visibleContentMarkersForBenchmark(slot, -samplingMargin, scrollViewport.height + samplingMargin)
            });
        }
        rows.sort((left, right) => left.row - right.row);
        return rows;
    }

    function followLatest() {
        root.followLiveTail = true;
        Qt.callLater(root.scrollToBottom);
    }

    function resetForNewContent() {
        root.anchorRestoreRunning = false;
        root.anchorSettlementSuppressed = false;
        root.anchorSettlementResumeScheduled = false;
        root.anchorSettlementResumeFramesRemaining = 0;
        root.pendingAnchor = null;
        root.lastVisibleAnchor = null;
        root.lastMovementEndedAnchor = null;
        root.lastMovementEndedRows = [];
        root.clearDeferredRowMeasurements();
        root.clearRowHeights();
        root.followLiveTail = true;
        Qt.callLater(root.scrollToBottom);
    }

    padding: 0

    contentItem: ListView {
        id: scrollViewport

        objectName: "codexTimelineScrollViewport"
        clip: true
        currentIndex: -1
        boundsBehavior: Flickable.StopAtBounds
        model: root.timelineModel
        spacing: root.rowSpacing
        cacheBuffer: Math.max(height * 3, 1200)
        reuseItems: true
        flickableDirection: Flickable.VerticalFlick
        ScrollBar.vertical: OverlayScrollBar {}

        header: Item {
            width: 1
            height: 20
        }

        footer: Item {
            width: 1
            height: root.bottomContentInset
        }

        delegate: Item {
            id: rowSlot

            required property int index
            required property string entryId
            property int sourceRow: index
            property int dataRevision: root.modelRevision
            property string loadedEntryId: ""
            property int loaderGeneration: 0
            property bool slotCompleted: false
            property bool pooled: false
            property bool contentMaterializationAllowed: false
            property bool contentMaterializationRequested: false
            property bool contentMaterializationReady: true
            property bool contentMeasurementReady: false
            property string heightCacheKey: root.defaultHeightCacheKey(entryId)
            property real measuredHeight: 0
            property real pendingMeasuredHeight: 0
            readonly property Item loadedItem: rowLoader.item

            function initializeLayoutHeight() {
                // Finish the row before ListView reads its size to place this identity.
                rowLoader.asynchronous = false;
                synchronizeItem();
                const prepare = rowLoader.item ? rowLoader.item["prepareForLayout"] : null;
                if (typeof prepare === "function")
                    prepare.call(rowLoader.item);
                const ready = rowLoader.item && loadedEntryId === entryId && contentMeasurementReady;
                const nextHeight = ready ? Math.ceil(Number(rowLoader.item.implicitHeight)) : 0;
                const cachedHeight = root.cachedRowHeight(heightCacheKey);
                root.discardDeferredRowMeasurement(entryId);
                pendingMeasuredHeight = 0;
                measuredHeight = Number.isFinite(nextHeight) && nextHeight > 0 ? nextHeight : root.rowHeightAt(heightCacheKey);
                if (ready && Number.isFinite(nextHeight) && nextHeight > 0) {
                    root.rememberRowHeight(heightCacheKey, nextHeight);
                    const delta = nextHeight - (cachedHeight > 0 ? cachedHeight : root.estimatedRowHeight);
                    if (Math.abs(delta) >= 0.5)
                        root.rowGeometryChanged(sourceRow, delta);
                }
                rowLoader.asynchronous = true;
            }

            function synchronizeLoadedItemState() {
                const loadedItem = rowLoader.item;
                if (!loadedItem) {
                    const retainedHeight = scrollViewport.moving ? Number(height) : 0;
                    contentMaterializationRequested = false;
                    contentMaterializationReady = true;
                    contentMeasurementReady = false;
                    measuredHeight = Number.isFinite(retainedHeight) && retainedHeight > 0 ? Math.ceil(retainedHeight) : 0;
                    heightCacheKey = root.defaultHeightCacheKey(entryId);
                    pendingMeasuredHeight = 0;
                    return;
                }
                const requested = loadedItem["contentMaterializationRequested"];
                contentMaterializationRequested = requested === undefined ? false : Boolean(requested);
                const materializationReady = loadedItem["contentMaterializationReady"];
                contentMaterializationReady = !contentMaterializationRequested || materializationReady === undefined || Boolean(materializationReady);
                const measurementReady = loadedItem["contentMeasurementReady"];
                contentMeasurementReady = measurementReady === undefined ? true : Boolean(measurementReady);
                const loadedHeightCacheKey = loadedItem["heightCacheKey"];
                heightCacheKey = loadedHeightCacheKey === undefined || String(loadedHeightCacheKey).length === 0 ? root.defaultHeightCacheKey(entryId) : String(loadedHeightCacheKey);
                const implicitHeight = Number(loadedItem.implicitHeight);
                const nextMeasuredHeight = contentMeasurementReady && Number.isFinite(implicitHeight) ? Math.ceil(implicitHeight) : 0;
                if (loadedEntryId === entryId)
                    commitMeasuredHeight(nextMeasuredHeight);
            }

            function commitMeasuredHeight(nextMeasuredHeight = measuredHeight) {
                const nextHeight = Number(nextMeasuredHeight);
                if (!slotCompleted || pooled || loadedEntryId !== entryId || !Number.isFinite(nextHeight) || nextHeight <= 0)
                    return;
                const previousHeight = height;
                const cachedHeight = root.cachedRowHeight(heightCacheKey);
                // A cached height still changes presented geometry when a pooled shell changes identity.
                if (scrollViewport.moving && Math.abs(previousHeight - nextHeight) >= 0.5) {
                    pendingMeasuredHeight = nextHeight;
                    root.deferRowMeasurement(entryId, heightCacheKey, sourceRow, previousHeight, nextHeight);
                    return;
                }
                if (cachedHeight === nextHeight) {
                    root.discardDeferredRowMeasurement(entryId);
                    pendingMeasuredHeight = 0;
                    measuredHeight = nextHeight;
                    return;
                }
                if (scrollViewport.moving) {
                    pendingMeasuredHeight = nextHeight;
                    root.deferRowMeasurement(entryId, heightCacheKey, sourceRow, previousHeight, nextHeight);
                    return;
                }
                const anchorBeforeChange = root.followLiveTail ? null : root.captureVisibleAnchor();
                root.discardDeferredRowMeasurement(entryId);
                pendingMeasuredHeight = 0;
                measuredHeight = nextHeight;
                root.rememberRowHeight(heightCacheKey, nextHeight);
                root.settleAnchorAfterRowHeightChange(sourceRow, previousHeight, nextHeight, anchorBeforeChange);
            }

            function applyDeferredMeasuredHeight(nextMeasuredHeight) {
                const nextHeight = Number(nextMeasuredHeight);
                if (loadedEntryId !== entryId || !Number.isFinite(nextHeight) || nextHeight <= 0)
                    return;
                pendingMeasuredHeight = 0;
                measuredHeight = nextHeight;
            }

            function synchronizeItem() {
                const loadedItem = rowLoader.item;
                if (!loadedItem)
                    return;
                if (entryId !== root.entryIdAt(sourceRow))
                    return;
                if (loadedEntryId !== entryId) {
                    replaceReusedItem();
                    return;
                }
                loadedItem["sourceRow"] = sourceRow;
                if (rowLoader.item !== loadedItem)
                    return;
                loadedItem["dataRevision"] = dataRevision;
                if (rowLoader.item !== loadedItem)
                    return;
                if (loadedItem["contentMaterializationAllowed"] !== undefined)
                    loadedItem["contentMaterializationAllowed"] = contentMaterializationAllowed;
                synchronizeLoadedItemState();
            }

            function replaceReusedItem() {
                root.discardDeferredRowMeasurement(loadedEntryId);
                ++loaderGeneration;
                rowLoader.active = false;
                rowLoader.active = true;
            }

            width: scrollViewport.width
            height: measuredHeight > 0 ? measuredHeight : root.rowHeightAt(heightCacheKey)
            clip: pendingMeasuredHeight > 0
            onDataRevisionChanged: synchronizeItem()
            onSourceRowChanged: synchronizeItem()
            onEntryIdChanged: {
                root.discardDeferredRowMeasurement(loadedEntryId);
                contentMaterializationAllowed = false;
                pendingMeasuredHeight = 0;
                synchronizeLoadedItemState();
                if (rowLoader.item && loadedEntryId !== entryId)
                    replaceReusedItem();
                root.scheduleViewportUpdate();
            }
            onContentMaterializationAllowedChanged: synchronizeItem()
            onContentMaterializationRequestedChanged: root.scheduleViewportUpdate()
            onContentMaterializationReadyChanged: root.scheduleViewportUpdate()
            onYChanged: root.scheduleViewportUpdate()
            onHeightChanged: root.scheduleViewportUpdate()
            Component.onCompleted: {
                initializeLayoutHeight();
                slotCompleted = true;
                commitMeasuredHeight();
                ++root.activeRowSlotCount;
                root.registerRowSlot(rowSlot);
            }
            Component.onDestruction: {
                root.discardDeferredRowMeasurement(loadedEntryId);
                root.unregisterRowSlot(rowSlot);
                --root.activeRowSlotCount;
            }
            ListView.onPooled: {
                root.discardDeferredRowMeasurement(loadedEntryId);
                pooled = true;
                contentMaterializationAllowed = false;
                root.scheduleViewportUpdate();
            }
            ListView.onReused: {
                initializeLayoutHeight();
                pooled = false;
                contentMaterializationAllowed = false;
                root.scheduleViewportUpdate();
            }

            Rectangle {
                anchors {
                    horizontalCenter: parent.horizontalCenter
                    top: parent.top
                }
                width: root.contentColumnWidth
                height: parent.height
                radius: 8
                color: root.palette.alternateBase
                opacity: 0.42
                visible: !rowSlot.loadedItem || rowSlot.measuredHeight <= 0
            }

            Loader {
                id: rowLoader

                anchors {
                    horizontalCenter: parent.horizontalCenter
                    top: parent.top
                }
                width: root.contentColumnWidth
                height: item ? item.implicitHeight : 0
                asynchronous: true
                sourceComponent: root.rowDelegate
                onItemChanged: rowSlot.synchronizeLoadedItemState()
                onLoaded: {
                    rowSlot.loadedEntryId = rowSlot.entryId;
                    rowSlot.synchronizeItem();
                    root.scheduleViewportUpdate();
                }
            }

            Connections {
                target: rowLoader.item
                ignoreUnknownSignals: true

                function onContentMaterializationRequestedChanged() {
                    rowSlot.synchronizeLoadedItemState();
                }

                function onContentMaterializationReadyChanged() {
                    rowSlot.synchronizeLoadedItemState();
                }

                function onContentMeasurementReadyChanged() {
                    rowSlot.synchronizeLoadedItemState();
                }

                function onHeightCacheKeyChanged() {
                    rowSlot.synchronizeLoadedItemState();
                }

                function onImplicitHeightChanged() {
                    rowSlot.synchronizeLoadedItemState();
                }
            }
        }

        onContentHeightChanged: {
            root.scheduleViewportUpdate(true);
        }
        onContentYChanged: root.scheduleViewportUpdate()
        onMovementStarted: {
            if (root.adjustingAnchor)
                return;
            root.anchorRestoreRunning = false;
            root.anchorSettlementSuppressed = true;
            root.anchorSettlementResumeScheduled = false;
            root.anchorSettlementResumeFramesRemaining = 0;
            root.pendingAnchor = null;
            root.followLiveTail = false;
            root.lastVisibleAnchor = root.captureVisibleAnchor();
            root.lastMovementEndedAnchor = null;
            root.lastMovementEndedRows = [];
            root.updateContentMaterialization();
        }
        onMovementEnded: {
            if (root.adjustingAnchor)
                return;
            const reachedLiveTail = atYEnd;
            const stoppedAnchor = root.captureVisibleAnchor();
            root.lastMovementEndedRows = root.visibleRowOffsetsForBenchmark();
            root.lastMovementEndedAnchor = stoppedAnchor ? {
                entryId: String(stoppedAnchor.entryId),
                offset: Number(stoppedAnchor.offset),
                row: Number(stoppedAnchor.row),
                contentY: Number(scrollViewport.contentY)
            } : null;
            root.followLiveTail = reachedLiveTail;
            const settled = root.flushDeferredRowMeasurements(stoppedAnchor);
            if (reachedLiveTail && !settled)
                root.scrollToBottom();
            root.lastVisibleAnchor = root.captureVisibleAnchor();
            root.anchorSettlementResumeFramesRemaining = Math.max(1, root.anchorSettlementQuietFrameCount);
            root.anchorSettlementResumeScheduled = true;
            root.scheduleViewportUpdate();
        }
    }

    Connections {
        target: root.timelineModel
        ignoreUnknownSignals: true

        function onRevisionChanged() {
            root.scheduleViewportUpdate(true);
        }

        function onStatisticsChanged() {
            root.scheduleViewportUpdate(true);
        }
    }

    FrameAnimation {
        running: root.anchorRestoreRunning
        onTriggered: {
            if (scrollViewport.moving) {
                root.anchorRestoreRunning = false;
                root.pendingAnchor = null;
                return;
            }
            scrollViewport.forceLayout();
            ++root.pendingAnchorPasses;
            const restored = root.restorePendingAnchor();
            if (restored && root.pendingAnchorHeightRevision === root.rowHeightRevision)
                ++root.pendingAnchorStablePasses;
            else
                root.pendingAnchorStablePasses = 0;
            root.pendingAnchorHeightRevision = root.rowHeightRevision;
            if (root.pendingAnchorStablePasses >= 3 || root.pendingAnchorPasses >= 30) {
                root.anchorRestoreRunning = false;
                root.lastVisibleAnchor = root.captureVisibleAnchor();
                root.pendingAnchor = null;
            }
        }
    }

    FrameAnimation {
        running: root.anchorSettlementResumeScheduled
        onTriggered: {
            if (root.anchorSettlementResumeFramesRemaining > 1) {
                --root.anchorSettlementResumeFramesRemaining;
                return;
            }
            root.anchorSettlementResumeFramesRemaining = 0;
            root.anchorSettlementResumeScheduled = false;
            root.anchorSettlementSuppressed = false;
            root.lastVisibleAnchor = root.captureVisibleAnchor();
            root.scheduleViewportUpdate();
        }
    }

    onModelRowCountChanged: {
        root.scheduleViewportUpdate(true);
    }
    onContentMaterializationMarginChanged: root.scheduleViewportUpdate()
    onMaximumConcurrentContentMaterializationsChanged: root.scheduleViewportUpdate()
    onContentColumnWidthChanged: {
        if (!root.completed)
            return;
        const anchor = root.captureVisibleAnchor() ?? root.lastVisibleAnchor;
        root.clearRowHeights();
        root.scheduleAnchorRestore(anchor);
    }

    Component.onCompleted: {
        root.completed = true;
        Qt.callLater(root.scrollToBottom);
    }
}
