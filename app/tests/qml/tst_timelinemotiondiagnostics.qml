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
        id: fakeViewport

        property real contentY: 0
        property bool moving: false

        signal anchorPositionCorrected(real displacement)
        signal rowGeometryChanged(int sourceRow, real heightDelta)
    }

    Pages.TimelineMotionDiagnostics {
        id: diagnostics

        targetViewport: fakeViewport
    }

    TestCase {
        name: "TimelineMotionDiagnostics"

        function init() {
            diagnostics.active = false;
            diagnostics.resetStatistics();
            diagnostics.maximumSampleCount = 180;
        }

        function test_countsOnlyFramesThatActuallyMoveTheViewport() {
            diagnostics.recordFrameAt(1000, 0, true);
            diagnostics.recordFrameAt(1016, 10, true);
            diagnostics.recordFrameAt(1032, 10, true);
            diagnostics.recordFrameAt(1048, 26, true);
            diagnostics.publishStatistics();

            compare(diagnostics.motionSampleCount, 3);
            compare(diagnostics.motionUpdateCount, 2);
            compare(diagnostics.frozenMotionFrameCount, 1);
            fuzzyCompare(diagnostics.motionFramesPerSecond, 1000 * 2 / 48, 0.01);
        }

        function test_doesNotTreatIdleFramesAsFrozenMotion() {
            diagnostics.recordFrameAt(1000, 0, false);
            diagnostics.recordFrameAt(1016, 0, false);
            diagnostics.recordFrameAt(1032, 0, true);
            diagnostics.recordFrameAt(1048, 12, true);
            diagnostics.publishStatistics();

            compare(diagnostics.motionSampleCount, 1);
            compare(diagnostics.motionUpdateCount, 1);
            compare(diagnostics.frozenMotionFrameCount, 0);
        }

        function test_recordsAnchorAndRowGeometryCorrectionsSeparately() {
            diagnostics.active = true;
            fakeViewport.anchorPositionCorrected(-3.5);
            fakeViewport.anchorPositionCorrected(7);
            fakeViewport.rowGeometryChanged(1, 40);
            fakeViewport.rowGeometryChanged(2, -12);

            compare(diagnostics.anchorCorrectionCount, 2);
            compare(diagnostics.maximumAnchorCorrectionPixels, 7);
            compare(diagnostics.rowGeometryChangeCount, 2);
            compare(diagnostics.maximumRowGeometryChangePixels, 40);
            verify(diagnostics.statisticsText.includes("Anchor 2"));
            verify(diagnostics.statisticsText.includes("reflow 2"));
        }

        function test_boundsTheRollingMotionWindow() {
            diagnostics.maximumSampleCount = 2;
            diagnostics.recordFrameAt(1000, 0, true);
            diagnostics.recordFrameAt(1010, 10, true);
            diagnostics.recordFrameAt(1020, 20, true);
            diagnostics.recordFrameAt(1030, 20, true);
            diagnostics.publishStatistics();

            compare(diagnostics.motionSampleCount, 2);
            compare(diagnostics.motionUpdateCount, 1);
            compare(diagnostics.frozenMotionFrameCount, 1);
        }
    }
}
