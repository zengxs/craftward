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
    required property Component rowDelegate
    property real bottomContentInset: 64
    property real contentHorizontalInset: 20
    property real contentMaximumWidth: 920
    property real estimatedRowHeight: 72
    property real rowSpacing: 10
    property real contentMaterializationMargin: viewportHeight
    property int maximumConcurrentContentMaterializations: 1
    property int activeContentMaterializationCount: 0
    property bool followLiveTail: true
    property var lastVisibleAnchor: null
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
    property real pendingAnchorContentYAdjustment: 0
    property bool anchorContentYAdjustmentScheduled: false
    property bool completed: false
    property int activeRowSlotCount: 0
    property var activeRowSlots: []
    property bool viewportUpdateScheduled: false
    property bool liveTailUpdatePending: false
    readonly property int modelRowCount: timelineModel ? Math.max(0, Number(timelineModel.totalRowCount)) : 0
    readonly property int modelRevision: timelineModel ? Number(timelineModel.revision) : 0
    readonly property real contentColumnWidth: Math.max(0, Math.min(availableWidth - contentHorizontalInset * 2, contentMaximumWidth))
    readonly property real frameBudgetMilliseconds: FrameTiming.budgetMilliseconds(root.Window.window)
    readonly property real contentY: scrollViewport.contentY
    readonly property real scrollContentHeight: scrollViewport.contentHeight
    readonly property real viewportHeight: scrollViewport.height
    readonly property bool moving: scrollViewport.moving

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
        if (typeof method === "function")
            return Number(method.call(root.timelineModel, entryId));
        const target = String(entryId);
        for (let row = 0; row < root.modelRowCount; ++row) {
            if (root.entryIdAt(row) === target)
                return row;
        }
        return -1;
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

    function settleAnchorAfterRowHeightChange(sourceRow, previousHeight, currentHeight) {
        const delta = Number(currentHeight) - Number(previousHeight);
        if (!Number.isFinite(delta) || Math.abs(delta) < 0.5)
            return;
        root.rowGeometryChanged(Number(sourceRow), delta);
        if (root.adjustingAnchor || root.anchorSettlementSuppressed || root.followLiveTail)
            return;
        if (scrollViewport.moving)
            return;
        const anchor = root.pendingAnchor ?? root.lastVisibleAnchor ?? root.captureVisibleAnchor();
        if (!anchor)
            return;
        const anchorRow = root.rowForAnchor(anchor);
        if (anchorRow >= 0 && Number(sourceRow) > anchorRow)
            return;
        if (anchorRow >= 0 && Number(sourceRow) < anchorRow)
            root.scheduleAnchorContentYAdjustment(delta);
        root.scheduleAnchorRestore(anchor);
    }

    function scheduleAnchorContentYAdjustment(delta) {
        root.pendingAnchorContentYAdjustment += Number(delta);
        if (root.anchorContentYAdjustmentScheduled)
            return;
        root.anchorContentYAdjustmentScheduled = true;
        Qt.callLater(root.applyPendingAnchorContentYAdjustment);
    }

    function applyPendingAnchorContentYAdjustment() {
        root.anchorContentYAdjustmentScheduled = false;
        const adjustment = root.pendingAnchorContentYAdjustment;
        root.pendingAnchorContentYAdjustment = 0;
        if (scrollViewport.moving || root.anchorSettlementSuppressed || !Number.isFinite(adjustment) || Math.abs(adjustment) < 0.5)
            return;
        scrollViewport.forceLayout();
        root.restorePendingAnchor();
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
        const slot = scrollViewport.itemAtIndex(row);
        if (!slot) {
            scrollViewport.positionViewAtIndex(row, ListView.Beginning);
            return false;
        }
        const top = slot.mapToItem(scrollViewport.contentItem, 0, 0).y;
        const minimumY = scrollViewport.originY;
        const maximumY = Math.max(minimumY, minimumY + scrollViewport.contentHeight - scrollViewport.height);
        const previousContentY = scrollViewport.contentY;
        root.adjustingAnchor = true;
        scrollViewport.contentY = Math.max(minimumY, Math.min(maximumY, top - Number(root.pendingAnchor.offset)));
        root.adjustingAnchor = false;
        const displacement = scrollViewport.contentY - previousContentY;
        if (Math.abs(displacement) >= 0.5)
            root.anchorPositionCorrected(displacement);
        return true;
    }

    function scrollToBottom() {
        if (!root.followLiveTail || root.modelRowCount <= 0)
            return;
        scrollViewport.positionViewAtEnd();
        Qt.callLater(() => {
            if (!root.followLiveTail)
                return;
            const minimumY = scrollViewport.originY;
            scrollViewport.contentY = Math.max(minimumY, minimumY + scrollViewport.contentHeight - scrollViewport.height);
            root.lastVisibleAnchor = root.captureVisibleAnchor();
        });
    }

    function positionAtContentY(contentY) {
        root.anchorRestoreRunning = false;
        root.anchorSettlementSuppressed = false;
        root.anchorSettlementResumeScheduled = false;
        root.anchorSettlementResumeFramesRemaining = 0;
        root.pendingAnchorContentYAdjustment = 0;
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

    function followLatest() {
        root.followLiveTail = true;
        Qt.callLater(root.scrollToBottom);
    }

    function resetForNewContent() {
        root.anchorRestoreRunning = false;
        root.anchorSettlementSuppressed = false;
        root.anchorSettlementResumeScheduled = false;
        root.anchorSettlementResumeFramesRemaining = 0;
        root.pendingAnchorContentYAdjustment = 0;
        root.pendingAnchor = null;
        root.lastVisibleAnchor = null;
        root.clearRowHeights();
        root.followLiveTail = true;
        Qt.callLater(root.scrollToBottom);
    }

    padding: 0

    contentItem: ListView {
        id: scrollViewport

        objectName: "codexTimelineScrollViewport"
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: root.modelRowCount
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
            property int sourceRow: index
            property int dataRevision: root.modelRevision
            property string loadedEntryId: ""
            property int loaderGeneration: 0
            property real lastEffectiveHeight: 0
            property bool slotCompleted: false
            property bool pooled: false
            property bool contentMaterializationAllowed: false
            property bool contentMaterializationRequested: false
            property bool contentMaterializationReady: true
            property bool contentMeasurementReady: false
            property string heightCacheKey: entryId
            property real measuredHeight: 0
            readonly property string entryId: {
                const currentRevision = dataRevision;
                return currentRevision >= 0 ? root.entryIdAt(sourceRow) : "";
            }
            readonly property Item loadedItem: rowLoader.item

            function synchronizeLoadedItemState() {
                const loadedItem = rowLoader.item;
                if (!loadedItem) {
                    contentMaterializationRequested = false;
                    contentMaterializationReady = true;
                    contentMeasurementReady = false;
                    heightCacheKey = entryId;
                    measuredHeight = 0;
                    return;
                }
                const requested = loadedItem["contentMaterializationRequested"];
                contentMaterializationRequested = requested === undefined ? false : Boolean(requested);
                const materializationReady = loadedItem["contentMaterializationReady"];
                contentMaterializationReady = !contentMaterializationRequested || materializationReady === undefined || Boolean(materializationReady);
                const measurementReady = loadedItem["contentMeasurementReady"];
                contentMeasurementReady = measurementReady === undefined ? true : Boolean(measurementReady);
                const loadedHeightCacheKey = loadedItem["heightCacheKey"];
                heightCacheKey = loadedHeightCacheKey === undefined || String(loadedHeightCacheKey).length === 0 ? entryId : String(loadedHeightCacheKey);
                const implicitHeight = Number(loadedItem.implicitHeight);
                measuredHeight = contentMeasurementReady && Number.isFinite(implicitHeight) ? Math.ceil(implicitHeight) : 0;
            }

            function synchronizeItem() {
                if (!rowLoader.item)
                    return;
                if (loadedEntryId !== root.entryIdAt(sourceRow)) {
                    reloadItem();
                    return;
                }
                rowLoader.item["sourceRow"] = sourceRow;
                rowLoader.item["dataRevision"] = dataRevision;
                if (rowLoader.item["contentMaterializationAllowed"] !== undefined)
                    rowLoader.item["contentMaterializationAllowed"] = contentMaterializationAllowed;
                synchronizeLoadedItemState();
            }

            function reloadItem() {
                ++loaderGeneration;
                const requestedGeneration = loaderGeneration;
                rowLoader.active = false;
                Qt.callLater(() => {
                    if (requestedGeneration === loaderGeneration)
                        rowLoader.active = true;
                });
            }

            width: scrollViewport.width
            height: measuredHeight > 0 ? measuredHeight : root.rowHeightAt(heightCacheKey)
            onDataRevisionChanged: synchronizeItem()
            onEntryIdChanged: {
                contentMaterializationAllowed = false;
                synchronizeLoadedItemState();
                lastEffectiveHeight = height;
                if (rowLoader.item && loadedEntryId !== entryId)
                    reloadItem();
                root.scheduleViewportUpdate();
            }
            onContentMaterializationAllowedChanged: synchronizeItem()
            onContentMaterializationRequestedChanged: root.scheduleViewportUpdate()
            onContentMaterializationReadyChanged: root.scheduleViewportUpdate()
            onYChanged: root.scheduleViewportUpdate()
            onHeightChanged: {
                root.scheduleViewportUpdate();
                if (!slotCompleted)
                    return;
                const previousHeight = lastEffectiveHeight;
                lastEffectiveHeight = height;
                root.settleAnchorAfterRowHeightChange(sourceRow, previousHeight, height);
            }
            onMeasuredHeightChanged: {
                if (measuredHeight > 0)
                    root.rememberRowHeight(heightCacheKey, measuredHeight);
            }
            Component.onCompleted: {
                lastEffectiveHeight = height;
                slotCompleted = true;
                ++root.activeRowSlotCount;
                root.registerRowSlot(rowSlot);
            }
            Component.onDestruction: {
                root.unregisterRowSlot(rowSlot);
                --root.activeRowSlotCount;
            }
            ListView.onPooled: {
                pooled = true;
                contentMaterializationAllowed = false;
                root.scheduleViewportUpdate();
            }
            ListView.onReused: {
                pooled = false;
                contentMaterializationAllowed = false;
                lastEffectiveHeight = height;
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
                visible: rowSlot.measuredHeight <= 0
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
            root.anchorRestoreRunning = false;
            root.anchorSettlementSuppressed = true;
            root.anchorSettlementResumeScheduled = false;
            root.anchorSettlementResumeFramesRemaining = 0;
            root.pendingAnchorContentYAdjustment = 0;
            root.pendingAnchor = null;
            root.followLiveTail = false;
            root.lastVisibleAnchor = root.captureVisibleAnchor();
            root.updateContentMaterialization();
        }
        onMovementEnded: {
            root.followLiveTail = atYEnd;
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
