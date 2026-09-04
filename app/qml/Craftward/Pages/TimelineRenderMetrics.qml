// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    id: root

    property var phaseAccumulator: createAccumulator()
    property var aggregateAccumulator: createAccumulator()

    function createAccumulator() {
        return {
            intervals: [],
            samples: [],
            minimumContentY: Number.NaN,
            maximumContentY: Number.NaN,
            maximumVelocity: 0,
            legTravels: [],
            legPeakVelocities: [],
            rowTravels: [],
            correctionCount: 0,
            maximumCorrection: 0,
            geometryChangeCount: 0,
            maximumGeometryChange: 0,
            maximumAnchorExcursion: 0,
            trajectoryMotionSampleCount: 0,
            stoppedTrajectoryFrameSampleCount: 0,
            maximumReverseRowMotion: 0,
            maximumTrajectoryRowGeometryDrift: 0,
            maximumStoppedRowDrift: 0,
            missingTrajectoryRowFrames: 0,
            trajectoryMotionTrace: []
        };
    }

    function resetPhase() {
        root.phaseAccumulator = root.createAccumulator();
    }

    function resetAll() {
        root.phaseAccumulator = root.createAccumulator();
        root.aggregateAccumulator = root.createAccumulator();
    }

    function recordPositionIn(accumulator, position) {
        accumulator.minimumContentY = Number.isFinite(accumulator.minimumContentY) ? Math.min(accumulator.minimumContentY, position) : position;
        accumulator.maximumContentY = Number.isFinite(accumulator.maximumContentY) ? Math.max(accumulator.maximumContentY, position) : position;
    }

    function recordPosition(position) {
        const numericPosition = Number(position);
        if (!Number.isFinite(numericPosition))
            return;
        root.recordPositionIn(root.phaseAccumulator, numericPosition);
        root.recordPositionIn(root.aggregateAccumulator, numericPosition);
    }

    function recordVelocityIn(accumulator, velocity) {
        accumulator.maximumVelocity = Math.max(accumulator.maximumVelocity, velocity);
    }

    function recordVelocity(velocity) {
        const absoluteVelocity = Math.abs(Number(velocity));
        if (!Number.isFinite(absoluteVelocity))
            return;
        root.recordVelocityIn(root.phaseAccumulator, absoluteVelocity);
        root.recordVelocityIn(root.aggregateAccumulator, absoluteVelocity);
    }

    function recordFrameIn(accumulator, interval, updated) {
        accumulator.intervals.push(interval);
        accumulator.samples.push({
            interval: interval,
            updated: updated
        });
    }

    function recordFrame(interval, updated) {
        const numericInterval = Number(interval);
        if (!Number.isFinite(numericInterval) || numericInterval <= 0)
            return;
        root.recordFrameIn(root.phaseAccumulator, numericInterval, Boolean(updated));
        root.recordFrameIn(root.aggregateAccumulator, numericInterval, Boolean(updated));
    }

    function recordLegIn(accumulator, travelPixels, peakVelocity, rowTravel) {
        accumulator.legTravels.push(Number(travelPixels));
        accumulator.legPeakVelocities.push(Number(peakVelocity));
        accumulator.rowTravels.push(Number(rowTravel));
    }

    function recordLeg(travelPixels, peakVelocity, rowTravel) {
        root.recordLegIn(root.phaseAccumulator, travelPixels, peakVelocity, rowTravel);
        root.recordLegIn(root.aggregateAccumulator, travelPixels, peakVelocity, rowTravel);
    }

    function recordAnchorCorrectionIn(accumulator, displacement) {
        ++accumulator.correctionCount;
        accumulator.maximumCorrection = Math.max(accumulator.maximumCorrection, displacement);
    }

    function recordAnchorCorrection(displacement) {
        const absoluteDisplacement = Math.abs(Number(displacement));
        if (!Number.isFinite(absoluteDisplacement))
            return;
        root.recordAnchorCorrectionIn(root.phaseAccumulator, absoluteDisplacement);
        root.recordAnchorCorrectionIn(root.aggregateAccumulator, absoluteDisplacement);
    }

    function recordRowGeometryChangeIn(accumulator, heightDelta) {
        ++accumulator.geometryChangeCount;
        accumulator.maximumGeometryChange = Math.max(accumulator.maximumGeometryChange, heightDelta);
    }

    function recordRowGeometryChange(heightDelta) {
        const absoluteDelta = Math.abs(Number(heightDelta));
        if (!Number.isFinite(absoluteDelta))
            return;
        root.recordRowGeometryChangeIn(root.phaseAccumulator, absoluteDelta);
        root.recordRowGeometryChangeIn(root.aggregateAccumulator, absoluteDelta);
    }

    function recordAnchorExcursionIn(accumulator, excursion) {
        accumulator.maximumAnchorExcursion = Math.max(accumulator.maximumAnchorExcursion, excursion);
    }

    function recordAnchorExcursion(excursion) {
        const absoluteExcursion = Math.abs(Number(excursion));
        if (!Number.isFinite(absoluteExcursion))
            return;
        root.recordAnchorExcursionIn(root.phaseAccumulator, absoluteExcursion);
        root.recordAnchorExcursionIn(root.aggregateAccumulator, absoluteExcursion);
    }

    function appendTraceEvent(accumulator, event, limit) {
        if (event && accumulator.trajectoryMotionTrace.length < limit)
            accumulator.trajectoryMotionTrace.push(event);
    }

    function recordTrajectoryMotionIn(accumulator, reverseMotion, geometryDrift, stoppedDrift, event, traceLimit) {
        accumulator.maximumReverseRowMotion = Math.max(accumulator.maximumReverseRowMotion, reverseMotion);
        accumulator.maximumTrajectoryRowGeometryDrift = Math.max(accumulator.maximumTrajectoryRowGeometryDrift, geometryDrift);
        accumulator.maximumStoppedRowDrift = Math.max(accumulator.maximumStoppedRowDrift, stoppedDrift);
        root.appendTraceEvent(accumulator, event, traceLimit);
    }

    function recordTrajectoryMotion(reverseMotion, geometryDrift, stoppedDrift, event = null) {
        const numericReverseMotion = Number(reverseMotion);
        const numericGeometryDrift = Number(geometryDrift);
        const numericStoppedDrift = Number(stoppedDrift);
        root.recordTrajectoryMotionIn(root.phaseAccumulator, numericReverseMotion, numericGeometryDrift, numericStoppedDrift, event, 8);
        root.recordTrajectoryMotionIn(root.aggregateAccumulator, numericReverseMotion, numericGeometryDrift, numericStoppedDrift, event, 24);
    }

    function recordTrackingGap(event) {
        root.appendTraceEvent(root.phaseAccumulator, event, 8);
        root.appendTraceEvent(root.aggregateAccumulator, event, 24);
    }

    function recordTrajectoryCoverage(coverage) {
        const rowMotionSamples = Number(coverage.rowMotionSamples ?? 0);
        const stoppedFrameSamples = Number(coverage.stoppedFrameSamples ?? 0);
        const missingRowFrames = Number(coverage.missingRowFrames ?? 0);
        for (const accumulator of [root.phaseAccumulator, root.aggregateAccumulator]) {
            accumulator.trajectoryMotionSampleCount += rowMotionSamples;
            accumulator.stoppedTrajectoryFrameSampleCount += stoppedFrameSamples;
            accumulator.missingTrajectoryRowFrames += missingRowFrames;
        }
    }

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

    function snapshot(accumulator, frameBudgetMilliseconds) {
        const intervals = accumulator.intervals;
        const samples = accumulator.samples;
        const sortedIntervals = intervals.slice().sort((left, right) => left - right);
        let totalMilliseconds = 0;
        let lateFrameCount = 0;
        let severeFrameCount = 0;
        let missedVsyncCount = 0;
        for (const intervalValue of intervals) {
            const interval = Number(intervalValue);
            totalMilliseconds += interval;
            if (interval > frameBudgetMilliseconds)
                ++lateFrameCount;
            if (interval > frameBudgetMilliseconds * 2)
                ++severeFrameCount;
            if (frameBudgetMilliseconds > 0)
                missedVsyncCount += Math.max(0, Math.round(interval / frameBudgetMilliseconds) - 1);
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
        const sortedLegTravels = accumulator.legTravels.slice().sort((left, right) => left - right);
        const sortedLegPeakVelocities = accumulator.legPeakVelocities.slice().sort((left, right) => left - right);
        const sortedRowTravels = accumulator.rowTravels.slice().sort((left, right) => left - right);
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
            p95FrameBudgetRatio: root.rounded(frameBudgetMilliseconds > 0 ? p95FrameMilliseconds / frameBudgetMilliseconds : 0),
            p99FrameBudgetRatio: root.rounded(frameBudgetMilliseconds > 0 ? p99FrameMilliseconds / frameBudgetMilliseconds : 0),
            worstFrameBudgetRatio: root.rounded(frameBudgetMilliseconds > 0 ? worstFrameMilliseconds / frameBudgetMilliseconds : 0),
            lateFrameCount: lateFrameCount,
            severeFrameCount: severeFrameCount,
            missedVsyncCount: missedVsyncCount,
            travelPixels: root.rounded(Number.isFinite(accumulator.minimumContentY) && Number.isFinite(accumulator.maximumContentY) ? accumulator.maximumContentY - accumulator.minimumContentY : 0),
            maximumObservedVelocity: root.rounded(accumulator.maximumVelocity),
            minimumLegTravelPixels: root.rounded(sortedLegTravels.length > 0 ? Number(sortedLegTravels[0]) : 0),
            maximumLegTravelPixels: root.rounded(sortedLegTravels.length > 0 ? Number(sortedLegTravels[sortedLegTravels.length - 1]) : 0),
            minimumLegPeakVelocity: root.rounded(sortedLegPeakVelocities.length > 0 ? Number(sortedLegPeakVelocities[0]) : 0),
            maximumLegPeakVelocity: root.rounded(sortedLegPeakVelocities.length > 0 ? Number(sortedLegPeakVelocities[sortedLegPeakVelocities.length - 1]) : 0),
            minimumLegRowTravel: sortedRowTravels.length > 0 ? Number(sortedRowTravels[0]) : 0,
            maximumLegRowTravel: sortedRowTravels.length > 0 ? Number(sortedRowTravels[sortedRowTravels.length - 1]) : 0,
            legMeasurementCount: accumulator.legTravels.length,
            anchorCorrectionCount: accumulator.correctionCount,
            maximumAnchorCorrectionPixels: root.rounded(accumulator.maximumCorrection),
            maximumAnchorExcursionPixels: root.rounded(accumulator.maximumAnchorExcursion),
            trajectoryRowMotionSampleCount: accumulator.trajectoryMotionSampleCount,
            stoppedTrajectoryFrameSampleCount: accumulator.stoppedTrajectoryFrameSampleCount,
            maximumReverseRowMotionPixels: root.rounded(accumulator.maximumReverseRowMotion),
            maximumTrajectoryRowGeometryDriftPixels: root.rounded(accumulator.maximumTrajectoryRowGeometryDrift),
            maximumStoppedRowDriftPixels: root.rounded(accumulator.maximumStoppedRowDrift),
            missingTrajectoryRowFrameCount: accumulator.missingTrajectoryRowFrames,
            rowGeometryChangeCount: accumulator.geometryChangeCount,
            maximumRowGeometryChangePixels: root.rounded(accumulator.maximumGeometryChange),
            trajectoryMotionTrace: accumulator.trajectoryMotionTrace.slice()
        };
    }

    function phaseSnapshot(frameBudgetMilliseconds) {
        return root.snapshot(root.phaseAccumulator, Number(frameBudgetMilliseconds));
    }

    function aggregateSnapshot(frameBudgetMilliseconds) {
        return root.snapshot(root.aggregateAccumulator, Number(frameBudgetMilliseconds));
    }
}
