// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

QtObject {
    id: root

    required property var targetViewport
    required property var targetWindow
    property bool active: false
    property string rendererName: "current"
    property string requestedThreadId: ""
    property string selectedThreadId: ""
    property bool conversationLoading: false
    property int rowCount: 0
    property real frameBudgetMilliseconds: 1000 / 60
    property int readinessTimeoutMilliseconds: 30000
    property int readinessStabilityMilliseconds: 750
    property int phasePreparationMilliseconds: 300
    property int legDurationMilliseconds: 2100
    property int legCompletionTimeoutMilliseconds: 500
    property int legPauseMilliseconds: 120
    property int phaseSettlementMilliseconds: 500
    property real flickContentVelocity: 32000
    property real flickDeceleration: 16000
    property int stoppedTrajectoryFrameSampleCount: 3
    property real movementEpsilon: 0.1
    property int minimumScrollableViewportCount: 4
    property int minimumFrameSampleCount: 60
    property int minimumMotionSampleCount: 60
    property real minimumMotionUpdateRatio: 0.98
    property int maximumFrozenMotionStreak: 1
    property real maximumP95FrameBudgetRatio: 1.25
    property real maximumP99FrameBudgetRatio: 1.75
    property real maximumWorstFrameBudgetRatio: 3
    property real maximumAnchorExcursionPixels: 1
    property real maximumReverseRowMotionPixels: 1
    property real maximumTrajectoryRowGeometryDriftPixels: 1
    property real maximumStoppedRowDriftPixels: 1
    property int minimumTrajectoryRowMotionSampleCount: 6
    property int minimumStoppedTrajectoryFrameSampleCount: 6
    property int maximumMissingTrajectoryRowFrameCount: 0
    property int maximumWarmRowGeometryChangeCount: 0
    property real minimumFlickTravelPixels: 24000
    property real minimumObservedFlickVelocity: 30000
    property int minimumFlickRowTravel: 100
    readonly property real displayRefreshRate: frameBudgetMilliseconds > 0 ? 1000 / frameBudgetMilliseconds : 0
    readonly property var phaseNames: ["cold", "warm-1", "warm-2"]
    readonly property var legDirections: [-1, 1]
    property int requiredLegMeasurementCount: legDirections.length
    property string state: "idle"
    property bool componentCompleted: false
    property bool resultEmitted: false
    property bool phaseActive: false
    property bool legActive: false
    property bool legEndRequested: false
    property bool settlementSamplingActive: false
    property int phaseIndex: -1
    property int legIndex: 0
    property int activeLegContentDirection: 0
    property int legStartRow: -1
    property real legStartContentY: Number.NaN
    property real legMaximumObservedVelocity: 0
    property real benchmarkStartedAt: Number.NaN
    property real readinessStableSince: Number.NaN
    property real lastReadinessContentHeight: Number.NaN
    property real readinessMilliseconds: 0
    property real phaseStartContentY: 0
    property real previousFrameTimestamp: Number.NaN
    property real previousContentY: Number.NaN
    property bool previousMoving: false
    property real previousTrajectoryContentY: Number.NaN
    property var previousTrajectoryVisibleRowOffsets: ({})
    property var movementEndBaselineAnchor: null
    property int stoppedTrajectoryFramesSampled: 0
    property var settlementReferenceAnchor: null
    property var phaseResults: []
    property var presentedWindowState: null
    readonly property TimelineRenderMetrics metricAccumulator: TimelineRenderMetrics {}

    signal finished(var benchmarkResult)

    function rounded(value, decimalPlaces = 3) {
        const numericValue = Number(value);
        if (!Number.isFinite(numericValue))
            return 0;
        const scale = Math.pow(10, decimalPlaces);
        return Math.round(numericValue * scale) / scale;
    }

    function thresholdsSnapshot() {
        return {
            minimumFrameSampleCount: root.minimumFrameSampleCount,
            minimumMotionSampleCount: root.minimumMotionSampleCount,
            minimumMotionUpdateRatio: root.minimumMotionUpdateRatio,
            maximumFrozenMotionStreak: root.maximumFrozenMotionStreak,
            maximumP95FrameBudgetRatio: root.maximumP95FrameBudgetRatio,
            maximumP99FrameBudgetRatio: root.maximumP99FrameBudgetRatio,
            maximumWorstFrameBudgetRatio: root.maximumWorstFrameBudgetRatio,
            maximumAnchorExcursionPixels: root.maximumAnchorExcursionPixels,
            maximumReverseRowMotionPixels: root.maximumReverseRowMotionPixels,
            maximumTrajectoryRowGeometryDriftPixels: root.maximumTrajectoryRowGeometryDriftPixels,
            maximumStoppedRowDriftPixels: root.maximumStoppedRowDriftPixels,
            minimumTrajectoryRowMotionSampleCount: root.minimumTrajectoryRowMotionSampleCount,
            minimumStoppedTrajectoryFrameSampleCount: root.minimumStoppedTrajectoryFrameSampleCount,
            maximumMissingTrajectoryRowFrameCount: root.maximumMissingTrajectoryRowFrameCount,
            maximumWarmRowGeometryChangeCount: root.maximumWarmRowGeometryChangeCount,
            minimumFlickTravelPixels: root.minimumFlickTravelPixels,
            minimumObservedFlickVelocity: root.minimumObservedFlickVelocity,
            minimumFlickRowTravel: root.minimumFlickRowTravel,
            requiredLegMeasurementCount: root.requiredLegMeasurementCount
        };
    }

    function clearTrajectorySampleState() {
        root.previousTrajectoryContentY = Number.NaN;
        root.previousTrajectoryVisibleRowOffsets = {};
        root.movementEndBaselineAnchor = null;
        root.stoppedTrajectoryFramesSampled = 0;
    }

    function clearLegSampleState() {
        root.previousFrameTimestamp = Number.NaN;
        root.previousContentY = Number.NaN;
        root.previousMoving = false;
        root.activeLegContentDirection = 0;
        root.legEndRequested = false;
        root.legStartContentY = Number.NaN;
        root.legMaximumObservedVelocity = 0;
        root.clearTrajectorySampleState();
    }

    function resetPhaseMetrics() {
        root.metricAccumulator.resetPhase();
        root.clearLegSampleState();
    }

    function resetAllMetrics() {
        root.metricAccumulator.resetAll();
        root.clearLegSampleState();
        root.phaseResults = [];
    }

    function requestPresentedWindow() {
        const targetWindow = root.targetWindow;
        if (root.presentedWindowState && root.presentedWindowState.window !== targetWindow)
            root.restorePresentedWindow();
        const windowFlags = targetWindow ? Number(targetWindow.flags) : Number.NaN;
        if (Number.isFinite(windowFlags)) {
            if (!root.presentedWindowState)
                root.presentedWindowState = {
                    window: targetWindow,
                    flags: windowFlags
                };
            if (!(windowFlags & Qt.WindowStaysOnTopHint))
                targetWindow.flags = windowFlags | Qt.WindowStaysOnTopHint;
        }
        const requestActivation = targetWindow ? targetWindow.requestActivate : null;
        if (typeof requestActivation === "function")
            requestActivation.call(targetWindow);
    }

    function restorePresentedWindow() {
        const presentedWindowState = root.presentedWindowState;
        root.presentedWindowState = null;
        if (presentedWindowState && presentedWindowState.window)
            presentedWindowState.window.flags = presentedWindowState.flags;
    }

    function requestPresentedFrame() {
        const updateWindow = root.targetWindow ? root.targetWindow.update : null;
        if (typeof updateWindow === "function")
            updateWindow.call(root.targetWindow);
    }

    function recordPresentedFrameAt(timestampMilliseconds, contentY, moving, verticalVelocity = Number.NaN) {
        if (!root.legActive)
            return;
        const timestamp = Number(timestampMilliseconds);
        const position = Number(contentY);
        const isMoving = Boolean(moving);
        if (!Number.isFinite(timestamp) || !Number.isFinite(position))
            return;

        root.metricAccumulator.recordPosition(position);
        const observedVelocity = Math.abs(Number(verticalVelocity));
        if (Number.isFinite(observedVelocity)) {
            root.metricAccumulator.recordVelocity(observedVelocity);
            root.legMaximumObservedVelocity = Math.max(root.legMaximumObservedVelocity, observedVelocity);
        }

        if (Number.isFinite(root.previousFrameTimestamp) && Number.isFinite(root.previousContentY) && root.previousMoving) {
            const interval = timestamp - root.previousFrameTimestamp;
            if (interval > 0) {
                const updated = Math.abs(position - root.previousContentY) >= root.movementEpsilon;
                root.metricAccumulator.recordFrame(interval, updated);
            }
        }

        root.previousFrameTimestamp = timestamp;
        root.previousContentY = position;
        root.previousMoving = isMoving;
    }

    function visibleAnchorRow() {
        const anchor = root.targetViewport ? root.targetViewport.captureVisibleAnchor() : null;
        const row = anchor ? Number(anchor.row) : -1;
        return Number.isInteger(row) && row >= 0 ? row : -1;
    }

    function recordLegMeasurement(endRow) {
        const resolvedEndRow = Number(endRow);
        const finalContentY = Number(root.targetViewport ? root.targetViewport.contentY : Number.NaN);
        const travel = Number.isFinite(root.legStartContentY) && Number.isFinite(finalContentY) ? Math.abs(finalContentY - root.legStartContentY) : 0;
        const peakVelocity = Number.isFinite(root.legMaximumObservedVelocity) ? root.legMaximumObservedVelocity : 0;
        const rows = root.legStartRow >= 0 && Number.isInteger(resolvedEndRow) && resolvedEndRow >= 0 ? Math.abs(resolvedEndRow - root.legStartRow) : 0;
        root.recordLegResult(travel, peakVelocity, rows);
    }

    function recordLegResult(travelPixels, peakVelocity, rowTravel) {
        root.metricAccumulator.recordLeg(travelPixels, peakVelocity, rowTravel);
    }

    function recordAnchorCorrection(displacement) {
        if (!root.phaseActive)
            return;
        const absoluteDisplacement = Math.abs(Number(displacement));
        if (!Number.isFinite(absoluteDisplacement) || absoluteDisplacement < root.movementEpsilon)
            return;
        root.metricAccumulator.recordAnchorCorrection(absoluteDisplacement);
    }

    function recordRowGeometryChange(heightDelta) {
        if (!root.phaseActive)
            return;
        const absoluteDelta = Math.abs(Number(heightDelta));
        if (!Number.isFinite(absoluteDelta) || absoluteDelta < root.movementEpsilon)
            return;
        root.metricAccumulator.recordRowGeometryChange(absoluteDelta);
    }

    function recordAnchorExcursion(excursion) {
        if (!root.phaseActive)
            return;
        const absoluteExcursion = Math.abs(Number(excursion));
        if (!Number.isFinite(absoluteExcursion))
            return;
        root.metricAccumulator.recordAnchorExcursion(absoluteExcursion);
    }

    function recordTrajectoryRowMotion(sample) {
        const entryId = String(sample.entryId);
        const row = Number(sample.row);
        const previousOffset = Number(sample.previousOffset);
        const currentOffset = Number(sample.currentOffset);
        const previousContentY = Number(sample.previousContentY);
        const currentContentY = Number(sample.currentContentY);
        const moving = Boolean(sample.moving);
        const velocity = Number(sample.velocity);
        const rowState = sample.rowState ?? null;
        const offsetDelta = Number(currentOffset) - Number(previousOffset);
        const direction = Math.sign(Number(root.activeLegContentDirection));
        const reverseMotion = direction !== 0 && Math.abs(offsetDelta) >= root.movementEpsilon && direction * offsetDelta > 0 ? Math.abs(offsetDelta) : 0;
        const previousRowContentY = Number(previousOffset) + Number(previousContentY);
        const currentRowContentY = Number(currentOffset) + Number(currentContentY);
        const trajectoryGeometryDrift = Boolean(sample.trackContentCoordinate) && Number.isFinite(previousRowContentY) && Number.isFinite(currentRowContentY) ? Math.abs(currentRowContentY - previousRowContentY) : 0;
        const stoppedDrift = Boolean(sample.trackStoppedOffset) && Number.isFinite(previousOffset) && Number.isFinite(currentOffset) ? Math.abs(offsetDelta) : 0;
        let trajectoryMotionEvent = null;
        if (reverseMotion > root.maximumReverseRowMotionPixels || trajectoryGeometryDrift > root.maximumTrajectoryRowGeometryDriftPixels || stoppedDrift > root.maximumStoppedRowDriftPixels) {
            trajectoryMotionEvent = {
                phase: root.phaseIndex >= 0 && root.phaseIndex < root.phaseNames.length ? root.phaseNames[root.phaseIndex] : "",
                kind: String(sample.kind ?? "trajectory-motion"),
                entryId: String(entryId),
                row: Number(row),
                contentY: root.rounded(root.targetViewport.contentY),
                contentDelta: root.rounded(Number(currentContentY) - Number(previousContentY)),
                previousOffset: root.rounded(previousOffset),
                offset: root.rounded(currentOffset),
                offsetDelta: root.rounded(offsetDelta),
                trajectoryGeometryDrift: root.rounded(trajectoryGeometryDrift),
                stoppedDrift: root.rounded(stoppedDrift),
                moving: Boolean(moving),
                velocity: root.rounded(velocity),
                rowHeightRevision: Number(root.targetViewport.rowHeightRevision ?? -1),
                rowHeight: root.rounded(rowState ? rowState.height : 0),
                pendingMeasuredHeight: root.rounded(rowState ? rowState.pendingMeasuredHeight : 0)
            };
        }
        root.metricAccumulator.recordTrajectoryMotion(reverseMotion, trajectoryGeometryDrift, stoppedDrift, trajectoryMotionEvent);
    }

    function recordTrajectoryTrackingGap(reason, moving, velocity, entryId = "", row = -1) {
        const trackingGapEvent = {
            phase: root.phaseIndex >= 0 && root.phaseIndex < root.phaseNames.length ? root.phaseNames[root.phaseIndex] : "",
            kind: "tracking-gap",
            reason: String(reason),
            entryId: String(entryId),
            row: Number(row),
            contentY: root.rounded(root.targetViewport.contentY),
            moving: Boolean(moving),
            velocity: root.rounded(velocity),
            rowHeightRevision: Number(root.targetViewport.rowHeightRevision ?? -1)
        };
        root.metricAccumulator.recordTrackingGap(trackingGapEvent);
    }

    function sampleTrajectoryRowMotion(presentedFrame = false) {
        if (!root.legActive || !root.targetViewport)
            return;
        const moving = Boolean(root.targetViewport.moving);
        const velocity = Math.abs(Number(root.targetViewport.verticalVelocity));
        if (!moving) {
            if (!presentedFrame || root.stoppedTrajectoryFramesSampled >= root.stoppedTrajectoryFrameSampleCount)
                return;
            root.metricAccumulator.recordTrajectoryCoverage({
                stoppedFrameSamples: 1
            });
            if (root.stoppedTrajectoryFramesSampled === 0) {
                const movementEndedAnchorMethod = root.targetViewport.movementEndedAnchorForBenchmark;
                const movementEndedAnchor = typeof movementEndedAnchorMethod === "function" ? movementEndedAnchorMethod.call(root.targetViewport) : null;
                const entryId = movementEndedAnchor ? String(movementEndedAnchor.entryId) : "";
                const lastPresentedOffset = Number(root.previousTrajectoryVisibleRowOffsets[entryId]);
                const lastPresentedContentY = Number(root.previousTrajectoryContentY);
                const movementEndedOffset = movementEndedAnchor ? Number(movementEndedAnchor.offset) : Number.NaN;
                const movementEndedContentY = movementEndedAnchor ? Number(movementEndedAnchor.contentY) : Number.NaN;
                if (movementEndedAnchor && Number.isFinite(lastPresentedOffset) && Number.isFinite(lastPresentedContentY) && Number.isFinite(movementEndedOffset) && Number.isFinite(movementEndedContentY)) {
                    root.metricAccumulator.recordTrajectoryCoverage({
                        rowMotionSamples: 1
                    });
                    root.recordTrajectoryRowMotion({
                        kind: "movement-boundary",
                        entryId: entryId,
                        row: movementEndedAnchor.row,
                        previousOffset: lastPresentedOffset,
                        currentOffset: movementEndedOffset,
                        previousContentY: lastPresentedContentY,
                        currentContentY: movementEndedContentY,
                        moving: false,
                        velocity: velocity,
                        trackContentCoordinate: true
                    });
                } else {
                    root.metricAccumulator.recordTrajectoryCoverage({
                        missingRowFrames: 1
                    });
                    root.recordTrajectoryTrackingGap("movement-end anchor did not survive from the last moving frame", false, velocity, entryId, movementEndedAnchor ? movementEndedAnchor.row : -1);
                }

                const comparisonAnchor = movementEndedAnchor;
                const expectedOffset = comparisonAnchor ? Number(comparisonAnchor.offset) : Number.NaN;
                const previousContentY = comparisonAnchor ? Number(comparisonAnchor.contentY) : Number.NaN;
                const currentOffset = comparisonAnchor ? Number(root.targetViewport.anchorOffsetForBenchmark(comparisonAnchor)) : Number.NaN;
                if (comparisonAnchor && Number.isFinite(expectedOffset) && Number.isFinite(previousContentY) && Number.isFinite(currentOffset)) {
                    root.metricAccumulator.recordTrajectoryCoverage({
                        rowMotionSamples: 1
                    });
                    root.recordTrajectoryRowMotion({
                        kind: "stopped-transaction",
                        entryId: entryId,
                        row: comparisonAnchor.row,
                        previousOffset: expectedOffset,
                        currentOffset: currentOffset,
                        previousContentY: previousContentY,
                        currentContentY: root.targetViewport.contentY,
                        moving: false,
                        velocity: velocity,
                        trackStoppedOffset: true
                    });
                    root.movementEndBaselineAnchor = {
                        entryId: entryId,
                        row: Number(comparisonAnchor.row),
                        offset: expectedOffset,
                        contentY: previousContentY
                    };
                } else {
                    root.metricAccumulator.recordTrajectoryCoverage({
                        missingRowFrames: 1
                    });
                    root.recordTrajectoryTrackingGap("movement-end anchor unavailable", false, velocity, entryId, comparisonAnchor ? comparisonAnchor.row : -1);
                }
            } else {
                const transactionAnchor = root.movementEndBaselineAnchor;
                const entryId = transactionAnchor ? String(transactionAnchor.entryId) : "";
                const expectedOffset = transactionAnchor ? Number(transactionAnchor.offset) : Number.NaN;
                const expectedContentY = transactionAnchor ? Number(transactionAnchor.contentY) : Number.NaN;
                const currentOffset = transactionAnchor ? Number(root.targetViewport.anchorOffsetForBenchmark(transactionAnchor)) : Number.NaN;
                if (transactionAnchor && Number.isFinite(expectedOffset) && Number.isFinite(expectedContentY) && Number.isFinite(currentOffset)) {
                    root.metricAccumulator.recordTrajectoryCoverage({
                        rowMotionSamples: 1
                    });
                    root.recordTrajectoryRowMotion({
                        kind: "stopped-transaction",
                        entryId: entryId,
                        row: transactionAnchor.row,
                        previousOffset: expectedOffset,
                        currentOffset: currentOffset,
                        previousContentY: expectedContentY,
                        currentContentY: root.targetViewport.contentY,
                        moving: false,
                        velocity: velocity,
                        trackStoppedOffset: true
                    });
                } else {
                    root.metricAccumulator.recordTrajectoryCoverage({
                        missingRowFrames: 1
                    });
                    root.recordTrajectoryTrackingGap("stopped transaction anchor unavailable", false, velocity, entryId, transactionAnchor ? transactionAnchor.row : -1);
                }
            }
            ++root.stoppedTrajectoryFramesSampled;
            if (root.stoppedTrajectoryFramesSampled >= root.stoppedTrajectoryFrameSampleCount) {
                root.movementEndBaselineAnchor = null;
            }
            return;
        }
        const trajectoryRowsMethod = root.targetViewport.trajectoryRowOffsetsForBenchmark;
        const visibleRowsMethod = root.targetViewport.visibleRowOffsetsForBenchmark;
        let visibleRows = typeof trajectoryRowsMethod === "function" ? trajectoryRowsMethod.call(root.targetViewport) : [];
        if ((!Array.isArray(visibleRows) || visibleRows.length === 0) && typeof visibleRowsMethod === "function")
            visibleRows = visibleRowsMethod.call(root.targetViewport);
        if (!Array.isArray(visibleRows) || visibleRows.length === 0) {
            const anchor = root.targetViewport.captureVisibleAnchor();
            const offset = anchor ? Number(root.targetViewport.anchorOffsetForBenchmark(anchor)) : Number.NaN;
            visibleRows = anchor && Number.isFinite(offset) ? [
                {
                    entryId: String(anchor.entryId),
                    row: Number(anchor.row),
                    offset: offset
                }
            ] : [];
        }
        if (visibleRows.length === 0) {
            root.metricAccumulator.recordTrajectoryCoverage({
                missingRowFrames: 1
            });
            root.recordTrajectoryTrackingGap("no trajectory rows", true, velocity);
            root.previousTrajectoryVisibleRowOffsets = {};
            root.previousTrajectoryContentY = Number.NaN;
            return;
        }
        const currentOffsets = {};
        const currentRows = {};
        for (const visibleRow of visibleRows) {
            const entryId = String(visibleRow.entryId ?? "");
            const offset = Number(visibleRow.offset);
            if (entryId.length > 0 && Number.isFinite(offset)) {
                currentOffsets[entryId] = offset;
                currentRows[entryId] = visibleRow;
            }
        }
        const currentEntryIds = Object.keys(currentOffsets);
        if (currentEntryIds.length === 0) {
            root.metricAccumulator.recordTrajectoryCoverage({
                missingRowFrames: 1
            });
            root.recordTrajectoryTrackingGap("no valid trajectory rows", true, velocity);
            root.previousTrajectoryVisibleRowOffsets = {};
            root.previousTrajectoryContentY = Number.NaN;
            return;
        }
        const previousOffsets = root.previousTrajectoryVisibleRowOffsets;
        if (Object.keys(previousOffsets).length > 0 && Number.isFinite(root.previousTrajectoryContentY)) {
            let commonRowCount = 0;
            for (const entryId of currentEntryIds) {
                const previousOffset = Number(previousOffsets[entryId]);
                if (!Number.isFinite(previousOffset))
                    continue;
                ++commonRowCount;
                root.recordTrajectoryRowMotion({
                    entryId: entryId,
                    row: currentRows[entryId].row,
                    previousOffset: previousOffset,
                    currentOffset: currentOffsets[entryId],
                    previousContentY: root.previousTrajectoryContentY,
                    currentContentY: root.targetViewport.contentY,
                    moving: true,
                    velocity: velocity,
                    rowState: currentRows[entryId],
                    trackContentCoordinate: true
                });
            }
            if (commonRowCount === 0) {
                root.metricAccumulator.recordTrajectoryCoverage({
                    missingRowFrames: 1
                });
                root.recordTrajectoryTrackingGap("no row survived between frames", true, velocity);
            } else {
                root.metricAccumulator.recordTrajectoryCoverage({
                    rowMotionSamples: 1
                });
            }
        }
        root.previousTrajectoryVisibleRowOffsets = currentOffsets;
        root.previousTrajectoryContentY = Number(root.targetViewport.contentY);
    }

    function beginSettlementSampling() {
        const anchor = root.targetViewport.captureVisibleAnchor();
        if (!anchor) {
            root.settlementSamplingActive = false;
            root.settlementReferenceAnchor = null;
            return;
        }
        root.settlementReferenceAnchor = {
            entryId: String(anchor.entryId),
            offset: Number(anchor.offset),
            row: Number(anchor.row)
        };
        root.settlementSamplingActive = true;
        root.sampleSettlementAnchor();
    }

    function sampleSettlementAnchor() {
        if (!root.settlementSamplingActive || !root.settlementReferenceAnchor)
            return;
        const currentOffset = Number(root.targetViewport.anchorOffsetForBenchmark(root.settlementReferenceAnchor));
        if (Number.isFinite(currentOffset))
            root.recordAnchorExcursion(currentOffset - Number(root.settlementReferenceAnchor.offset));
    }

    function endSettlementSampling() {
        root.sampleSettlementAnchor();
        root.settlementSamplingActive = false;
        root.settlementReferenceAnchor = null;
    }

    function currentPhaseMetrics() {
        return root.metricAccumulator.phaseSnapshot(root.frameBudgetMilliseconds);
    }

    function allMetrics() {
        return root.metricAccumulator.aggregateSnapshot(root.frameBudgetMilliseconds);
    }

    function evaluateMetrics(metrics, phaseName = "") {
        const failures = [];
        if (metrics.frameSampleCount < root.minimumFrameSampleCount)
            failures.push("insufficient presented-frame samples");
        if (metrics.motionSampleCount < root.minimumMotionSampleCount)
            failures.push("insufficient motion samples");
        if (metrics.motionUpdateRatio < root.minimumMotionUpdateRatio)
            failures.push("motion update ratio below threshold");
        if (metrics.longestFrozenMotionStreak > root.maximumFrozenMotionStreak)
            failures.push("frozen-motion streak above threshold");
        if (metrics.p95FrameBudgetRatio > root.maximumP95FrameBudgetRatio)
            failures.push("p95 frame time above threshold");
        if (metrics.p99FrameBudgetRatio > root.maximumP99FrameBudgetRatio)
            failures.push("p99 frame time above threshold");
        if (metrics.worstFrameBudgetRatio > root.maximumWorstFrameBudgetRatio)
            failures.push("worst frame time above threshold");
        if (metrics.maximumAnchorExcursionPixels > root.maximumAnchorExcursionPixels)
            failures.push("post-scroll anchor excursion above threshold");
        if (metrics.maximumReverseRowMotionPixels > root.maximumReverseRowMotionPixels)
            failures.push("reverse row motion during flick trajectory");
        if (metrics.maximumTrajectoryRowGeometryDriftPixels > root.maximumTrajectoryRowGeometryDriftPixels)
            failures.push("row geometry drifted during flick trajectory");
        if (metrics.maximumStoppedRowDriftPixels > root.maximumStoppedRowDriftPixels)
            failures.push("row drifted after flick stopped");
        if (metrics.trajectoryRowMotionSampleCount < root.minimumTrajectoryRowMotionSampleCount)
            failures.push("insufficient flick-trajectory row-motion samples");
        if (metrics.stoppedTrajectoryFrameSampleCount < root.minimumStoppedTrajectoryFrameSampleCount)
            failures.push("insufficient stopped-frame row-motion samples");
        if (metrics.missingTrajectoryRowFrameCount > root.maximumMissingTrajectoryRowFrameCount)
            failures.push("tracked flick-trajectory row was unavailable");
        if (String(phaseName).startsWith("warm-") && metrics.rowGeometryChangeCount > root.maximumWarmRowGeometryChangeCount)
            failures.push("warm trajectory changed row geometry");
        if (metrics.minimumLegTravelPixels < root.minimumFlickTravelPixels)
            failures.push("flick travel below threshold");
        if (metrics.minimumLegPeakVelocity < root.minimumObservedFlickVelocity)
            failures.push("flick velocity below threshold");
        if (metrics.minimumLegRowTravel < root.minimumFlickRowTravel)
            failures.push("flick row travel below threshold");
        if (metrics.legMeasurementCount !== root.requiredLegMeasurementCount)
            failures.push("incomplete flick leg measurements");
        return failures;
    }

    function buildPhaseResult(name) {
        const metrics = root.currentPhaseMetrics();
        const failures = root.evaluateMetrics(metrics, name);
        return {
            name: String(name),
            passed: failures.length === 0,
            failures: failures,
            metrics: metrics
        };
    }

    function stopTimers() {
        readinessTimer.stop();
        phasePreparationTimer.stop();
        legTimer.stop();
        legCompletionTimer.stop();
        legPauseTimer.stop();
        phaseSettlementTimer.stop();
    }

    function finish(benchmarkResult) {
        if (root.resultEmitted)
            return;
        root.stopTimers();
        root.legActive = false;
        root.endSettlementSampling();
        root.phaseActive = false;
        root.state = "finished";
        root.resultEmitted = true;
        root.restorePresentedWindow();
        root.finished(benchmarkResult);
    }

    function failBeforeMeasurement(failure) {
        root.finish({
            schemaVersion: 1,
            benchmark: "timeline-render",
            renderer: root.rendererName,
            requestedThreadId: root.requestedThreadId,
            selectedThreadId: root.selectedThreadId,
            passed: false,
            failures: [String(failure)],
            frameBudgetMilliseconds: root.rounded(root.frameBudgetMilliseconds),
            displayRefreshRate: root.rounded(root.displayRefreshRate),
            thresholds: root.thresholdsSnapshot(),
            phases: []
        });
    }

    function beginBenchmark() {
        root.stopTimers();
        root.requestPresentedWindow();
        root.resultEmitted = false;
        root.phaseActive = false;
        root.legActive = false;
        root.settlementSamplingActive = false;
        root.phaseIndex = -1;
        root.legIndex = 0;
        root.resetAllMetrics();
        root.benchmarkStartedAt = Date.now();
        root.readinessStableSince = Number.NaN;
        root.lastReadinessContentHeight = Number.NaN;
        root.readinessMilliseconds = 0;
        root.state = "waiting-for-content";
        if (root.rendererName !== "current") {
            root.failBeforeMeasurement("unsupported renderer adapter: " + root.rendererName);
            return;
        }
        if (root.requestedThreadId.length === 0) {
            root.failBeforeMeasurement("no benchmark thread was requested");
            return;
        }
        readinessTimer.start();
        root.pollReadiness();
    }

    function readinessConditionsMet() {
        if (!root.targetViewport || !root.targetWindow)
            return false;
        if (root.selectedThreadId !== root.requestedThreadId || root.conversationLoading || root.rowCount <= 0)
            return false;
        const viewportHeight = Number(root.targetViewport.viewportHeight);
        const scrollRange = Number(root.targetViewport.maximumContentY) - Number(root.targetViewport.minimumContentY);
        return Number.isFinite(viewportHeight) && viewportHeight > 0 && Number.isFinite(scrollRange) && scrollRange >= viewportHeight * root.minimumScrollableViewportCount;
    }

    function pollReadiness() {
        if (!root.active || root.resultEmitted)
            return;
        const now = Date.now();
        if (now - root.benchmarkStartedAt >= root.readinessTimeoutMilliseconds) {
            root.failBeforeMeasurement("conversation did not become benchmark-ready before the timeout");
            return;
        }
        if (!root.readinessConditionsMet()) {
            root.readinessStableSince = Number.NaN;
            root.lastReadinessContentHeight = Number.NaN;
            return;
        }

        const contentHeight = Number(root.targetViewport.scrollContentHeight);
        if (!Number.isFinite(root.lastReadinessContentHeight) || Math.abs(contentHeight - root.lastReadinessContentHeight) >= 0.5) {
            root.lastReadinessContentHeight = contentHeight;
            root.readinessStableSince = now;
            return;
        }
        if (!Number.isFinite(root.readinessStableSince))
            root.readinessStableSince = now;
        if (now - root.readinessStableSince < root.readinessStabilityMilliseconds)
            return;

        readinessTimer.stop();
        root.readinessMilliseconds = now - root.benchmarkStartedAt;
        root.phaseIndex = 0;
        root.preparePhase();
    }

    function preparePhase() {
        if (root.phaseIndex >= root.phaseNames.length) {
            root.completeBenchmark();
            return;
        }
        root.state = "preparing-" + root.phaseNames[root.phaseIndex];
        root.phaseActive = false;
        root.legActive = false;
        root.endSettlementSampling();
        root.targetViewport.cancelFlickForBenchmark();
        root.targetViewport.followLiveTail = false;
        root.targetViewport.positionAtContentY(root.targetViewport.maximumContentY);
        root.phaseStartContentY = root.targetViewport.contentY;
        phasePreparationTimer.restart();
    }

    function beginPreparedPhase() {
        root.resetPhaseMetrics();
        root.phaseActive = true;
        root.legIndex = 0;
        root.state = "running-" + root.phaseNames[root.phaseIndex];
        root.beginNextLeg();
    }

    function beginNextLeg() {
        if (root.legIndex >= root.legDirections.length) {
            root.state = "settling-" + root.phaseNames[root.phaseIndex];
            phaseSettlementTimer.restart();
            return;
        }
        root.endSettlementSampling();
        root.clearLegSampleState();
        root.activeLegContentDirection = Math.sign(Number(root.legDirections[root.legIndex]));
        root.legActive = true;
        root.legStartRow = root.visibleAnchorRow();
        root.legStartContentY = Number(root.targetViewport.contentY);
        root.legMaximumObservedVelocity = 0;
        root.requestPresentedWindow();
        root.targetViewport.flickContentForBenchmark(root.legDirections[root.legIndex] * root.flickContentVelocity, root.flickDeceleration);
        legTimer.restart();
    }

    function requestEndCurrentLeg() {
        if (!root.legActive)
            return;
        root.legEndRequested = true;
        root.targetViewport.cancelFlickForBenchmark();
        if (root.stoppedTrajectoryFramesSampled >= root.stoppedTrajectoryFrameSampleCount) {
            root.endCurrentLeg();
            return;
        }
        root.requestPresentedFrame();
        legCompletionTimer.restart();
    }

    function recordCurrentPresentedFrame() {
        if (!root.legActive)
            return;
        root.recordPresentedFrameAt(Date.now(), root.targetViewport.contentY, root.targetViewport.moving, root.targetViewport.verticalVelocity);
        root.sampleTrajectoryRowMotion(true);
        if (!root.legEndRequested)
            return;
        if (root.stoppedTrajectoryFramesSampled >= root.stoppedTrajectoryFrameSampleCount)
            root.endCurrentLeg();
        else
            root.requestPresentedFrame();
    }

    function endCurrentLeg() {
        if (!root.legActive)
            return;
        legTimer.stop();
        legCompletionTimer.stop();
        root.targetViewport.cancelFlickForBenchmark();
        root.recordLegMeasurement(root.visibleAnchorRow());
        root.legActive = false;
        root.clearLegSampleState();
        root.beginSettlementSampling();
        ++root.legIndex;
        if (root.legIndex >= root.legDirections.length) {
            root.state = "settling-" + root.phaseNames[root.phaseIndex];
            phaseSettlementTimer.restart();
        } else {
            legPauseTimer.restart();
        }
    }

    function finishCurrentPhase() {
        root.endSettlementSampling();
        root.phaseActive = false;
        const phaseResult = root.buildPhaseResult(root.phaseNames[root.phaseIndex]);
        root.phaseResults = root.phaseResults.concat([phaseResult]);
        ++root.phaseIndex;
        root.preparePhase();
    }

    function completeBenchmark() {
        const failures = [];
        for (const phaseResult of root.phaseResults) {
            for (const phaseFailure of phaseResult.failures)
                failures.push(phaseResult.name + ": " + phaseFailure);
        }
        root.finish({
            schemaVersion: 1,
            benchmark: "timeline-render",
            renderer: root.rendererName,
            requestedThreadId: root.requestedThreadId,
            selectedThreadId: root.selectedThreadId,
            rowCount: root.rowCount,
            contentHeight: root.rounded(root.targetViewport.scrollContentHeight),
            viewportHeight: root.rounded(root.targetViewport.viewportHeight),
            activeRowSlotCount: Number(root.targetViewport.activeRowSlotCount),
            readinessMilliseconds: root.rounded(root.readinessMilliseconds),
            frameBudgetMilliseconds: root.rounded(root.frameBudgetMilliseconds),
            displayRefreshRate: root.rounded(root.displayRefreshRate),
            trajectory: {
                phaseNames: root.phaseNames,
                legDirections: root.legDirections,
                flickContentVelocity: root.flickContentVelocity,
                flickDeceleration: root.flickDeceleration,
                stoppedTrajectoryFrameSampleCount: root.stoppedTrajectoryFrameSampleCount,
                legDurationMilliseconds: root.legDurationMilliseconds,
                legCompletionTimeoutMilliseconds: root.legCompletionTimeoutMilliseconds,
                legPauseMilliseconds: root.legPauseMilliseconds,
                phaseSettlementMilliseconds: root.phaseSettlementMilliseconds
            },
            passed: failures.length === 0,
            failures: failures,
            thresholds: root.thresholdsSnapshot(),
            metrics: root.allMetrics(),
            phases: root.phaseResults
        });
    }

    property Timer readinessTimer: Timer {
        interval: 50
        repeat: true
        onTriggered: root.pollReadiness()
    }

    property Timer phasePreparationTimer: Timer {
        interval: root.phasePreparationMilliseconds
        onTriggered: root.beginPreparedPhase()
    }

    property Timer legTimer: Timer {
        interval: root.legDurationMilliseconds
        onTriggered: root.requestEndCurrentLeg()
    }

    property Timer legCompletionTimer: Timer {
        interval: root.legCompletionTimeoutMilliseconds
        onTriggered: root.endCurrentLeg()
    }

    property Timer legPauseTimer: Timer {
        interval: root.legPauseMilliseconds
        onTriggered: root.beginNextLeg()
    }

    property Timer phaseSettlementTimer: Timer {
        interval: root.phaseSettlementMilliseconds
        onTriggered: root.finishCurrentPhase()
    }

    property Connections frameConnections: Connections {
        target: root.targetWindow
        enabled: root.active && (root.legActive || root.settlementSamplingActive) && root.targetWindow !== null
        ignoreUnknownSignals: true

        function onFrameSwapped() {
            if (root.legActive)
                root.recordCurrentPresentedFrame();
            if (root.settlementSamplingActive)
                root.sampleSettlementAnchor();
        }
    }

    property Connections viewportConnections: Connections {
        target: root.targetViewport
        enabled: root.active && root.phaseActive && root.targetViewport !== null
        ignoreUnknownSignals: true

        function onAnchorPositionCorrected(displacement) {
            root.recordAnchorCorrection(displacement);
        }

        function onRowGeometryChanged(sourceRow, heightDelta) {
            root.recordRowGeometryChange(heightDelta);
        }
    }

    onActiveChanged: {
        if (!root.componentCompleted)
            return;
        if (root.active)
            root.beginBenchmark();
        else {
            root.stopTimers();
            root.legActive = false;
            root.endSettlementSampling();
            root.phaseActive = false;
            root.restorePresentedWindow();
            root.state = "idle";
        }
    }

    Component.onCompleted: {
        root.componentCompleted = true;
        if (root.active)
            root.beginBenchmark();
    }
    Component.onDestruction: root.restorePresentedWindow()
}
