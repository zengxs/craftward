// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 320
    height: 240
    property var finishedBenchmarkResult: null

    QtObject {
        id: fakeWindow

        property int activationRequestCount: 0
        property int flags: 0
        property int updateRequestCount: 0

        signal frameSwapped

        function requestActivate() {
            ++activationRequestCount;
        }

        function update() {
            ++updateRequestCount;
        }
    }

    QtObject {
        id: replacementWindow

        property int activationRequestCount: 0
        property int flags: 0

        signal frameSwapped

        function requestActivate() {
            ++activationRequestCount;
        }
    }

    QtObject {
        id: fakeViewport

        property real contentY: 0
        property real verticalVelocity: 0
        property bool moving: false
        property real viewportHeight: 600
        property real scrollContentHeight: 12000
        property real minimumContentY: 0
        property real maximumContentY: 11400
        property int activeRowSlotCount: 20
        property bool followLiveTail: false
        property var benchmarkAnchor: null
        property var benchmarkMovementEndedAnchor: null
        property var benchmarkMovementEndedRows: []
        property var benchmarkOffsets: ({})
        property var benchmarkVisibleRows: []

        signal anchorPositionCorrected(real displacement)
        signal rowGeometryChanged(int sourceRow, real heightDelta)

        function flickContentForBenchmark(verticalVelocity) {
        }

        function cancelFlickForBenchmark() {
        }

        function positionAtContentY(position) {
            contentY = position;
        }

        function captureVisibleAnchor() {
            return benchmarkAnchor;
        }

        function entryIdAt(row) {
            return "entry:" + row;
        }

        function anchorOffsetForBenchmark(anchor) {
            if (!anchor)
                return Number.NaN;
            const trackedOffset = benchmarkOffsets[String(anchor.entryId)];
            return trackedOffset === undefined ? anchor.offset : trackedOffset;
        }

        function movementEndedAnchorForBenchmark() {
            return benchmarkMovementEndedAnchor;
        }

        function movementEndedRowsForBenchmark() {
            return benchmarkMovementEndedRows;
        }

        function visibleRowOffsetsForBenchmark() {
            if (benchmarkVisibleRows.length > 0)
                return benchmarkVisibleRows;
            const anchor = captureVisibleAnchor();
            if (!anchor)
                return [];
            return [
                {
                    entryId: anchor.entryId,
                    row: anchor.row,
                    offset: anchorOffsetForBenchmark(anchor)
                }
            ];
        }

        function trajectoryRowOffsetsForBenchmark() {
            return visibleRowOffsetsForBenchmark();
        }
    }

    Pages.TimelineRenderBenchmark {
        id: benchmark

        targetViewport: fakeViewport
        targetWindow: fakeWindow
        requestedThreadId: "thread:test"
        selectedThreadId: "thread:test"
        rowCount: 100
        frameBudgetMilliseconds: 1000 / 60
        minimumFlickTravelPixels: 0
        minimumObservedFlickVelocity: 0
        minimumFlickRowTravel: 0
        minimumTrajectoryRowMotionSampleCount: 0
        minimumStoppedTrajectoryFrameSampleCount: 0
        requiredLegMeasurementCount: 0
        onFinished: benchmarkResult => suite.finishedBenchmarkResult = benchmarkResult
    }

    Component {
        id: defaultBenchmarkComponent

        Pages.TimelineRenderBenchmark {
            targetViewport: fakeViewport
            targetWindow: fakeWindow
        }
    }

    TestCase {
        name: "TimelineRenderBenchmark"

        function init() {
            benchmark.restorePresentedWindow();
            benchmark.targetWindow = fakeWindow;
            fakeWindow.activationRequestCount = 0;
            fakeWindow.flags = 0;
            fakeWindow.updateRequestCount = 0;
            replacementWindow.activationRequestCount = 0;
            replacementWindow.flags = 0;
            benchmark.active = false;
            benchmark.rendererName = "current";
            suite.finishedBenchmarkResult = null;
            benchmark.stopTimers();
            benchmark.resetAllMetrics();
            benchmark.phaseActive = true;
            benchmark.legActive = true;
            benchmark.clearLegSampleState();
            benchmark.minimumFlickTravelPixels = 0;
            benchmark.minimumObservedFlickVelocity = 0;
            benchmark.minimumFlickRowTravel = 0;
            benchmark.minimumStoppedTrajectoryFrameSampleCount = 0;
            benchmark.requiredLegMeasurementCount = 0;
            fakeViewport.benchmarkAnchor = null;
            fakeViewport.benchmarkMovementEndedAnchor = null;
            fakeViewport.benchmarkMovementEndedRows = [];
            fakeViewport.benchmarkOffsets = {};
            fakeViewport.benchmarkVisibleRows = [];
            fakeViewport.contentY = 0;
            fakeViewport.verticalVelocity = 0;
            fakeViewport.moving = false;
        }

        function recordSmoothFrames(frameCount) {
            const interval = 1000 / 60;
            benchmark.recordPresentedFrameAt(1000, 0, true);
            for (let frame = 1; frame <= frameCount; ++frame)
                benchmark.recordPresentedFrameAt(1000 + interval * frame, frame * 8, true);
        }

        function test_acceptsConsistentMotionWithinTheFrameBudget() {
            recordSmoothFrames(120);

            const result = benchmark.buildPhaseResult("smooth");

            verify(result.passed, result.failures.join(", "));
            compare(result.metrics.frameSampleCount, 120);
            compare(result.metrics.motionUpdateRatio, 1);
            verify(result.metrics.p99FrameBudgetRatio <= 1.01);
        }

        function test_acceptsTheSemanticRendererAdapter() {
            benchmark.rendererName = "semantic";

            benchmark.beginBenchmark();

            verify(!benchmark.resultEmitted);
            compare(benchmark.state, "waiting-for-content");
            compare(suite.finishedBenchmarkResult, null);
            benchmark.stopTimers();
        }

        function test_rejectsAnUnknownRendererAdapter() {
            benchmark.rendererName = "unknown";

            benchmark.beginBenchmark();

            verify(benchmark.resultEmitted);
            compare(benchmark.state, "finished");
            verify(suite.finishedBenchmarkResult !== null);
            compare(suite.finishedBenchmarkResult.failures, ["unsupported renderer adapter: unknown"]);
        }

        function test_resettingAPhaseRetainsAggregateMetrics() {
            recordSmoothFrames(2);
            benchmark.recordAnchorCorrection(7);

            benchmark.resetPhaseMetrics();

            compare(benchmark.currentPhaseMetrics().frameSampleCount, 0);
            compare(benchmark.currentPhaseMetrics().anchorCorrectionCount, 0);
            compare(benchmark.allMetrics().frameSampleCount, 2);
            compare(benchmark.allMetrics().anchorCorrectionCount, 1);
        }

        function test_requestsAVisibleWindowForEveryFlickLeg() {
            benchmark.legIndex = 0;

            benchmark.beginNextLeg();
            benchmark.legTimer.stop();

            compare(fakeWindow.activationRequestCount, 1);
            verify(Boolean(fakeWindow.flags & Qt.WindowStaysOnTopHint));
        }

        function test_eachPhaseUsesOneBidirectionalStressPair() {
            compare(benchmark.legDirections, [-1, 1]);
        }

        function test_defaultStressTrajectoryHasLongDistanceHeadroom() {
            const defaults = createTemporaryObject(defaultBenchmarkComponent, suite);
            verify(defaults !== null);
            const stoppingTimeMilliseconds = 1000 * defaults.flickContentVelocity / defaults.flickDeceleration;
            const unboundedTravelPixels = defaults.flickContentVelocity * defaults.flickContentVelocity / (2 * defaults.flickDeceleration);

            verify(defaults.flickContentVelocity >= 30000);
            verify(unboundedTravelPixels >= 30000);
            verify(defaults.legDurationMilliseconds >= stoppingTimeMilliseconds);
            verify(defaults.minimumFlickTravelPixels >= 24000);
            verify(defaults.minimumFlickRowTravel >= 100);
            verify(defaults.minimumObservedFlickVelocity >= 30000);
            compare(defaults.requiredLegMeasurementCount, defaults.legDirections.length);
            compare(defaults.stoppedTrajectoryFrameSampleCount * defaults.legDirections.length, defaults.minimumStoppedTrajectoryFrameSampleCount);
        }

        function test_preparesEveryPhaseAtTheCurrentLiveTail() {
            benchmark.phaseIndex = 0;
            fakeViewport.maximumContentY = 11400;
            benchmark.preparePhase();
            benchmark.phasePreparationTimer.stop();
            compare(fakeViewport.contentY, 11400);

            benchmark.phaseIndex = 1;
            fakeViewport.maximumContentY = 16400;
            benchmark.preparePhase();
            benchmark.phasePreparationTimer.stop();
            compare(fakeViewport.contentY, 16400);
        }

        function test_restoresWindowFlagsAfterPresentationSampling() {
            fakeWindow.flags = Qt.WindowTitleHint;

            benchmark.requestPresentedWindow();
            verify(Boolean(fakeWindow.flags & Qt.WindowStaysOnTopHint));

            benchmark.restorePresentedWindow();
            compare(fakeWindow.flags, Qt.WindowTitleHint);
        }

        function test_restoresTheCapturedWindowWhenTheTargetChanges() {
            fakeWindow.flags = Qt.WindowTitleHint;
            replacementWindow.flags = Qt.WindowMinimizeButtonHint;

            benchmark.requestPresentedWindow();
            benchmark.targetWindow = replacementWindow;
            benchmark.restorePresentedWindow();

            compare(fakeWindow.flags, Qt.WindowTitleHint);
            compare(replacementWindow.flags, Qt.WindowMinimizeButtonHint);
        }

        function test_rejectsFramesThatRenderWithoutViewportMovement() {
            const interval = 1000 / 60;
            benchmark.recordPresentedFrameAt(1000, 0, true);
            for (let frame = 1; frame <= 120; ++frame) {
                const frozenPosition = frame >= 30 && frame <= 34 ? 29 * 8 : frame * 8;
                benchmark.recordPresentedFrameAt(1000 + interval * frame, frozenPosition, true);
            }

            const result = benchmark.buildPhaseResult("frozen");

            verify(!result.passed);
            verify(result.failures.includes("motion update ratio below threshold"));
            verify(result.failures.includes("frozen-motion streak above threshold"));
            compare(result.metrics.longestFrozenMotionStreak, 5);
        }

        function test_rejectsASevereSingleFrameStall() {
            const interval = 1000 / 60;
            let timestamp = 1000;
            benchmark.recordPresentedFrameAt(timestamp, 0, true);
            for (let frame = 1; frame <= 120; ++frame) {
                timestamp += frame === 60 ? 80 : interval;
                benchmark.recordPresentedFrameAt(timestamp, frame * 8, true);
            }

            const result = benchmark.buildPhaseResult("stalled");

            verify(!result.passed);
            verify(result.failures.includes("worst frame time above threshold"));
            compare(result.metrics.worstFrameMilliseconds, 80);
        }

        function test_doesNotCountIdleTimeBetweenScrollLegs() {
            recordSmoothFrames(60);
            benchmark.legActive = false;
            benchmark.recordPresentedFrameAt(5000, 480, false);
            benchmark.legActive = true;
            benchmark.clearLegSampleState();
            benchmark.recordPresentedFrameAt(6000, 480, true);
            benchmark.recordPresentedFrameAt(6000 + 1000 / 60, 488, true);

            const metrics = benchmark.currentPhaseMetrics();

            compare(metrics.frameSampleCount, 61);
            verify(metrics.worstFrameMilliseconds < 17);
        }

        function test_reportsObservedFlickVelocityAndTravel() {
            benchmark.recordPresentedFrameAt(1000, 1000, true, 24000);
            benchmark.recordPresentedFrameAt(1000 + 1000 / 60, 7000, true, 18000);
            benchmark.recordPresentedFrameAt(1000 + 2000 / 60, 14000, true, 12000);

            const metrics = benchmark.currentPhaseMetrics();

            compare(metrics.travelPixels, 13000);
            compare(metrics.maximumObservedVelocity, 24000);
        }

        function test_reportsRowsCrossedByAFlickLeg() {
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:10",
                offset: 0,
                row: 10
            };
            benchmark.legIndex = 0;
            benchmark.beginNextLeg();
            benchmark.recordPresentedFrameAt(1000, 25000, true, 31000);
            fakeViewport.contentY = 25000;
            fakeViewport.moving = false;
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:175",
                offset: 0,
                row: 175
            };

            benchmark.endCurrentLeg();
            benchmark.stopTimers();

            const metrics = benchmark.currentPhaseMetrics();
            compare(metrics.minimumLegRowTravel, 165);
            compare(metrics.maximumLegRowTravel, 165);
            compare(metrics.minimumLegTravelPixels, 25000);
            compare(metrics.minimumLegPeakVelocity, 31000);
        }

        function test_rejectsAConfiguredFlickThatNeverReachesStressVelocityOrDistance() {
            benchmark.minimumFlickTravelPixels = 12000;
            benchmark.minimumObservedFlickVelocity = 20000;
            benchmark.recordLegResult(24000, 31000, 0);
            benchmark.recordLegResult(11999, 19999, 0);
            recordSmoothFrames(120);

            const result = benchmark.buildPhaseResult("underpowered");

            verify(result.failures.includes("flick travel below threshold"));
            verify(result.failures.includes("flick velocity below threshold"));
            compare(result.metrics.minimumLegTravelPixels, 11999);
            compare(result.metrics.minimumLegPeakVelocity, 19999);
        }

        function test_rejectsAStressFlickThatCrossesTooFewRows() {
            benchmark.minimumFlickRowTravel = 50;
            benchmark.recordLegResult(0, 0, 120);
            benchmark.recordLegResult(0, 0, 49);
            recordSmoothFrames(120);

            const result = benchmark.buildPhaseResult("underpowered");

            verify(result.failures.includes("flick row travel below threshold"));
            compare(result.metrics.minimumLegRowTravel, 49);
        }

        function test_rejectsAPhaseWithAnIncompleteLegMeasurementSet() {
            benchmark.requiredLegMeasurementCount = 2;
            benchmark.recordLegResult(25000, 31000, 120);
            recordSmoothFrames(120);

            const result = benchmark.buildPhaseResult("underpowered");

            verify(result.failures.includes("incomplete flick leg measurements"));
            compare(result.metrics.legMeasurementCount, 1);
        }

        function test_recordsAnUnderpoweredLegWhenItsAnchorIsUnavailable() {
            benchmark.legStartContentY = Number.NaN;
            benchmark.legStartRow = -1;
            benchmark.legMaximumObservedVelocity = 31000;

            benchmark.recordLegMeasurement(-1);

            const metrics = benchmark.currentPhaseMetrics();
            compare(metrics.minimumLegTravelPixels, 0);
            compare(metrics.maximumLegTravelPixels, 0);
            compare(metrics.minimumLegPeakVelocity, 31000);
            compare(metrics.maximumLegPeakVelocity, 31000);
            compare(metrics.minimumLegRowTravel, 0);
            compare(metrics.maximumLegRowTravel, 0);
            compare(metrics.legMeasurementCount, 1);
        }

        function test_reportsPostScrollGeometryAndAnchorActivity() {
            benchmark.recordAnchorCorrection(-3.5);
            benchmark.recordAnchorCorrection(7);
            benchmark.recordRowGeometryChange(42);
            benchmark.recordRowGeometryChange(-12);
            benchmark.recordAnchorExcursion(3.5);

            const metrics = benchmark.currentPhaseMetrics();

            compare(metrics.anchorCorrectionCount, 2);
            compare(metrics.maximumAnchorCorrectionPixels, 7);
            compare(metrics.maximumAnchorExcursionPixels, 3.5);
            compare(metrics.rowGeometryChangeCount, 2);
            compare(metrics.maximumRowGeometryChangePixels, 42);
        }

        function test_rejectsPostScrollAnchorExcursion() {
            recordSmoothFrames(120);
            benchmark.recordAnchorExcursion(4);

            const result = benchmark.buildPhaseResult("cold");

            verify(!result.passed);
            verify(result.failures.includes("post-scroll anchor excursion above threshold"));
        }

        function test_rejectsReverseRowMotionAtHighVelocity() {
            recordSmoothFrames(120);
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:10",
                offset: 0,
                row: 10
            };
            fakeViewport.benchmarkOffsets = {
                "entry:10": 100
            };
            fakeViewport.contentY = 1000;
            fakeViewport.verticalVelocity = 31000;
            fakeViewport.moving = true;
            benchmark.activeLegContentDirection = 1;

            benchmark.sampleTrajectoryRowMotion();

            compare(benchmark.previousTrajectoryVisibleRowOffsets["entry:10"], 100);
            fakeViewport.benchmarkOffsets = {
                "entry:10": 160
            };
            fakeViewport.contentY = 1040;
            benchmark.sampleTrajectoryRowMotion();

            const result = benchmark.buildPhaseResult("cold");
            verify(!result.passed);
            verify(result.failures.includes("reverse row motion during flick trajectory"));
            verify(result.failures.includes("row geometry drifted during flick trajectory"));
            compare(result.metrics.trajectoryRowMotionSampleCount, 1);
            compare(result.metrics.maximumReverseRowMotionPixels, 60);
            compare(result.metrics.maximumTrajectoryRowGeometryDriftPixels, 100);
        }

        function test_detectsTransientGeometryDriftAndReturnAtHighVelocity() {
            recordSmoothFrames(120);
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:11",
                offset: -5,
                row: 11
            };
            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:11",
                    row: 11,
                    offset: -5
                }
            ];
            fakeViewport.contentY = 1000;
            fakeViewport.verticalVelocity = 31000;
            fakeViewport.moving = true;
            benchmark.activeLegContentDirection = 1;
            benchmark.sampleTrajectoryRowMotion();

            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:11",
                    row: 11,
                    offset: -375
                }
            ];
            fakeViewport.contentY = 1400;
            benchmark.sampleTrajectoryRowMotion();

            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:11",
                    row: 11,
                    offset: -805
                }
            ];
            fakeViewport.contentY = 1800;
            benchmark.sampleTrajectoryRowMotion();

            const result = benchmark.buildPhaseResult("cold");
            verify(!result.passed);
            verify(result.failures.includes("row geometry drifted during flick trajectory"));
            compare(result.metrics.maximumReverseRowMotionPixels, 0);
            compare(result.metrics.maximumTrajectoryRowGeometryDriftPixels, 30);
            compare(result.metrics.trajectoryMotionTrace.length, 2);
        }

        function test_keepsSamplingAcrossNaturalVisibleAnchorChanges() {
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:10",
                offset: -60,
                row: 10
            };
            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:10",
                    row: 10,
                    offset: -60
                },
                {
                    entryId: "entry:11",
                    row: 11,
                    offset: -5
                }
            ];
            fakeViewport.contentY = 1000;
            fakeViewport.verticalVelocity = 3500;
            fakeViewport.moving = true;
            benchmark.activeLegContentDirection = 1;
            benchmark.sampleTrajectoryRowMotion();

            fakeViewport.benchmarkAnchor = {
                entryId: "entry:11",
                offset: -5,
                row: 11
            };
            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:11",
                    row: 11,
                    offset: -65
                },
                {
                    entryId: "entry:12",
                    row: 12,
                    offset: 10
                }
            ];
            fakeViewport.contentY = 1060;
            benchmark.sampleTrajectoryRowMotion();

            compare(benchmark.previousTrajectoryVisibleRowOffsets["entry:11"], -65);
            compare(benchmark.currentPhaseMetrics().trajectoryRowMotionSampleCount, 1);
            compare(benchmark.currentPhaseMetrics().maximumReverseRowMotionPixels, 0);
            compare(benchmark.currentPhaseMetrics().maximumTrajectoryRowGeometryDriftPixels, 0);
            compare(benchmark.currentPhaseMetrics().missingTrajectoryRowFrameCount, 0);
        }

        function test_detectsReverseMotionAcrossAVisibleAnchorChange() {
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:10",
                offset: -60,
                row: 10
            };
            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:10",
                    row: 10,
                    offset: -60
                },
                {
                    entryId: "entry:11",
                    row: 11,
                    offset: -5
                }
            ];
            fakeViewport.contentY = 1000;
            fakeViewport.verticalVelocity = 3500;
            fakeViewport.moving = true;
            benchmark.activeLegContentDirection = 1;
            benchmark.sampleTrajectoryRowMotion();

            fakeViewport.benchmarkAnchor = {
                entryId: "entry:11",
                offset: 20,
                row: 11
            };
            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:11",
                    row: 11,
                    offset: 20
                },
                {
                    entryId: "entry:12",
                    row: 12,
                    offset: 95
                }
            ];
            fakeViewport.contentY = 1075;
            benchmark.sampleTrajectoryRowMotion();

            compare(benchmark.currentPhaseMetrics().trajectoryRowMotionSampleCount, 1);
            compare(benchmark.currentPhaseMetrics().maximumReverseRowMotionPixels, 25);
            compare(benchmark.currentPhaseMetrics().trajectoryMotionTrace.length, 1);
            compare(benchmark.currentPhaseMetrics().trajectoryMotionTrace[0].entryId, "entry:11");
            compare(benchmark.currentPhaseMetrics().trajectoryMotionTrace[0].offsetDelta, 25);
        }

        function test_reportsWhenNoVisibleRowSurvivesBetweenFrames() {
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:10",
                offset: -5,
                row: 10
            };
            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:10",
                    row: 10,
                    offset: -5
                }
            ];
            fakeViewport.verticalVelocity = 3500;
            fakeViewport.moving = true;
            benchmark.activeLegContentDirection = 1;
            benchmark.sampleTrajectoryRowMotion();

            fakeViewport.benchmarkAnchor = {
                entryId: "entry:20",
                offset: -5,
                row: 20
            };
            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:20",
                    row: 20,
                    offset: -5
                }
            ];
            fakeViewport.contentY = 2000;
            benchmark.sampleTrajectoryRowMotion();

            compare(benchmark.currentPhaseMetrics().missingTrajectoryRowFrameCount, 1);
        }

        function test_rejectsAPhaseWithoutStoppedFrameCoverageForBothLegs() {
            benchmark.minimumStoppedTrajectoryFrameSampleCount = 6;
            recordSmoothFrames(120);

            const result = benchmark.buildPhaseResult("cold");

            verify(result.failures.includes("insufficient stopped-frame row-motion samples"));
        }

        function test_samplesStoppedFramesAndCatchesAReturn() {
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:10",
                offset: 0,
                row: 10
            };
            fakeViewport.benchmarkOffsets = {
                "entry:10": 100
            };
            fakeViewport.contentY = 1000;
            fakeViewport.verticalVelocity = 3500;
            fakeViewport.moving = true;
            benchmark.activeLegContentDirection = 1;
            benchmark.sampleTrajectoryRowMotion();

            fakeViewport.benchmarkOffsets = {
                "entry:10": 80
            };
            fakeViewport.benchmarkMovementEndedAnchor = {
                entryId: "entry:10",
                row: 10,
                offset: 90,
                contentY: 1010
            };
            fakeViewport.contentY = 1010;
            fakeViewport.verticalVelocity = 0;
            fakeViewport.moving = false;
            benchmark.sampleTrajectoryRowMotion(true);
            fakeViewport.benchmarkOffsets = {
                "entry:10": 90
            };
            benchmark.sampleTrajectoryRowMotion(true);
            benchmark.sampleTrajectoryRowMotion(true);
            benchmark.sampleTrajectoryRowMotion(true);

            const metrics = benchmark.currentPhaseMetrics();
            compare(metrics.trajectoryRowMotionSampleCount, 4);
            compare(metrics.stoppedTrajectoryFrameSampleCount, 3);
            compare(metrics.maximumStoppedRowDriftPixels, 10);
            compare(metrics.maximumReverseRowMotionPixels, 0);
            compare(metrics.trajectoryMotionTrace.length, 1);
        }

        function test_detectsAStoppedJumpAndReturnInANonAnchorVisibleRow() {
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:10",
                offset: 0,
                row: 10
            };
            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:10",
                    row: 10,
                    offset: 0
                },
                {
                    entryId: "entry:11",
                    row: 11,
                    offset: 100
                }
            ];
            fakeViewport.contentY = 1000;
            fakeViewport.verticalVelocity = 3500;
            fakeViewport.moving = true;
            benchmark.activeLegContentDirection = 1;
            benchmark.sampleTrajectoryRowMotion();

            fakeViewport.benchmarkMovementEndedAnchor = {
                entryId: "entry:10",
                row: 10,
                offset: -10,
                contentY: 1010
            };
            fakeViewport.benchmarkMovementEndedRows = [
                {
                    entryId: "entry:10",
                    row: 10,
                    offset: -10
                },
                {
                    entryId: "entry:11",
                    row: 11,
                    offset: 90
                }
            ];
            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:10",
                    row: 10,
                    offset: -10
                },
                {
                    entryId: "entry:11",
                    row: 11,
                    offset: 140
                }
            ];
            fakeViewport.contentY = 1010;
            fakeViewport.verticalVelocity = 0;
            fakeViewport.moving = false;
            benchmark.sampleTrajectoryRowMotion(true);
            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:10",
                    row: 10,
                    offset: -10
                },
                {
                    entryId: "entry:11",
                    row: 11,
                    offset: 90
                }
            ];
            benchmark.sampleTrajectoryRowMotion(true);
            benchmark.sampleTrajectoryRowMotion(true);

            const result = benchmark.buildPhaseResult("cold");
            verify(!result.passed);
            verify(result.failures.includes("row drifted after flick stopped"));
            compare(result.metrics.maximumReverseRowMotionPixels, 0);
            compare(result.metrics.maximumStoppedRowDriftPixels, 50);
            compare(result.metrics.stoppedTrajectoryFrameSampleCount, 3);
            compare(result.metrics.trajectoryMotionTrace.length, 1);
            compare(result.metrics.trajectoryMotionTrace[0].entryId, "entry:11");
        }

        function test_detectsStoppedContentDriftInsideAStableRowShell() {
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:10",
                offset: 0,
                row: 10
            };
            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:10",
                    row: 10,
                    offset: 0,
                    visualMarkers: [
                        {
                            markerId: "content:end",
                            offset: 80
                        }
                    ]
                }
            ];
            fakeViewport.contentY = 1000;
            fakeViewport.verticalVelocity = 3500;
            fakeViewport.moving = true;
            benchmark.activeLegContentDirection = 1;
            benchmark.sampleTrajectoryRowMotion();

            fakeViewport.benchmarkMovementEndedAnchor = {
                entryId: "entry:10",
                row: 10,
                offset: -10,
                contentY: 1010
            };
            fakeViewport.benchmarkMovementEndedRows = [
                {
                    entryId: "entry:10",
                    row: 10,
                    offset: -10,
                    visualMarkers: [
                        {
                            markerId: "content:end",
                            offset: 70
                        }
                    ]
                }
            ];
            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:10",
                    row: 10,
                    offset: -10,
                    visualMarkers: [
                        {
                            markerId: "content:end",
                            offset: 120
                        }
                    ]
                }
            ];
            fakeViewport.contentY = 1010;
            fakeViewport.verticalVelocity = 0;
            fakeViewport.moving = false;
            benchmark.sampleTrajectoryRowMotion(true);

            const result = benchmark.buildPhaseResult("cold");
            verify(!result.passed);
            verify(result.failures.includes("row drifted after flick stopped"));
            compare(result.metrics.maximumStoppedRowDriftPixels, 50);
            compare(result.metrics.trajectoryMotionTrace.length, 1);
            compare(result.metrics.trajectoryMotionTrace[0].entryId, "entry:10");
            compare(result.metrics.trajectoryMotionTrace[0].markerId, "content:end");
        }

        function test_rejectsAStoppedForwardDriftThatPersists() {
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:10",
                offset: 0,
                row: 10
            };
            fakeViewport.benchmarkOffsets = {
                "entry:10": 100
            };
            fakeViewport.contentY = 1000;
            fakeViewport.verticalVelocity = 3500;
            fakeViewport.moving = true;
            benchmark.activeLegContentDirection = 1;
            benchmark.sampleTrajectoryRowMotion();

            fakeViewport.benchmarkOffsets = {
                "entry:10": 80
            };
            fakeViewport.benchmarkMovementEndedAnchor = {
                entryId: "entry:10",
                row: 10,
                offset: 90,
                contentY: 1010
            };
            fakeViewport.contentY = 1010;
            fakeViewport.verticalVelocity = 0;
            fakeViewport.moving = false;
            benchmark.sampleTrajectoryRowMotion(true);
            benchmark.sampleTrajectoryRowMotion(true);
            benchmark.sampleTrajectoryRowMotion(true);

            const result = benchmark.buildPhaseResult("cold");
            verify(!result.passed);
            verify(result.failures.includes("row drifted after flick stopped"));
            compare(result.metrics.maximumReverseRowMotionPixels, 0);
            compare(result.metrics.maximumStoppedRowDriftPixels, 10);
        }

        function test_detectsDriftBetweenTheLastMovingFrameAndMovementEndSnapshot() {
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:10",
                offset: 100,
                row: 10
            };
            fakeViewport.benchmarkOffsets = {
                "entry:10": 100
            };
            fakeViewport.contentY = 1000;
            fakeViewport.moving = true;
            benchmark.sampleTrajectoryRowMotion();

            fakeViewport.benchmarkMovementEndedAnchor = {
                entryId: "entry:10",
                row: 10,
                offset: 100,
                contentY: 1010
            };
            fakeViewport.contentY = 1010;
            fakeViewport.moving = false;
            benchmark.sampleTrajectoryRowMotion(true);

            const result = benchmark.buildPhaseResult("cold");
            verify(!result.passed);
            verify(result.failures.includes("row geometry drifted during flick trajectory"));
            compare(result.metrics.maximumTrajectoryRowGeometryDriftPixels, 10);
            compare(result.metrics.maximumStoppedRowDriftPixels, 0);
        }

        function test_detectsMovementBoundaryDriftInANonAnchorVisibleRow() {
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:10",
                offset: 0,
                row: 10
            };
            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:10",
                    row: 10,
                    offset: 0
                },
                {
                    entryId: "entry:11",
                    row: 11,
                    offset: 100
                }
            ];
            fakeViewport.contentY = 1000;
            fakeViewport.moving = true;
            benchmark.sampleTrajectoryRowMotion();

            fakeViewport.benchmarkMovementEndedAnchor = {
                entryId: "entry:10",
                row: 10,
                offset: -10,
                contentY: 1010
            };
            fakeViewport.benchmarkMovementEndedRows = [
                {
                    entryId: "entry:10",
                    row: 10,
                    offset: -10
                },
                {
                    entryId: "entry:11",
                    row: 11,
                    offset: 100
                }
            ];
            fakeViewport.benchmarkVisibleRows = fakeViewport.benchmarkMovementEndedRows;
            fakeViewport.contentY = 1010;
            fakeViewport.moving = false;
            benchmark.sampleTrajectoryRowMotion(true);

            const result = benchmark.buildPhaseResult("cold");
            verify(!result.passed);
            verify(result.failures.includes("row geometry drifted during flick trajectory"));
            compare(result.metrics.maximumTrajectoryRowGeometryDriftPixels, 10);
            compare(result.metrics.maximumStoppedRowDriftPixels, 0);
            compare(result.metrics.trajectoryMotionTrace.length, 1);
            compare(result.metrics.trajectoryMotionTrace[0].entryId, "entry:11");
            compare(result.metrics.trajectoryMotionTrace[0].kind, "movement-boundary");
        }

        function test_detectsMovementBoundaryContentDriftInsideAStableRowShell() {
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:10",
                offset: 0,
                row: 10
            };
            fakeViewport.benchmarkVisibleRows = [
                {
                    entryId: "entry:10",
                    row: 10,
                    offset: 0,
                    visualMarkers: [
                        {
                            markerId: "content:end",
                            offset: 80
                        }
                    ]
                }
            ];
            fakeViewport.contentY = 1000;
            fakeViewport.moving = true;
            benchmark.sampleTrajectoryRowMotion();

            fakeViewport.benchmarkMovementEndedAnchor = {
                entryId: "entry:10",
                row: 10,
                offset: -10,
                contentY: 1010
            };
            fakeViewport.benchmarkMovementEndedRows = [
                {
                    entryId: "entry:10",
                    row: 10,
                    offset: -10,
                    visualMarkers: [
                        {
                            markerId: "content:end",
                            offset: 80
                        }
                    ]
                }
            ];
            fakeViewport.benchmarkVisibleRows = fakeViewport.benchmarkMovementEndedRows;
            fakeViewport.contentY = 1010;
            fakeViewport.moving = false;
            benchmark.sampleTrajectoryRowMotion(true);

            const result = benchmark.buildPhaseResult("cold");
            verify(!result.passed);
            verify(result.failures.includes("row geometry drifted during flick trajectory"));
            compare(result.metrics.maximumTrajectoryRowGeometryDriftPixels, 10);
            compare(result.metrics.maximumStoppedRowDriftPixels, 0);
            compare(result.metrics.trajectoryMotionTrace.length, 1);
            compare(result.metrics.trajectoryMotionTrace[0].entryId, "entry:10");
            compare(result.metrics.trajectoryMotionTrace[0].markerId, "content:end");
            compare(result.metrics.trajectoryMotionTrace[0].kind, "movement-boundary-content");
        }

        function test_waitsForActualStoppedFramesBeforeCompletingALeg() {
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:10",
                offset: 100,
                row: 10
            };
            fakeViewport.benchmarkOffsets = {
                "entry:10": 100
            };
            fakeViewport.contentY = 1000;
            fakeViewport.moving = true;
            benchmark.sampleTrajectoryRowMotion();
            fakeViewport.benchmarkMovementEndedAnchor = {
                entryId: "entry:10",
                row: 10,
                offset: 100,
                contentY: 1000
            };
            fakeViewport.moving = false;

            benchmark.requestEndCurrentLeg();
            benchmark.legCompletionTimer.stop();

            verify(benchmark.legActive);
            compare(fakeWindow.updateRequestCount, 1);
            benchmark.recordCurrentPresentedFrame();
            verify(benchmark.legActive);
            benchmark.recordCurrentPresentedFrame();
            verify(benchmark.legActive);
            benchmark.recordCurrentPresentedFrame();
            verify(!benchmark.legActive);
            benchmark.stopTimers();

            compare(benchmark.currentPhaseMetrics().stoppedTrajectoryFrameSampleCount, 3);
            compare(fakeWindow.updateRequestCount, 3);
        }

        function test_doesNotTreatATimerSampleAsAPresentedStoppedFrame() {
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:10",
                offset: 0,
                row: 10
            };
            fakeViewport.benchmarkOffsets = {
                "entry:10": 100
            };
            fakeViewport.contentY = 1000;
            fakeViewport.moving = true;
            benchmark.sampleTrajectoryRowMotion();

            fakeViewport.benchmarkMovementEndedAnchor = {
                entryId: "entry:10",
                row: 10,
                offset: 100,
                contentY: 1000
            };
            fakeViewport.moving = false;
            benchmark.sampleTrajectoryRowMotion();

            compare(benchmark.currentPhaseMetrics().stoppedTrajectoryFrameSampleCount, 0);
        }

        function test_legFinalizationDoesNotSynthesizeAStoppedFrame() {
            fakeViewport.benchmarkAnchor = {
                entryId: "entry:10",
                offset: 0,
                row: 10
            };
            fakeViewport.benchmarkOffsets = {
                "entry:10": 100
            };
            fakeViewport.contentY = 1000;
            fakeViewport.moving = true;
            benchmark.sampleTrajectoryRowMotion();

            fakeViewport.benchmarkMovementEndedAnchor = {
                entryId: "entry:10",
                row: 10,
                offset: 100,
                contentY: 1000
            };
            fakeViewport.moving = false;
            benchmark.endCurrentLeg();
            benchmark.stopTimers();

            compare(benchmark.currentPhaseMetrics().stoppedTrajectoryFrameSampleCount, 0);
        }

        function test_rejectsRepeatedGeometryChangesOnAWarmTrajectory() {
            recordSmoothFrames(120);
            benchmark.recordRowGeometryChange(12);

            const result = benchmark.buildPhaseResult("warm-1");

            verify(!result.passed);
            verify(result.failures.includes("warm trajectory changed row geometry"));
        }
    }
}
