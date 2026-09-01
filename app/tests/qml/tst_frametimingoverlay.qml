// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Window
import QtTest
import Craftward.Components

Item {
    id: suite

    width: 400
    height: 240

    FrameTimingOverlay {
        id: overlay

        targetWindow: suite.Window.window
        frameBudgetMilliseconds: 1000 / 60
        publicationIntervalMilliseconds: 250
    }

    TestCase {
        name: "FrameTimingOverlay"
        when: windowShown

        function init() {
            overlay.active = false;
            overlay.resetStatistics();
            overlay.maximumSampleCount = 180;
            overlay.supplementalStatisticsText = "";
        }

        function test_isHiddenByDefault() {
            compare(overlay.active, false);
            compare(overlay.visible, false);

            overlay.active = true;

            compare(overlay.visible, true);
        }

        function test_calculatesFrameTailStatistics() {
            overlay.recordFrameAt(1000);
            overlay.recordFrameAt(1016);
            overlay.recordFrameAt(1032);
            overlay.recordFrameAt(1052);
            overlay.recordFrameAt(1092);
            overlay.publishStatistics();

            compare(overlay.sampleCount, 4);
            fuzzyCompare(overlay.swapFramesPerSecond, 1000 / 23, 0.01);
            compare(overlay.p95FrameMilliseconds, 40);
            compare(overlay.p99FrameMilliseconds, 40);
            compare(overlay.worstFrameMilliseconds, 40);
            compare(overlay.lateFrameCount, 2);
            compare(overlay.severeFrameCount, 1);
            verify(overlay.statisticsText.includes("Swap"));
        }

        function test_recordsLongStallsAsSevereFrames() {
            overlay.recordFrameAt(1000);
            overlay.recordFrameAt(1016);
            overlay.recordFrameAt(1416);
            overlay.publishStatistics();

            compare(overlay.sampleCount, 2);
            compare(overlay.worstFrameMilliseconds, 400);
            compare(overlay.severeFrameCount, 1);
        }

        function test_boundsTheRollingSampleWindow() {
            overlay.maximumSampleCount = 3;
            overlay.recordFrameAt(1000);
            overlay.recordFrameAt(1010);
            overlay.recordFrameAt(1021);
            overlay.recordFrameAt(1033);
            overlay.recordFrameAt(1046);
            overlay.publishStatistics();

            compare(overlay.sampleCount, 3);
            compare(overlay.p95FrameMilliseconds, 13);
            compare(overlay.worstFrameMilliseconds, 13);
        }

        function test_countsMissedVsyncsAndTheirLongestStreak() {
            overlay.recordFrameAt(1000);
            overlay.recordFrameAt(1016);
            overlay.recordFrameAt(1049);
            overlay.recordFrameAt(1082);
            overlay.recordFrameAt(1098);
            overlay.publishStatistics();

            compare(overlay.missedVsyncCount, 2);
            compare(overlay.longestMissedVsyncStreak, 2);
        }

        function test_appendsSupplementalStatisticsWithoutUnderstandingThem() {
            overlay.supplementalStatisticsText = "Motion 60.0 FPS";
            overlay.recordFrameAt(1000);
            overlay.recordFrameAt(1016);
            overlay.publishStatistics();

            verify(overlay.statisticsText.includes("Motion 60.0 FPS"));
        }
    }
}
