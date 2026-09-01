// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Window
import "FrameTiming.js" as FrameTiming

Rectangle {
    id: root

    required property var targetWindow
    property bool active: false
    property int maximumSampleCount: 180
    property real publicationIntervalMilliseconds: 250
    property real frameBudgetMilliseconds: FrameTiming.budgetMilliseconds(targetWindow)
    property string supplementalStatisticsText: ""
    property var frameIntervals: []
    property real previousFrameTimestamp: Number.NaN
    property real lastPublicationTimestamp: Number.NaN
    property int sampleCount: 0
    property real swapFramesPerSecond: 0
    property real p95FrameMilliseconds: 0
    property real p99FrameMilliseconds: 0
    property real worstFrameMilliseconds: 0
    property int lateFrameCount: 0
    property int severeFrameCount: 0
    property int missedVsyncCount: 0
    property int longestMissedVsyncStreak: 0
    readonly property string statisticsText: {
        if (root.sampleCount === 0)
            return "Frame timing\nWaiting for rendered frames…";
        const displayRefreshRate = root.frameBudgetMilliseconds > 0 ? 1000 / root.frameBudgetMilliseconds : 0;
        const supplementalText = root.supplementalStatisticsText.length > 0 ? "\n" + root.supplementalStatisticsText : "";
        return "Display " + displayRefreshRate.toFixed(0) + " Hz  ·  Swap " + root.swapFramesPerSecond.toFixed(1) + " FPS\n" + "p95 " + root.p95FrameMilliseconds.toFixed(1) + " ms  ·  p99 " + root.p99FrameMilliseconds.toFixed(1) + " ms  ·  worst " + root.worstFrameMilliseconds.toFixed(1) + " ms\n" + "Missed vsync " + root.missedVsyncCount + "  ·  longest streak " + root.longestMissedVsyncStreak + supplementalText;
    }

    function percentile(sortedValues, fraction) {
        if (sortedValues.length === 0)
            return 0;
        const index = Math.max(0, Math.min(sortedValues.length - 1, Math.ceil(fraction * sortedValues.length) - 1));
        return Number(sortedValues[index]);
    }

    function clearPublishedStatistics() {
        sampleCount = 0;
        swapFramesPerSecond = 0;
        p95FrameMilliseconds = 0;
        p99FrameMilliseconds = 0;
        worstFrameMilliseconds = 0;
        lateFrameCount = 0;
        severeFrameCount = 0;
        missedVsyncCount = 0;
        longestMissedVsyncStreak = 0;
    }

    function resetStatistics() {
        frameIntervals = [];
        previousFrameTimestamp = Number.NaN;
        lastPublicationTimestamp = Number.NaN;
        clearPublishedStatistics();
    }

    function publishStatistics() {
        if (frameIntervals.length === 0) {
            clearPublishedStatistics();
            return;
        }

        const sortedIntervals = frameIntervals.slice().sort((left, right) => left - right);
        const totalMilliseconds = frameIntervals.reduce((total, interval) => total + interval, 0);
        sampleCount = frameIntervals.length;
        swapFramesPerSecond = totalMilliseconds > 0 ? 1000 * sampleCount / totalMilliseconds : 0;
        p95FrameMilliseconds = percentile(sortedIntervals, 0.95);
        p99FrameMilliseconds = percentile(sortedIntervals, 0.99);
        worstFrameMilliseconds = sortedIntervals[sortedIntervals.length - 1];
        lateFrameCount = frameIntervals.reduce((count, interval) => count + (interval > frameBudgetMilliseconds ? 1 : 0), 0);
        severeFrameCount = frameIntervals.reduce((count, interval) => count + (interval > frameBudgetMilliseconds * 2 ? 1 : 0), 0);
        let currentMissedVsyncStreak = 0;
        missedVsyncCount = 0;
        longestMissedVsyncStreak = 0;
        for (const interval of frameIntervals) {
            const missedVsyncs = frameBudgetMilliseconds > 0 ? Math.max(0, Math.round(interval / frameBudgetMilliseconds) - 1) : 0;
            missedVsyncCount += missedVsyncs;
            if (missedVsyncs > 0) {
                currentMissedVsyncStreak += missedVsyncs;
                longestMissedVsyncStreak = Math.max(longestMissedVsyncStreak, currentMissedVsyncStreak);
            } else {
                currentMissedVsyncStreak = 0;
            }
        }
    }

    function recordFrameAt(timestampMilliseconds) {
        const timestamp = Number(timestampMilliseconds);
        if (!Number.isFinite(timestamp))
            return;

        if (Number.isFinite(previousFrameTimestamp)) {
            const interval = timestamp - previousFrameTimestamp;
            if (interval > 0) {
                frameIntervals.push(interval);
                if (frameIntervals.length > maximumSampleCount)
                    frameIntervals.splice(0, frameIntervals.length - maximumSampleCount);
            }
        }

        previousFrameTimestamp = timestamp;
        if (!Number.isFinite(lastPublicationTimestamp))
            lastPublicationTimestamp = timestamp;
        if (timestamp - lastPublicationTimestamp >= publicationIntervalMilliseconds) {
            publishStatistics();
            lastPublicationTimestamp = timestamp;
        }
    }

    anchors {
        rightMargin: 12
        topMargin: 12
    }
    width: Math.max(248, statisticsText.implicitWidth + 24)
    height: statisticsText.implicitHeight + 20
    radius: 8
    color: "#e61f2937"
    border {
        width: 1
        color: "#596b7280"
    }
    visible: active
    z: 1000000

    FrameAnimation {
        running: root.active
    }

    Text {
        id: statisticsText

        anchors.centerIn: parent
        text: root.statisticsText
        color: "#f9fafb"
        font {
            pixelSize: 11
        }
        lineHeight: 1.2
        renderType: Text.NativeRendering
    }

    Connections {
        target: root.targetWindow
        enabled: root.active

        function onFrameSwapped() {
            root.recordFrameAt(Date.now());
        }
    }

    onActiveChanged: resetStatistics()
}
