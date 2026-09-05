// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import Craftward.Components
import "../../qml/Craftward/Pages" as Pages
import "CodexTimelineTestFixtures.js" as Fixtures

Item {
    id: suite

    width: 720
    height: 480
    property var viewport
    property bool hasRunningEvidence: false
    property bool activityShimmerEnabled: false
    property bool forkEnabled: true
    property bool showForkActions: true
    property string lastForkedTurnId

    ListModel {
        id: messageSegments

        ListElement {
            segmentId: "message-segment"
            codeBlock: false
            segmentText: "Rendered message"
            language: ""
            markdown: true
        }
    }

    QtObject {
        id: messageDocument

        property var renderModel: messageSegments
    }

    ListModel {
        id: fakeTimelineModel

        property int revision: 0
        property var rows: []
        readonly property int totalRowCount: count

        function resetRows(nextRows) {
            rows = nextRows;
            clear();
            for (const row of nextRows) {
                append({
                    entryId: String(row.entryId)
                });
            }
            ++revision;
        }

        function entryIdAt(sourceRow) {
            const row = rows[sourceRow];
            return row ? row.entryId : "";
        }

        function valueAt(sourceRow, roleName) {
            const row = rows[sourceRow];
            return row ? row[roleName] : undefined;
        }

        function indexOfEntryId(entryId) {
            const target = String(entryId);
            for (let row = 0; row < count; ++row) {
                if (entryIdAt(row) === target)
                    return row;
            }
            return -1;
        }
    }

    Component {
        id: rowComponent

        Pages.CodexTimelineRow {
            timelineModel: fakeTimelineModel
            turnExpanded: false
            hasRunningEvidence: suite.hasRunningEvidence
            activityShimmerEnabled: suite.activityShimmerEnabled
            forkEnabled: suite.forkEnabled
            showForkActions: suite.showForkActions
            wallClockUnixMilliseconds: 0
            onForkRequested: turnId => suite.lastForkedTurnId = turnId
        }
    }

    Component {
        id: viewportComponent

        Pages.CodexTimelineViewport {
            width: 680
            height: 360
            timelineModel: fakeTimelineModel
            rowDelegate: rowComponent
            bottomContentInset: 0
            contentHorizontalInset: 20
            contentMaximumWidth: 640
            estimatedRowHeight: 120
        }
    }

    TestCase {
        name: "CodexTimelineRowIntegration"
        when: windowShown

        function messageRow(overrides) {
            return Object.assign({
                entryId: "message:turn-1:message-1",
                turnId: "turn-1",
                turnForkable: true,
                latestTurn: true,
                activityGroup: false,
                fromUser: false,
                finalAnswer: true,
                detailRow: false,
                firstDetailInTurn: false,
                detailCountInTurn: 0,
                standaloneActivity: false,
                text: "Copy this message",
                markupDocument: messageDocument
            }, overrides ?? {});
        }

        function createViewport(rows) {
            destroyViewport();
            fakeTimelineModel.resetRows(rows);
            suite.viewport = createTemporaryObject(viewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.activeRowSlotCount > 0);
            tryVerify(() => suite.viewport.delegateForEntry(rows[0].entryId) !== null);
            const row = suite.viewport.delegateForEntry(rows[0].entryId);
            tryVerify(() => row.implicitHeight > 0);
            verify(waitForRendering(suite.viewport));
            return row;
        }

        function destroyViewport() {
            if (!suite.viewport)
                return;
            suite.viewport.destroy();
            suite.viewport = null;
            wait(0);
        }

        function init() {
            ApplicationClipboard.reset();
            suite.hasRunningEvidence = false;
            suite.activityShimmerEnabled = false;
            suite.forkEnabled = true;
            suite.showForkActions = true;
            suite.lastForkedTurnId = "";
        }

        function cleanup() {
            destroyViewport();
            fakeTimelineModel.resetRows([]);
        }

        function test_hoverRevealsOlderAnswerWithoutChangingRowHeight() {
            const row = createViewport([messageRow({
                    latestTurn: false
                })]);
            const actions = findChild(row, "codexMessageActions");
            verify(actions !== null);
            verify(actions.available);
            tryVerify(() => row.implicitHeight > 24);
            tryCompare(actions, "opacity", 0);
            const retainedHeight = row.implicitHeight;

            mouseMove(row, 10, 8);

            tryCompare(actions, "opacity", 1);
            compare(row.implicitHeight, retainedHeight);
        }

        function test_latestRunningEvidenceSuppressesActionsButNotOlderMessages() {
            suite.hasRunningEvidence = true;
            let row = createViewport([messageRow()]);
            let actions = findChild(row, "codexMessageActions");
            verify(actions !== null);
            verify(!actions.available);
            verify(!actions.visible);

            row = createViewport([messageRow({
                    entryId: "message:turn-0:message-1",
                    turnId: "turn-0",
                    latestTurn: false
                })]);
            actions = findChild(row, "codexMessageActions");
            verify(actions !== null);
            verify(actions.available);
        }

        function test_userCopyUsesClipboardGlueAndKeepsGeometryStable() {
            const row = createViewport([messageRow({
                    fromUser: true,
                    finalAnswer: false
                })]);
            const copyButton = findChild(row, "codexMessageCopyButton");
            const actions = findChild(row, "codexMessageActions");
            verify(copyButton !== null);
            verify(actions !== null);
            const retainedHeight = row.implicitHeight;

            mouseClick(copyButton);

            compare(ApplicationClipboard.copyCount, 1);
            compare(ApplicationClipboard.lastCopiedText, "Copy this message");
            verify(actions.copied);
            compare(row.implicitHeight, retainedHeight);
            verify(findChild(row, "codexMessageForkButton") === null || !findChild(row, "codexMessageForkButton").visible);
        }

        function test_forkRequiresAFinalForkableAnswerAndEmitsItsTurnId() {
            const row = createViewport([messageRow()]);
            const forkButton = findChild(row, "codexMessageForkButton");
            verify(forkButton !== null);
            verify(forkButton.visible);

            mouseClick(forkButton);

            compare(suite.lastForkedTurnId, "turn-1");
        }

        function test_forkHonorsEnablementVisibilityAndModelConstraints() {
            suite.forkEnabled = false;
            let row = createViewport([messageRow()]);
            let forkButton = findChild(row, "codexMessageForkButton");
            verify(forkButton !== null);
            verify(forkButton.visible);
            verify(!forkButton.enabled);
            mouseClick(forkButton);
            compare(suite.lastForkedTurnId, "");

            suite.forkEnabled = true;
            suite.showForkActions = false;
            row = createViewport([messageRow()]);
            forkButton = findChild(row, "codexMessageForkButton");
            verify(forkButton !== null);
            verify(!forkButton.visible);

            suite.showForkActions = true;
            row = createViewport([messageRow({
                    turnForkable: false
                })]);
            forkButton = findChild(row, "codexMessageForkButton");
            verify(forkButton !== null);
            verify(!forkButton.visible);
        }

        function test_runningParentActivityShimmerDoesNotChangeGeometry() {
            const row = createViewport([Fixtures.standaloneActivityRow()]);
            const shimmer = findChild(row, "codexActivityShimmer");
            verify(shimmer !== null);
            verify(!shimmer.visible);
            const retainedHeight = row.implicitHeight;

            suite.activityShimmerEnabled = true;

            tryVerify(() => shimmer.visible);
            compare(row.implicitHeight, retainedHeight);

            suite.activityShimmerEnabled = false;

            tryVerify(() => !shimmer.visible);
            compare(row.implicitHeight, retainedHeight);
        }
    }
}
