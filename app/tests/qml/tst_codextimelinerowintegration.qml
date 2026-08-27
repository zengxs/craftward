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

    QtObject {
        id: fakePageModel

        property int revision: 0
        property var rows: []
        readonly property int pageCount: rows.length > 0 ? 1 : 0

        function pageId(page) {
            return page === 0 ? "page:turn-1" : "";
        }

        function pageFirstRow(page) {
            return page === 0 ? 0 : -1;
        }

        function pageRowCount(page) {
            return page === 0 ? rows.length : 0;
        }

        function valueAt(sourceRow, roleName) {
            const row = rows[sourceRow];
            return row ? row[roleName] : undefined;
        }
    }

    Component {
        id: rowComponent

        Pages.CodexTimelineRow {
            timelineModel: fakePageModel
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
            pageModel: fakePageModel
            rowDelegate: rowComponent
            bottomContentInset: 0
            contentHorizontalInset: 20
            contentMaximumWidth: 640
            estimatedPageHeight: 120
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
            fakePageModel.rows = rows;
            ++fakePageModel.revision;
            suite.viewport = createTemporaryObject(viewportComponent, suite);
            verify(suite.viewport !== null);
            tryVerify(() => suite.viewport.loadedPageCount === 1);
            tryVerify(() => suite.viewport.delegateForEntry(rows[0].entryId) !== null);
            const row = suite.viewport.delegateForEntry(rows[0].entryId);
            tryVerify(() => row.implicitHeight > 0);
            wait(0);
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
            fakePageModel.rows = [];
            ++fakePageModel.revision;
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
