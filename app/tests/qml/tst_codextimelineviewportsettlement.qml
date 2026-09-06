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
    property real minimumMovingSampleDistance: 0
    property real maximumObservedVelocity: 0

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
            if (suite.viewport.moving) {
                suite.maximumObservedVelocity = Math.max(suite.maximumObservedVelocity, Math.abs(frame.velocity));
                if (suite.movingFrames.length > 0 && Math.abs(frame.contentY - suite.movingFrames[suite.movingFrames.length - 1].contentY) < suite.minimumMovingSampleDistance)
                    return;
                suite.movingFrames = suite.movingFrames.concat([frame]);
            } else {
                suite.stoppedFrames = suite.stoppedFrames.concat([frame]);
            }
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
            suite.maximumObservedVelocity = 0;
            wait(0);
            sourceModel.clear();
            ++sourceModel.revision;
        }

        function compareStoppedRows(before, after) {
            const current = {};
            for (const row of after.rows)
                current[row.entryId] = row;
            const result = {
                common: 0,
                markers: 0,
                drift: 0,
                missing: [],
                changes: []
            };
            for (const previous of before.rows) {
                const row = current[previous.entryId];
                if (!row) {
                    result.missing.push(previous.entryId);
                    continue;
                }
                ++result.common;
                const drift = Math.abs(row.offset - previous.offset);
                result.drift = Math.max(result.drift, drift);
                if (drift > 1)
                    result.changes.push(previous.entryId + ":" + drift);
                const markers = {};
                for (const marker of row.visualMarkers)
                    markers[marker.markerId] = marker.offset;
                for (const marker of previous.visualMarkers) {
                    if (markers[marker.markerId] === undefined) {
                        result.missing.push(previous.entryId + "/" + marker.markerId);
                        continue;
                    }
                    ++result.markers;
                    const markerDrift = Math.abs(markers[marker.markerId] - marker.offset);
                    result.drift = Math.max(result.drift, markerDrift);
                    if (markerDrift > 1)
                        result.changes.push(previous.entryId + "/" + marker.markerId + ":" + markerDrift);
                }
            }
            return result;
        }

        function expectedRowHeight(row) {
            return row >= 388 ? suite.heightPattern[row % 3] : 72;
        }

        function verifyFrameGeometry(frame, rowTops, origin, label) {
            verify(frame.rows.length > 1, label + " must contain visible rows");
            let markerCount = 0;
            for (const row of frame.rows.concat(frame.trajectory ?? [])) {
                compare(row.entryId, sourceModel.entryIdAt(row.row), label + " row identity");
                const top = origin + rowTops[row.row];
                const height = expectedRowHeight(row.row);
                const drift = Math.abs(row.offset + frame.contentY - top);
                verify(drift <= 1, label + ": " + row.entryId + " drifted " + drift + " px");
                compare(row.height, height, label + ": " + row.entryId + " height");
                for (const marker of row.visualMarkers) {
                    ++markerCount;
                    verify(marker.markerId === "content:start" || marker.markerId === "content:end");
                    const expectedOffset = top + (marker.markerId === "content:end" ? height : 0) - frame.contentY;
                    verify(Math.abs(marker.offset - expectedOffset) <= 1, label + ": " + row.entryId + "/" + marker.markerId + " drifted " + (marker.offset - expectedOffset) + " px");
                }
            }
            verify(markerCount > 0, label + " must contain content markers");
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
                },
                {
                    tag: "sparse-moving-frames",
                    prewarmTail: false,
                    columnLayout: true,
                    minimumMovingSampleDistance: 6000
                }
            ];
        }

        function test_preservesEveryPresentedRowWhenAFastFlickReachesTheBottomBoundary(data) {
            suite.columnLayout = data.columnLayout ?? false;
            suite.heightPattern = data.heightPattern ?? [48, 72, 96];
            suite.minimumMovingSampleDistance = data.minimumMovingSampleDistance ?? 0;
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
            if (data.prewarmTail) {
                tryVerify(() => Math.abs(suite.viewport.anchorOffsetForBenchmark({
                        entryId: "entry:399",
                        row: 399
                    }) + expectedRowHeight(399) + suite.viewport.bottomContentInset - suite.viewport.height) <= 1, 5000, "The warm tail must be positioned above the composer inset");
            }
            verify(waitForRendering(suite.viewport));
            suite.viewport.followLiveTail = false;
            const scrollViewport = findChild(suite.viewport, "codexTimelineScrollViewport");
            verify(scrollViewport !== null);
            scrollViewport.positionViewAtIndex(60, ListView.Beginning);
            tryVerify(() => suite.viewport.delegateForEntry("entry:60") !== null && Math.abs(suite.viewport.anchorOffsetForBenchmark({
                    entryId: "entry:60",
                    row: 60
                })) <= 1 && !suite.viewport.viewportUpdateScheduled && !suite.viewport.anchorRestoreRunning);
            verify(waitForRendering(suite.viewport));
            const cachedCount = Object.keys(suite.viewport.rowHeights).length;
            verify(cachedCount > 0 && cachedCount < sourceModel.count, "The height cache must be partially warm");
            const start = suite.snapshot();
            verify(start.rows.length > 1);
            // The fixture has known heights, so every sample can be checked independently.
            // Adjacent frames need not share a row when the runner delays presentation.
            const rowTops = [0];
            for (let row = 0; row < sourceModel.count; ++row)
                rowTops.push(rowTops[row] + expectedRowHeight(row) + suite.viewport.rowSpacing);
            const origin = start.rows[0].offset + start.contentY - rowTops[start.rows[0].row];
            verifyFrameGeometry(start, rowTops, origin, "Start");
            suite.sampling = true;

            suite.viewport.flickContentForBenchmark(32000, 16000);

            tryVerify(() => suite.stoppedFrames.length >= 6, 5000);
            suite.sampling = false;
            verify(suite.maximumObservedVelocity >= 30000, "The stress flick reached only " + suite.maximumObservedVelocity + " px/s in presented frames");
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

            let previous = start;
            let disjointFrameCount = 0;
            for (let frame = 0; frame < suite.movingFrames.length; ++frame) {
                const current = suite.movingFrames[frame];
                verifyFrameGeometry(current, rowTops, origin, "Moving frame " + frame);
                verify(current.contentY >= previous.contentY - 1, "Moving frame " + frame + " reversed scroll direction");
                if (frame > 0 && !previous.trajectory.some(before => current.trajectory.some(after => before.entryId === after.entryId)))
                    ++disjointFrameCount;
                previous = current;
            }
            if (suite.minimumMovingSampleDistance > 0)
                verify(disjointFrameCount > 0, "Sparse samples must exercise frames with no common rows");
            verifyFrameGeometry(baseline, rowTops, origin, "Movement boundary");
            verify(baseline.contentY >= previous.contentY - 1, "Movement end reversed scroll direction");
            for (let frame = 0; frame < suite.stoppedFrames.length; ++frame) {
                const stopped = compareStoppedRows(baseline, suite.stoppedFrames[frame]);
                verify(stopped.common > 1 && stopped.markers > 0 && stopped.drift <= 1 && stopped.missing.length === 0, "Stopped frame " + frame + ": " + JSON.stringify(stopped));
            }
        }
    }
}
