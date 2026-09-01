// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

QtObject {
    id: root

    required property var targetViewport
    property bool active: false
    property int maximumSampleCount: 180
    property real publicationIntervalMilliseconds: 250
    property real movementEpsilon: 0.1
    property var motionSamples: []
    property real previousFrameTimestamp: Number.NaN
    property real previousContentY: Number.NaN
    property bool previousMoving: false
    property real lastPublicationTimestamp: Number.NaN
    property int motionSampleCount: 0
    property int motionUpdateCount: 0
    property int frozenMotionFrameCount: 0
    property real motionFramesPerSecond: 0
    property int anchorCorrectionCount: 0
    property real maximumAnchorCorrectionPixels: 0
    property int rowGeometryChangeCount: 0
    property real maximumRowGeometryChangePixels: 0
    readonly property string statisticsText: {
        const motionSummary = root.motionSampleCount > 0
            ? "Motion " + root.motionFramesPerSecond.toFixed(1) + " FPS  ·  frozen " + root.frozenMotionFrameCount + "/" + root.motionSampleCount
            : "Motion idle";
        return motionSummary + "\nAnchor " + root.anchorCorrectionCount + " (max " + root.maximumAnchorCorrectionPixels.toFixed(1) + " px)" + "  ·  reflow " + root.rowGeometryChangeCount + " (max " + root.maximumRowGeometryChangePixels.toFixed(1) + " px)";
    }

    function clearPublishedMotionStatistics() {
        motionSampleCount = 0;
        motionUpdateCount = 0;
        frozenMotionFrameCount = 0;
        motionFramesPerSecond = 0;
    }

    function resetStatistics() {
        motionSamples = [];
        previousFrameTimestamp = Number.NaN;
        previousContentY = Number.NaN;
        previousMoving = false;
        lastPublicationTimestamp = Number.NaN;
        anchorCorrectionCount = 0;
        maximumAnchorCorrectionPixels = 0;
        rowGeometryChangeCount = 0;
        maximumRowGeometryChangePixels = 0;
        clearPublishedMotionStatistics();
    }

    function publishStatistics() {
        if (motionSamples.length === 0) {
            clearPublishedMotionStatistics();
            return;
        }

        let totalMilliseconds = 0;
        let updateCount = 0;
        for (const sample of motionSamples) {
            totalMilliseconds += Number(sample.interval);
            if (sample.updated)
                ++updateCount;
        }
        motionSampleCount = motionSamples.length;
        motionUpdateCount = updateCount;
        frozenMotionFrameCount = motionSampleCount - motionUpdateCount;
        motionFramesPerSecond = totalMilliseconds > 0 ? 1000 * motionUpdateCount / totalMilliseconds : 0;
    }

    function recordFrameAt(timestampMilliseconds, contentY, moving) {
        const timestamp = Number(timestampMilliseconds);
        const position = Number(contentY);
        const isMoving = Boolean(moving);
        if (!Number.isFinite(timestamp) || !Number.isFinite(position))
            return;

        if (Number.isFinite(previousFrameTimestamp) && Number.isFinite(previousContentY) && previousMoving && isMoving) {
            const interval = timestamp - previousFrameTimestamp;
            if (interval > 0) {
                motionSamples.push({
                    interval: interval,
                    updated: Math.abs(position - previousContentY) >= movementEpsilon
                });
                if (motionSamples.length > maximumSampleCount)
                    motionSamples.splice(0, motionSamples.length - maximumSampleCount);
            }
        }

        previousFrameTimestamp = timestamp;
        previousContentY = position;
        previousMoving = isMoving;
        if (!Number.isFinite(lastPublicationTimestamp))
            lastPublicationTimestamp = timestamp;
        if (timestamp - lastPublicationTimestamp >= publicationIntervalMilliseconds) {
            publishStatistics();
            lastPublicationTimestamp = timestamp;
        }
    }

    function recordAnchorCorrection(displacement) {
        const absoluteDisplacement = Math.abs(Number(displacement));
        if (!Number.isFinite(absoluteDisplacement) || absoluteDisplacement < movementEpsilon)
            return;
        ++anchorCorrectionCount;
        maximumAnchorCorrectionPixels = Math.max(maximumAnchorCorrectionPixels, absoluteDisplacement);
    }

    function recordRowGeometryChange(heightDelta) {
        const absoluteDelta = Math.abs(Number(heightDelta));
        if (!Number.isFinite(absoluteDelta) || absoluteDelta < movementEpsilon)
            return;
        ++rowGeometryChangeCount;
        maximumRowGeometryChangePixels = Math.max(maximumRowGeometryChangePixels, absoluteDelta);
    }

    property FrameAnimation frameSampler: FrameAnimation {
        running: root.active && root.targetViewport !== null
        onTriggered: root.recordFrameAt(elapsedTime * 1000, root.targetViewport.contentY, root.targetViewport.moving)
    }

    property Connections viewportConnections: Connections {
        target: root.targetViewport
        enabled: root.active && root.targetViewport !== null
        ignoreUnknownSignals: true

        function onAnchorPositionCorrected(displacement) {
            root.recordAnchorCorrection(displacement);
        }

        function onRowGeometryChanged(sourceRow, heightDelta) {
            root.recordRowGeometryChange(heightDelta);
        }
    }

    onActiveChanged: resetStatistics()
}
