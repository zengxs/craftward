// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 320
    height: 240

    QtObject {
        id: fakeWindow

        signal frameSwapped
    }

    QtObject {
        id: fakeViewport

        property real contentY: 0
        property bool moving: false
        property real viewportHeight: 600
        property real scrollContentHeight: 12000
        property real minimumContentY: 0
        property real maximumContentY: 11400
        property int activeRowSlotCount: 20
        property bool followLiveTail: false
        property var benchmarkAnchor: null

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

        function anchorOffsetForBenchmark(anchor) {
            return anchor ? anchor.offset : Number.NaN;
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
    }

    TestCase {
        name: "TimelineRenderBenchmark"

        function init() {
            benchmark.active = false;
            benchmark.stopTimers();
            benchmark.resetAllMetrics();
            benchmark.phaseActive = true;
            benchmark.legActive = true;
            benchmark.clearLegSampleState();
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

        function test_rejectsRepeatedGeometryChangesOnAWarmTrajectory() {
            recordSmoothFrames(120);
            benchmark.recordRowGeometryChange(12);

            const result = benchmark.buildPhaseResult("warm-1");

            verify(!result.passed);
            verify(result.failures.includes("warm trajectory changed row geometry"));
        }
    }
}
