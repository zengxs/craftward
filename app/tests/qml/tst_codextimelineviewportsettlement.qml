// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Window
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 800
    height: 600
    property var viewport: null
    property bool sampling: false
    property var movingFrames: []
    property var stoppedFrames: []
    property bool columnLayout: false
    property var heightPattern: [48, 72, 96]

    function snapshot() {
        return {
            contentY: viewport.contentY,
            maximumContentY: viewport.maximumContentY,
            velocity: viewport.verticalVelocity,
            rows: viewport.visibleRowOffsetsForBenchmark(),
            trajectory: viewport.trajectoryRowOffsetsForBenchmark()
        };
    }

    Connections {
        target: suite.Window.window

        function onFrameSwapped() {
            if (!suite.sampling || !suite.viewport)
                return;
            const frame = suite.snapshot();
            if (suite.viewport.moving)
                suite.movingFrames = suite.movingFrames.concat([frame]);
            else
                suite.stoppedFrames = suite.stoppedFrames.concat([frame]);
        }
    }

    Timer {
        interval: 16
        repeat: true
        running: suite.sampling
        onTriggered: suite.Window.window.update()
    }

    ListModel {
        id: sourceModel

        property int revision: 0

        function entryIdAt(row) {
            return row >= 0 && row < count ? get(row).entryId : "";
        }

        function indexOfEntryId(entryId) {
            const row = Number(String(entryId).slice(6));
            return row >= 0 && row < count && entryIdAt(row) === entryId ? row : -1;
        }
    }

    Component {
        id: viewportComponent

        Pages.CodexTimelineViewport {
            width: 600
            height: 600
            timelineModel: sourceModel
            heightCacheNamespace: "current"
            estimatedRowHeight: 72
            bottomContentInset: 64
            rowDelegate: Component {
                Item {
                    property int sourceRow: -1
                    property int dataRevision: -1
                    readonly property string entryId: sourceModel.entryIdAt(sourceRow)
                    readonly property string heightCacheKey: "current:" + entryId

                    function prepareForLayout() {
                        rowColumn.forceLayout();
                    }

                    implicitHeight: suite.columnLayout ? rowColumn.implicitHeight : rowContent.height

                    Column {
                        id: rowColumn

                        width: parent.width

                        Rectangle {
                            id: rowContent

                            width: parent.width
                            height: sourceRow >= 388 ? suite.heightPattern[Math.abs(sourceRow) % 3] : 72
                            color: sourceRow % 2 === 0 ? "lightsteelblue" : "lightgray"
                        }
                    }
                }
            }
        }
    }

    TestCase {
        name: "CodexTimelineViewportSettlement"
        when: windowShown

        function cleanup() {
            suite.sampling = false;
            if (suite.viewport)
                suite.viewport.destroy();
            suite.viewport = null;
            suite.movingFrames = [];
            suite.stoppedFrames = [];
            wait(0);
            sourceModel.clear();
            ++sourceModel.revision;
        }

        function compareRows(before, after, contentCoordinates, requireAll) {
            const current = {};
            for (const row of after.rows)
                current[row.entryId] = row;
            const result = {
                common: 0,
                markers: 0,
                drift: 0,
                reverse: 0,
                missing: [],
                changes: []
            };
            const contentDelta = contentCoordinates ? after.contentY - before.contentY : 0;
            for (const previous of before.rows) {
                const row = current[previous.entryId];
                if (!row) {
                    if (requireAll)
                        result.missing.push(previous.entryId);
                    continue;
                }
                ++result.common;
                const drift = Math.abs(row.offset - previous.offset + contentDelta);
                result.drift = Math.max(result.drift, drift);
                if (contentCoordinates)
                    result.reverse = Math.max(result.reverse, row.offset - previous.offset);
                if (drift > 1)
                    result.changes.push(previous.entryId + ":" + drift);
                const markers = {};
                for (const marker of row.visualMarkers)
                    markers[marker.markerId] = marker.offset;
                for (const marker of previous.visualMarkers) {
                    if (markers[marker.markerId] === undefined) {
                        if (requireAll)
                            result.missing.push(previous.entryId + "/" + marker.markerId);
                        continue;
                    }
                    ++result.markers;
                    const markerDrift = Math.abs(markers[marker.markerId] - marker.offset + contentDelta);
                    result.drift = Math.max(result.drift, markerDrift);
                    if (markerDrift > 1)
                        result.changes.push(previous.entryId + "/" + marker.markerId + ":" + markerDrift);
                }
            }
            return result;
        }

        function test_preservesEveryPresentedRowWhenAFastFlickReachesTheBottomBoundary_data() {
            return [
                {
                    tag: "warm-tail",
                    prewarmTail: true
                },
                {
                    tag: "cold-tail",
                    prewarmTail: false
                },
                {
                    tag: "cold-tail-column",
                    prewarmTail: false,
                    columnLayout: true
                },
                {
                    tag: "expanding-tail-column",
                    prewarmTail: false,
                    columnLayout: true,
                    heightPattern: [72, 144, 240],
                    estimatedBoundary: true
                }
            ];
        }

        function test_preservesEveryPresentedRowWhenAFastFlickReachesTheBottomBoundary(data) {
            suite.columnLayout = data.columnLayout ?? false;
            suite.heightPattern = data.heightPattern ?? [48, 72, 96];
            for (let row = 0; row < 400; ++row)
                sourceModel.append({
                    entryId: "entry:" + row
                });
            ++sourceModel.revision;
            suite.viewport = createTemporaryObject(viewportComponent, suite, {
                followLiveTail: data.prewarmTail
            });
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.delegateForEntry(data.prewarmTail ? "entry:399" : "entry:0") !== null);
            wait(300);
            suite.viewport.followLiveTail = false;
            const scrollViewport = findChild(suite.viewport, "codexTimelineScrollViewport");
            verify(scrollViewport !== null);
            scrollViewport.positionViewAtIndex(60, ListView.Beginning);
            wait(300);
            const cachedCount = Object.keys(suite.viewport.rowHeights).length;
            verify(cachedCount > 0 && cachedCount < sourceModel.count, "The height cache must be partially warm");
            const start = suite.snapshot();
            verify(start.rows.length > 1);
            suite.sampling = true;

            suite.viewport.flickContentForBenchmark(32000, 16000);

            tryVerify(() => Math.abs(suite.viewport.verticalVelocity) >= 30000, 200);
            tryVerify(() => suite.stoppedFrames.length >= 6, 5000);
            suite.sampling = false;
            verify(suite.movingFrames.length > 0);
            const anchor = suite.viewport.movementEndedAnchorForBenchmark();
            const baseline = {
                contentY: anchor.contentY,
                rows: suite.viewport.movementEndedRowsForBenchmark()
            };
            verify(baseline.contentY - start.contentY >= 24000, "Travel: " + JSON.stringify({
                start: start.contentY,
                end: baseline.contentY,
                first: start.rows[0].row,
                last: baseline.rows[0].row
            }));
            verify(baseline.rows[0].row - start.rows[0].row >= 100);
            verify(suite.viewport.followLiveTail || (data.estimatedBoundary && baseline.contentY >= start.maximumContentY - 1), "The flick must reach the bottom boundary: " + JSON.stringify({
                start: start.contentY,
                end: baseline.contentY,
                maximum: suite.viewport.maximumContentY,
                rows: baseline.rows
            }));
            verify(baseline.rows.filter(row => row.row >= 388 && suite.heightPattern[row.row % 3] !== 72).length >= 2, "At least two visible rows must have non-estimated content heights: " + JSON.stringify(baseline.rows));

            for (let frame = 1; frame < suite.movingFrames.length; ++frame) {
                const previous = suite.movingFrames[frame - 1];
                const current = suite.movingFrames[frame];
                const moving = compareRows({
                    contentY: previous.contentY,
                    rows: previous.trajectory
                }, {
                    contentY: current.contentY,
                    rows: current.trajectory
                }, true, false);
                verify(moving.common > 0 && moving.drift <= 1 && moving.reverse <= 1, "Moving frame " + frame + ": " + JSON.stringify(moving));
            }

            const lastMoving = suite.movingFrames[suite.movingFrames.length - 1];
            const boundary = compareRows(lastMoving, baseline, true, false);
            verify(boundary.common > 0 && boundary.markers > 0, "The last moving frame must cover the movement-end snapshot");
            verify(boundary.drift <= 1, "Movement-boundary drift: " + JSON.stringify(boundary));
            for (let frame = 0; frame < suite.stoppedFrames.length; ++frame) {
                const stopped = compareRows(baseline, suite.stoppedFrames[frame], false, true);
                verify(stopped.common > 1 && stopped.markers > 0 && stopped.drift <= 1 && stopped.missing.length === 0, "Stopped frame " + frame + ": " + JSON.stringify(stopped));
            }
        }
    }
}
