// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import QtTest
import Craftward.Components
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 480
    height: 280

    property var viewport
    property int delegateReuseCount: 0
    property int coalescedHeightStep: 0
    property int resizeStep: 0

    function submitHeightDuringMovement() {
        if (coalescedHeightStep >= 8)
            return;
        const row = coalescedHeightStep;
        viewport.recordRowHeight(row, "entry-" + row, 100 + row * 4);
        ++coalescedHeightStep;
        if (coalescedHeightStep < 8)
            Qt.callLater(suite.submitHeightDuringMovement);
    }

    function submitResize() {
        const widths = [360, 280, 440, 320, 400, 300];
        if (resizeStep >= widths.length)
            return;
        viewport.width = widths[resizeStep];
        ++resizeStep;
        if (resizeStep < widths.length)
            Qt.callLater(suite.submitResize);
    }

    ListModel {
        id: timelineModel
    }

    ListModel {
        id: documentModel
    }

    Component {
        id: viewportComponent

        Pages.CodexTimelineViewport {
            width: 420
            height: 220
            loading: false
            layoutKey: "thread-1"
            rowKeyProvider: row => row >= 0 && row < timelineModel.count ? timelineModel.get(row).entryId : ""
            estimatedRowHeight: 50
            rowSpacing: 10
            model: timelineModel

            delegate: Item {
                id: rowDelegate

                required property int row
                required property string entryId
                required property real desiredHeight
                readonly property var viewport: TableView.view

                implicitHeight: desiredHeight

                function reportHeight() {
                    heightReportTimer.restart();
                }

                Component.onCompleted: reportHeight()
                onImplicitHeightChanged: reportHeight()
                onWidthChanged: reportHeight()
                TableView.onReused: {
                    suite.delegateReuseCount += 1;
                    reportHeight();
                }

                Timer {
                    id: heightReportTimer

                    interval: 1
                    onTriggered: {
                        if (rowDelegate.viewport)
                            rowDelegate.viewport.recordRowHeight(rowDelegate.row, rowDelegate.entryId, rowDelegate.implicitHeight);
                    }
                }
            }
        }
    }

    Component {
        id: markupViewportComponent

        Pages.CodexTimelineViewport {
            width: 420
            height: 220
            loading: false
            layoutKey: "thread-1"
            rowKeyProvider: row => row >= 0 && row < timelineModel.count ? timelineModel.get(row).entryId : ""
            estimatedRowHeight: 100
            rowSpacing: 10
            model: timelineModel

            delegate: Item {
                id: markupDelegate

                required property int row
                required property string entryId
                readonly property var viewport: TableView.view

                implicitHeight: messageCard.implicitHeight

                function reportHeight() {
                    heightReportTimer.restart();
                }

                Component.onCompleted: reportHeight()
                onImplicitHeightChanged: reportHeight()
                onWidthChanged: reportHeight()
                TableView.onReused: reportHeight()

                Rectangle {
                    id: messageCard

                    width: parent.width * 0.86
                    implicitHeight: messageContent.implicitHeight + 24

                    ColumnLayout {
                        id: messageContent

                        anchors {
                            fill: parent
                            margins: 12
                        }

                        MarkupDocumentView {
                            objectName: "markupDocument"
                            Layout.fillWidth: true
                            documentModel: documentModel
                        }
                    }
                }

                Timer {
                    id: heightReportTimer

                    interval: 1
                    onTriggered: {
                        if (markupDelegate.viewport)
                            markupDelegate.viewport.recordRowHeight(markupDelegate.row, markupDelegate.entryId, markupDelegate.implicitHeight);
                    }
                }
            }
        }
    }

    TestCase {
        name: "CodexTimelineViewport"
        when: windowShown

        function appendRows(count, height) {
            const first = timelineModel.count;
            for (let index = 0; index < count; ++index) {
                timelineModel.append({
                    "entryId": "entry-" + (first + index),
                    "desiredHeight": height
                });
            }
        }

        function appendVariableRows(count) {
            const first = timelineModel.count;
            for (let index = 0; index < count; ++index) {
                const row = first + index;
                timelineModel.append({
                    "entryId": "entry-" + row,
                    "desiredHeight": row % 3 === 0 ? 180 : (row % 3 === 1 ? 44 : 108)
                });
            }
        }

        function visualY(row) {
            const item = suite.viewport.itemAtCell(Qt.point(0, row));
            verify(item !== null, "Expected the anchor row to be loaded");
            return item.mapToItem(suite.viewport, 0, 0).y;
        }

        function init() {
            timelineModel.clear();
            documentModel.clear();
            suite.delegateReuseCount = 0;
            suite.coalescedHeightStep = 0;
            suite.resizeStep = 0;
            suite.viewport = viewportComponent.createObject(suite);
            verify(suite.viewport !== null);
        }

        function cleanup() {
            suite.viewport.destroy();
            suite.viewport = null;
        }

        function test_contentGeometryUsesMeasuredAndEstimatedRows() {
            timelineModel.append({
                "entryId": "short",
                "desiredHeight": 40
            });
            timelineModel.append({
                "entryId": "medium",
                "desiredHeight": 80
            });
            timelineModel.append({
                "entryId": "long",
                "desiredHeight": 120
            });

            suite.viewport.recordRowHeight(0, "short", 40);
            suite.viewport.recordRowHeight(1, "medium", 80);
            suite.viewport.recordRowHeight(2, "long", 120);
            tryCompare(suite.viewport, "contentHeight", 260);
            const stableHeight = suite.viewport.contentHeight;
            suite.viewport.positionViewAtRow(2, TableView.AlignBottom);
            wait(80);
            compare(suite.viewport.contentHeight, stableHeight);
        }

        function test_timelineRowsAreReused() {
            suite.viewport.followLiveTail = false;
            appendRows(100, 50);
            tryCompare(suite.viewport, "rows", 100);
            suite.viewport.forceLayout();
            suite.viewport.positionViewAtRow(0, TableView.AlignTop);
            tryCompare(suite.viewport, "topRow", 0);
            suite.delegateReuseCount = 0;

            suite.viewport.positionViewAtRow(90, TableView.AlignTop);
            tryCompare(suite.viewport, "topRow", 90);
            wait(40);

            verify(suite.delegateReuseCount > 0);
        }

        function test_contentHeightIsPublishedAfterVisibleRowGeometry() {
            appendRows(20, 50);
            tryCompare(suite.viewport, "contentHeight", 1190);
            wait(20);
            const publishedRowHeights = [];
            const observeGeometry = function () {
                if (Math.abs(suite.viewport.contentHeight - 1230) < 0.5)
                    publishedRowHeights.push(suite.viewport.rowHeight(19));
            };
            suite.viewport.contentHeightChanged.connect(observeGeometry);

            suite.viewport.recordRowHeight(19, "entry-19", 90);
            wait(20);
            suite.viewport.contentHeightChanged.disconnect(observeGeometry);

            compare(publishedRowHeights.length, 1, JSON.stringify(publishedRowHeights));
            compare(publishedRowHeights[0], 90);
        }

        function test_heightChangeAboveViewportPreservesAnchor() {
            appendRows(20, 50);
            tryCompare(suite.viewport, "contentHeight", 1190);
            suite.viewport.followLiveTail = false;
            suite.viewport.positionViewAtRow(10, TableView.AlignTop);
            wait(80);

            const anchorRow = suite.viewport.topRow;
            const anchorY = visualY(anchorRow);
            compare(anchorRow, 10);

            suite.viewport.recordRowHeight(anchorRow - 1, "entry-" + (anchorRow - 1), 90);
            wait(20);
            fuzzyCompare(visualY(anchorRow), anchorY, 0.5);
            compare(suite.viewport.topRow, anchorRow);
        }

        function test_recycledAnchorFallsBackToMeasuredHeightDelta() {
            appendRows(20, 160);
            timelineModel.setProperty(11, "desiredHeight", 20);
            tryVerify(() => suite.viewport.rows === 20);
            for (let row = 0; row < timelineModel.count; ++row) {
                suite.viewport.recordRowHeight(row, "entry-" + row, timelineModel.get(row).desiredHeight);
            }
            tryCompare(suite.viewport, "contentHeight", 3250);
            suite.viewport.followLiveTail = false;
            suite.viewport.forceLayout();
            suite.viewport.positionViewAtRow(10, TableView.AlignTop);
            tryCompare(suite.viewport, "topRow", 10);
            suite.viewport.contentY += 150;
            wait(20);

            compare(suite.viewport.topRow, 10);
            const markerY = visualY(12);
            timelineModel.setProperty(10, "desiredHeight", 1);
            tryVerify(() => suite.viewport.rowHeight(10) === 1);
            wait(20);

            fuzzyCompare(visualY(12), markerY, 0.5);
        }

        function test_rowReflowWaitsUntilAnActiveFlickEnds() {
            appendRows(30, 160);
            tryVerify(() => suite.viewport.rows === 30);
            for (let row = 0; row < timelineModel.count; ++row)
                suite.viewport.recordRowHeight(row, "entry-" + row, 160);
            tryCompare(suite.viewport, "contentHeight", 5090);
            suite.viewport.followLiveTail = false;
            suite.viewport.forceLayout();
            suite.viewport.positionViewAtRow(20, TableView.AlignTop);
            tryCompare(suite.viewport, "topRow", 20);

            suite.viewport.flick(0, 2000);
            tryVerify(() => suite.viewport.flicking);
            wait(20);
            const changedRow = suite.viewport.topRow;
            timelineModel.setProperty(changedRow, "desiredHeight", 220);
            wait(20);

            compare(suite.viewport.rowHeight(changedRow), 160);
            verify(suite.viewport.flicking);
            const contentYDuringFlick = suite.viewport.contentY;
            wait(50);
            verify(suite.viewport.contentY < contentYDuringFlick - 0.5);

            suite.viewport.cancelFlick();
            tryVerify(() => !suite.viewport.moving);
            tryVerify(() => suite.viewport.rowHeight(changedRow) === 220);
        }

        function test_followLatestTracksAppendsAndLastRowGrowth() {
            appendRows(12, 50);
            suite.viewport.followLatest();
            tryVerify(() => suite.viewport.atYEnd);

            timelineModel.append({
                "entryId": "entry-12",
                "desiredHeight": 90
            });
            tryVerify(() => suite.viewport.atYEnd);

            suite.viewport.recordRowHeight(12, "entry-12", 140);
            tryVerify(() => suite.viewport.atYEnd);
        }

        function test_widthChangeKeepsExistingHeightsAsProvisionalGeometry() {
            appendRows(20, 50);
            tryCompare(suite.viewport, "contentHeight", 1190);
            suite.viewport.followLiveTail = false;
            suite.viewport.recordRowHeight(2, "entry-2", 200);
            tryCompare(suite.viewport, "contentHeight", 1340);

            suite.viewport.width += 20;
            wait(20);
            compare(suite.viewport.contentHeight, 1340);
        }

        function test_widthChangePreservesTheVisibleAnchor() {
            appendRows(12, 80);
            tryVerify(() => suite.viewport.rows === 12);
            for (let row = 0; row < timelineModel.count; ++row)
                suite.viewport.recordRowHeight(row, "entry-" + row, 80);
            tryCompare(suite.viewport, "contentHeight", 1070);
            suite.viewport.followLiveTail = false;
            suite.viewport.positionViewAtRow(6, TableView.AlignTop);
            wait(40);

            const anchorRow = suite.viewport.topRow;
            const anchorY = visualY(anchorRow);
            suite.viewport.width -= 100;
            wait(20);

            compare(suite.viewport.topRow, anchorRow);
            fuzzyCompare(visualY(anchorRow), anchorY, 0.5);
        }

        function test_layoutKeyChangeStartsWithFreshGeometryAtTheTail() {
            appendRows(20, 50);
            tryCompare(suite.viewport, "contentHeight", 1190);
            suite.viewport.followLiveTail = false;
            suite.viewport.recordRowHeight(2, "entry-2", 200);
            tryCompare(suite.viewport, "contentHeight", 1340);

            suite.viewport.layoutKey = "thread-2";
            tryCompare(suite.viewport, "contentHeight", 1190);
            tryVerify(() => suite.viewport.followLiveTail && suite.viewport.atYEnd);
        }

        function test_markdownReflowsAndExpandsItsRowWhenWidthChanges() {
            suite.viewport.destroy();
            timelineModel.clear();
            timelineModel.append({
                "entryId": "entry-0",
                "desiredHeight": 100
            });
            documentModel.append({
                "blockId": "prose:0",
                "codeBlock": false,
                "blockText": "This long paragraph contains enough words to require several wrapped lines when the timeline becomes narrow. It should remain fully visible and the message height should grow after resizing.",
                "plainText": "This long paragraph contains enough words to require several wrapped lines when the timeline becomes narrow. It should remain fully visible and the message height should grow after resizing.",
                "language": "",
                "markdown": true
            });
            suite.viewport = markupViewportComponent.createObject(suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.rows === 1);
            wait(30);

            const item = suite.viewport.itemAtCell(Qt.point(0, 0));
            verify(item !== null);
            const markup = findChild(item, "markupDocument");
            verify(markup !== null);
            const wideWidth = markup.width;
            const wideHeight = markup.implicitHeight;

            suite.viewport.width = 300;
            tryVerify(() => markup.width < wideWidth);
            tryVerify(() => markup.implicitHeight > wideHeight);
            tryVerify(() => item.height + 0.5 >= item.implicitHeight);
        }

        function test_movementDefersHeightCommitsUntilItEnds() {
            appendRows(60, 80);
            tryCompare(suite.viewport, "rows", 60);
            suite.viewport.followLiveTail = false;
            for (let row = 0; row < timelineModel.count; ++row)
                suite.viewport.recordRowHeight(row, "entry-" + row, 80);
            tryCompare(suite.viewport, "contentHeight", 5390);
            suite.viewport.forceLayout();
            suite.viewport.contentY = suite.viewport.originY + 45 * 90;
            tryCompare(suite.viewport, "topRow", 45);
            suite.viewport.flick(0, 1800);
            tryVerify(() => suite.viewport.moving);

            let contentHeightChanges = 0;
            const recordContentHeightChange = function () {
                ++contentHeightChanges;
            };
            suite.viewport.contentHeightChanged.connect(recordContentHeightChange);
            suite.submitHeightDuringMovement();
            tryCompare(suite, "coalescedHeightStep", 8);
            wait(30);

            compare(contentHeightChanges, 0);
            compare(suite.viewport.contentHeight, 5390);

            suite.viewport.cancelFlick();
            tryVerify(() => !suite.viewport.moving);
            tryCompare(suite.viewport, "contentHeight", 5662);
            suite.viewport.contentHeightChanged.disconnect(recordContentHeightChange);

            compare(contentHeightChanges, 1);
            compare(suite.viewport.contentHeight, 5662);
        }

        function test_resizeStormConvergesViewportGeometry() {
            appendRows(40, 80);
            tryCompare(suite.viewport, "rows", 40);
            suite.submitResize();
            tryCompare(suite, "resizeStep", 6);
            wait(30);

            compare(suite.viewport.width, 300);
            fuzzyCompare(suite.viewport.contentWidth, suite.viewport.width, 0.5);
            fuzzyCompare(suite.viewport.columnWidth(0), suite.viewport.width, 0.5);
            fuzzyCompare(suite.viewport.contentX, suite.viewport.originX, 0.5);
            const item = suite.viewport.itemAtCell(Qt.point(0, suite.viewport.topRow));
            verify(item !== null);
            fuzzyCompare(item.width, suite.viewport.width, 0.5);
        }

        function test_sameLayoutRestoresCachedHeightByEntryId() {
            appendRows(20, 50);
            tryCompare(suite.viewport, "contentHeight", 1190);
            suite.viewport.recordRowHeight(2, "entry-2", 200);
            tryCompare(suite.viewport, "contentHeight", 1340);

            timelineModel.clear();
            appendRows(20, 50);
            tryCompare(suite.viewport, "contentHeight", 1340);
        }

        function test_changedRowKeysDoNotRetainMeasurementsByPosition() {
            appendRows(3, 50);
            tryCompare(suite.viewport, "contentHeight", 170);
            suite.viewport.recordRowHeight(1, "entry-1", 180);
            tryCompare(suite.viewport, "contentHeight", 300);

            timelineModel.setProperty(1, "entryId", "replacement-1");
            suite.viewport._synchronizeRows(false);

            compare(suite.viewport._rowKeys[1], "replacement-1");
            compare(suite.viewport._rowHeights[1], 50);
        }

        function test_slowUpwardScrollDoesNotReverseAVisibleRow() {
            appendVariableRows(30);
            tryVerify(() => suite.viewport.rows === 30);
            wait(30);
            suite.viewport.followLiveTail = false;
            suite.viewport.positionViewAtRow(20, TableView.AlignTop);
            wait(40);

            const markerRow = suite.viewport.topRow;
            let previousVisualY = visualY(markerRow);
            for (let step = 0; step < 10; ++step) {
                suite.viewport.contentY = Math.max(suite.viewport.originY, suite.viewport.contentY - 8);
                for (let sample = 0; sample < 8; ++sample) {
                    wait(2);
                    const currentVisualY = visualY(markerRow);
                    verify(currentVisualY + 0.5 >= previousVisualY, "row " + markerRow + " reversed from " + previousVisualY + " to " + currentVisualY);
                    previousVisualY = currentVisualY;
                }
            }
        }
    }
}
