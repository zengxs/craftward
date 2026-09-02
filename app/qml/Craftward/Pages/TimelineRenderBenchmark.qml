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
    property int legDurationMilliseconds: 550
    property int legPauseMilliseconds: 120
    property int phaseSettlementMilliseconds: 500
    property real flickContentVelocity: 7000
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
    property int maximumWarmRowGeometryChangeCount: 0
    readonly property real displayRefreshRate: frameBudgetMilliseconds > 0 ? 1000 / frameBudgetMilliseconds : 0
    readonly property var phaseNames: ["cold", "warm-1", "warm-2"]
    readonly property var legDirections: [-1, -1, 1, -1, -1, 1]
    property string state: "idle"
    property bool componentCompleted: false
    property bool resultEmitted: false
    property bool phaseActive: false
    property bool legActive: false
    property bool settlementSamplingActive: false
    property int phaseIndex: -1
    property int legIndex: 0
    property real benchmarkStartedAt: Number.NaN
    property real readinessStableSince: Number.NaN
    property real lastReadinessContentHeight: Number.NaN
    property real readinessMilliseconds: 0
    property real phaseStartContentY: 0
    property real previousFrameTimestamp: Number.NaN
    property real previousContentY: Number.NaN
    property bool previousMoving: false
    property var frameIntervals: []
    property var motionSamples: []
    property int anchorCorrectionCount: 0
    property real maximumAnchorCorrectionPixels: 0
    property int rowGeometryChangeCount: 0
    property real maximumRowGeometryChangePixels: 0
    property real maximumObservedAnchorExcursionPixels: 0
    property var settlementReferenceAnchor: null
    property var allFrameIntervals: []
    property var allMotionSamples: []
    property int allAnchorCorrectionCount: 0
    property real allMaximumAnchorCorrectionPixels: 0
    property int allRowGeometryChangeCount: 0
    property real allMaximumRowGeometryChangePixels: 0
    property real allMaximumObservedAnchorExcursionPixels: 0
    property var phaseResults: []

    signal finished(var benchmarkResult)

    function rounded(value, decimalPlaces = 3) {
        const numericValue = Number(value);
        if (!Number.isFinite(numericValue))
            return 0;
        const scale = Math.pow(10, decimalPlaces);
        return Math.round(numericValue * scale) / scale;
    }

    function percentile(sortedValues, fraction) {
        if (sortedValues.length === 0)
            return 0;
        const index = Math.max(0, Math.min(sortedValues.length - 1, Math.ceil(fraction * sortedValues.length) - 1));
        return Number(sortedValues[index]);
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
            maximumWarmRowGeometryChangeCount: root.maximumWarmRowGeometryChangeCount
        };
    }

    function clearLegSampleState() {
        root.previousFrameTimestamp = Number.NaN;
        root.previousContentY = Number.NaN;
        root.previousMoving = false;
    }

    function resetPhaseMetrics() {
        root.frameIntervals = [];
        root.motionSamples = [];
        root.anchorCorrectionCount = 0;
        root.maximumAnchorCorrectionPixels = 0;
        root.rowGeometryChangeCount = 0;
        root.maximumRowGeometryChangePixels = 0;
        root.maximumObservedAnchorExcursionPixels = 0;
        root.clearLegSampleState();
    }

    function resetAllMetrics() {
        root.resetPhaseMetrics();
        root.allFrameIntervals = [];
        root.allMotionSamples = [];
        root.allAnchorCorrectionCount = 0;
        root.allMaximumAnchorCorrectionPixels = 0;
        root.allRowGeometryChangeCount = 0;
        root.allMaximumRowGeometryChangePixels = 0;
        root.allMaximumObservedAnchorExcursionPixels = 0;
        root.phaseResults = [];
    }

    function recordPresentedFrameAt(timestampMilliseconds, contentY, moving) {
        if (!root.legActive)
            return;
        const timestamp = Number(timestampMilliseconds);
        const position = Number(contentY);
        const isMoving = Boolean(moving);
        if (!Number.isFinite(timestamp) || !Number.isFinite(position))
            return;

        if (Number.isFinite(root.previousFrameTimestamp) && Number.isFinite(root.previousContentY) && root.previousMoving) {
            const interval = timestamp - root.previousFrameTimestamp;
            if (interval > 0) {
                const updated = Math.abs(position - root.previousContentY) >= root.movementEpsilon;
                const sample = {
                    interval: interval,
                    updated: updated
                };
                root.frameIntervals.push(interval);
                root.motionSamples.push(sample);
                root.allFrameIntervals.push(interval);
                root.allMotionSamples.push(sample);
            }
        }

        root.previousFrameTimestamp = timestamp;
        root.previousContentY = position;
        root.previousMoving = isMoving;
    }

    function recordAnchorCorrection(displacement) {
        if (!root.phaseActive)
            return;
        const absoluteDisplacement = Math.abs(Number(displacement));
        if (!Number.isFinite(absoluteDisplacement) || absoluteDisplacement < root.movementEpsilon)
            return;
        ++root.anchorCorrectionCount;
        ++root.allAnchorCorrectionCount;
        root.maximumAnchorCorrectionPixels = Math.max(root.maximumAnchorCorrectionPixels, absoluteDisplacement);
        root.allMaximumAnchorCorrectionPixels = Math.max(root.allMaximumAnchorCorrectionPixels, absoluteDisplacement);
    }

    function recordRowGeometryChange(heightDelta) {
        if (!root.phaseActive)
            return;
        const absoluteDelta = Math.abs(Number(heightDelta));
        if (!Number.isFinite(absoluteDelta) || absoluteDelta < root.movementEpsilon)
            return;
        ++root.rowGeometryChangeCount;
        ++root.allRowGeometryChangeCount;
        root.maximumRowGeometryChangePixels = Math.max(root.maximumRowGeometryChangePixels, absoluteDelta);
        root.allMaximumRowGeometryChangePixels = Math.max(root.allMaximumRowGeometryChangePixels, absoluteDelta);
    }

    function recordAnchorExcursion(excursion) {
        if (!root.phaseActive)
            return;
        const absoluteExcursion = Math.abs(Number(excursion));
        if (!Number.isFinite(absoluteExcursion))
            return;
        root.maximumObservedAnchorExcursionPixels = Math.max(root.maximumObservedAnchorExcursionPixels, absoluteExcursion);
        root.allMaximumObservedAnchorExcursionPixels = Math.max(root.allMaximumObservedAnchorExcursionPixels, absoluteExcursion);
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

    function metricsFor(intervals, samples, correctionCount, maximumCorrection, geometryChangeCount, maximumGeometryChange, maximumAnchorExcursion) {
        const sortedIntervals = intervals.slice().sort((left, right) => left - right);
        let totalMilliseconds = 0;
        let lateFrameCount = 0;
        let severeFrameCount = 0;
        let missedVsyncCount = 0;
        for (const intervalValue of intervals) {
            const interval = Number(intervalValue);
            totalMilliseconds += interval;
            if (interval > root.frameBudgetMilliseconds)
                ++lateFrameCount;
            if (interval > root.frameBudgetMilliseconds * 2)
                ++severeFrameCount;
            if (root.frameBudgetMilliseconds > 0)
                missedVsyncCount += Math.max(0, Math.round(interval / root.frameBudgetMilliseconds) - 1);
        }

        let updateCount = 0;
        let currentFrozenStreak = 0;
        let longestFrozenStreak = 0;
        for (const sample of samples) {
            if (sample.updated) {
                ++updateCount;
                currentFrozenStreak = 0;
            } else {
                ++currentFrozenStreak;
                longestFrozenStreak = Math.max(longestFrozenStreak, currentFrozenStreak);
            }
        }

        const p95FrameMilliseconds = root.percentile(sortedIntervals, 0.95);
        const p99FrameMilliseconds = root.percentile(sortedIntervals, 0.99);
        const worstFrameMilliseconds = sortedIntervals.length > 0 ? sortedIntervals[sortedIntervals.length - 1] : 0;
        return {
            frameSampleCount: intervals.length,
            motionSampleCount: samples.length,
            motionUpdateCount: updateCount,
            frozenMotionFrameCount: samples.length - updateCount,
            longestFrozenMotionStreak: longestFrozenStreak,
            motionUpdateRatio: root.rounded(samples.length > 0 ? updateCount / samples.length : 0),
            presentedFramesPerSecond: root.rounded(totalMilliseconds > 0 ? 1000 * intervals.length / totalMilliseconds : 0),
            motionFramesPerSecond: root.rounded(totalMilliseconds > 0 ? 1000 * updateCount / totalMilliseconds : 0),
            p95FrameMilliseconds: root.rounded(p95FrameMilliseconds),
            p99FrameMilliseconds: root.rounded(p99FrameMilliseconds),
            worstFrameMilliseconds: root.rounded(worstFrameMilliseconds),
            p95FrameBudgetRatio: root.rounded(root.frameBudgetMilliseconds > 0 ? p95FrameMilliseconds / root.frameBudgetMilliseconds : 0),
            p99FrameBudgetRatio: root.rounded(root.frameBudgetMilliseconds > 0 ? p99FrameMilliseconds / root.frameBudgetMilliseconds : 0),
            worstFrameBudgetRatio: root.rounded(root.frameBudgetMilliseconds > 0 ? worstFrameMilliseconds / root.frameBudgetMilliseconds : 0),
            lateFrameCount: lateFrameCount,
            severeFrameCount: severeFrameCount,
            missedVsyncCount: missedVsyncCount,
            anchorCorrectionCount: correctionCount,
            maximumAnchorCorrectionPixels: root.rounded(maximumCorrection),
            maximumAnchorExcursionPixels: root.rounded(maximumAnchorExcursion),
            rowGeometryChangeCount: geometryChangeCount,
            maximumRowGeometryChangePixels: root.rounded(maximumGeometryChange)
        };
    }

    function currentPhaseMetrics() {
        return root.metricsFor(root.frameIntervals, root.motionSamples, root.anchorCorrectionCount, root.maximumAnchorCorrectionPixels, root.rowGeometryChangeCount, root.maximumRowGeometryChangePixels, root.maximumObservedAnchorExcursionPixels);
    }

    function allMetrics() {
        return root.metricsFor(root.allFrameIntervals, root.allMotionSamples, root.allAnchorCorrectionCount, root.allMaximumAnchorCorrectionPixels, root.allRowGeometryChangeCount, root.allMaximumRowGeometryChangePixels, root.allMaximumObservedAnchorExcursionPixels);
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
        if (String(phaseName).startsWith("warm-") && metrics.rowGeometryChangeCount > root.maximumWarmRowGeometryChangeCount)
            failures.push("warm trajectory changed row geometry");
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
        if (root.phaseIndex === 0) {
            root.targetViewport.positionAtContentY(root.targetViewport.maximumContentY);
            root.phaseStartContentY = root.targetViewport.contentY;
        } else {
            root.targetViewport.positionAtContentY(Math.min(root.phaseStartContentY, root.targetViewport.maximumContentY));
        }
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
        root.legActive = true;
        root.targetViewport.flickContentForBenchmark(root.legDirections[root.legIndex] * root.flickContentVelocity);
        legTimer.restart();
    }

    function endCurrentLeg() {
        if (!root.legActive)
            return;
        root.recordPresentedFrameAt(Date.now(), root.targetViewport.contentY, root.targetViewport.moving);
        root.targetViewport.cancelFlickForBenchmark();
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
                legDurationMilliseconds: root.legDurationMilliseconds,
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
                root.recordPresentedFrameAt(Date.now(), root.targetViewport.contentY, root.targetViewport.moving);
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
            root.state = "idle";
        }
    }

    Component.onCompleted: {
        root.componentCompleted = true;
        if (root.active)
            root.beginBenchmark();
    }
}
