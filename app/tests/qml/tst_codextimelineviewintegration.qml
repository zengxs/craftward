// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import Craftward.Codex
import "../../qml/Craftward/Pages" as Pages
import "CodexTimelineTestFixtures.js" as Fixtures

Item {
    id: suite

    width: 720
    height: 480
    property var timelineView

    ListModel {
        id: messageSegments

        ListElement {
            segmentId: "answer"
            codeBlock: false
            segmentText: "Answer"
            language: ""
            markdown: true
        }
    }

    QtObject {
        id: messageDocument

        property var renderModel: messageSegments
    }

    QtObject {
        id: timelineData

        property int revision: 0
        property var rows: []

        function valueAt(sourceRow, roleName) {
            const row = rows[sourceRow];
            return row ? row[roleName] : undefined;
        }
    }

    CodexConversationController {
        id: fakeController

        timeline: timelineData
        threadId: "thread-1"
    }

    Component {
        id: timelineViewComponent

        Pages.CodexTimelineView {
            width: 680
            height: 360
            controller: fakeController
            forkEnabled: true
            showForkActions: true
            bottomContentInset: 0
        }
    }

    TestCase {
        name: "CodexTimelineViewIntegration"
        when: windowShown

        function answerRow() {
            return {
                entryId: "message:turn-1:answer-1",
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
                text: "Answer",
                markupDocument: messageDocument
            };
        }

        function init() {
            fakeController.hasRunningEvidence = false;
            fakeController.waitingOnApproval = false;
            fakeController.waitingOnUserInput = false;
            timelineData.rows = [answerRow()];
            ++timelineData.revision;
            suite.timelineView = createTemporaryObject(timelineViewComponent, suite);
            verify(suite.timelineView !== null);
            tryVerify(() => findChild(suite.timelineView, "codexMessageActions") !== null);
        }

        function cleanup() {
            suite.timelineView = null;
            timelineData.rows = [];
            ++timelineData.revision;
        }

        function test_mapsControllerRunningEvidenceIntoTheLatestTimelineRow() {
            const actions = findChild(suite.timelineView, "codexMessageActions");
            verify(actions !== null);
            verify(actions.available);

            fakeController.hasRunningEvidence = true;

            tryVerify(() => !actions.available);
            verify(!actions.visible);

            fakeController.hasRunningEvidence = false;

            tryVerify(() => actions.available);
            verify(actions.visible);
        }

        function test_shimmerRequiresLiveEvidenceAndPausesForInteraction() {
            suite.timelineView.destroy();
            timelineData.rows = [Fixtures.standaloneActivityRow()];
            ++timelineData.revision;
            fakeController.hasRunningEvidence = true;
            suite.timelineView = createTemporaryObject(timelineViewComponent, suite);
            verify(suite.timelineView !== null);
            tryVerify(() => findChild(suite.timelineView, "codexActivityShimmer") !== null);
            const shimmer = findChild(suite.timelineView, "codexActivityShimmer");
            verify(shimmer !== null);
            tryVerify(() => shimmer.visible);

            fakeController.waitingOnApproval = true;
            tryVerify(() => !shimmer.visible);

            fakeController.waitingOnApproval = false;
            tryVerify(() => shimmer.visible);

            fakeController.waitingOnUserInput = true;
            tryVerify(() => !shimmer.visible);

            fakeController.waitingOnUserInput = false;
            fakeController.hasRunningEvidence = false;
            tryVerify(() => !shimmer.visible);
        }
    }
}
