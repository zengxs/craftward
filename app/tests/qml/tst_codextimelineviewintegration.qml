// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtTest
import Craftward.Codex
import Craftward.Components
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
            segmentText: "[Documentation](https://example.com)"
            language: ""
            markdown: true
        }

        ListElement {
            segmentId: "example"
            codeBlock: true
            segmentText: "example()"
            language: "cpp"
            markdown: false
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

        function detailRow() {
            return {
                entryId: "message:turn-1:detail-1",
                turnId: "turn-1",
                turnForkable: false,
                latestTurn: true,
                activityGroup: false,
                fromUser: false,
                finalAnswer: false,
                detailRow: true,
                firstDetailInTurn: true,
                detailCountInTurn: 1,
                standaloneActivity: false,
                text: "Detail",
                markupDocument: messageDocument
            };
        }

        function init() {
            ApplicationClipboard.reset();
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

        function test_semanticRendererMaterializesBlockRowsOnlyWhileSelected() {
            const scrollViewport = findChild(suite.timelineView, "codexTimelineScrollViewport");
            verify(scrollViewport !== null);
            compare(scrollViewport.count, 1);

            suite.timelineView.timelineRenderBenchmarkRenderer = "semantic";

            tryCompare(scrollViewport, "count", 2);
            const semanticModel = suite.timelineView.activeTimelineModel;

            suite.timelineView.timelineRenderBenchmarkRenderer = "current";

            tryCompare(scrollViewport, "count", 1);
            compare(semanticModel.totalRowCount, 0);

            suite.timelineView.timelineRenderBenchmarkRenderer = "semantic";

            tryCompare(scrollViewport, "count", 2);
        }

        function test_semanticRendererRendersOnlyTheSelectedBlock() {
            suite.timelineView.timelineRenderBenchmarkRenderer = "semantic";
            const scrollViewport = findChild(suite.timelineView, "codexTimelineScrollViewport");
            verify(scrollViewport !== null);
            tryCompare(scrollViewport, "count", 2);
            const viewport = scrollViewport.parent;
            verify(viewport !== null);
            tryVerify(() => viewport.delegateForEntry("message:turn-1:answer-1") !== null);
            tryVerify(() => viewport.delegateForEntry("message:turn-1:answer-1/markup/example") !== null);

            const proseRow = viewport.delegateForEntry("message:turn-1:answer-1");
            const codeRow = viewport.delegateForEntry("message:turn-1:answer-1/markup/example");
            verify(findChild(proseRow, "markupCodeSurface") === null);
            const codeText = findChild(codeRow, "markupCodeText");
            verify(codeText !== null);
            compare(codeText.text, "example()");
        }

        function test_semanticRendererPreservesBlockSelectionLinksAndCopy() {
            suite.timelineView.timelineRenderBenchmarkRenderer = "semantic";
            const scrollViewport = findChild(suite.timelineView, "codexTimelineScrollViewport");
            verify(scrollViewport !== null);
            tryCompare(scrollViewport, "count", 2);
            const viewport = scrollViewport.parent;
            const firstEntryId = "message:turn-1:answer-1";
            const codeEntryId = firstEntryId + "/markup/example";
            tryVerify(() => viewport.delegateForEntry(firstEntryId) !== null);
            tryVerify(() => viewport.delegateForEntry(codeEntryId) !== null);

            const proseText = findChild(viewport.delegateForEntry(firstEntryId), "markupProseText");
            verify(proseText !== null);
            const linkRectangle = proseText.positionToRectangle(1);
            compare(proseText.linkAt(linkRectangle.x + 1, linkRectangle.y + linkRectangle.height / 2), "https://example.com");
            proseText.selectAll();
            verify(proseText.selectedText.includes("Documentation"));

            const codeRow = viewport.delegateForEntry(codeEntryId);
            const codeText = findChild(codeRow, "markupCodeText");
            const copyButton = findChild(codeRow, "markupCodeCopyButton");
            verify(codeText !== null);
            verify(copyButton !== null);
            codeText.selectAll();
            compare(codeText.selectedText, "example()");
            codeText.forceActiveFocus();
            tryVerify(() => copyButton.visible);
            copyButton.forceActiveFocus();
            tryVerify(() => copyButton.activeFocus);
            keyClick(Qt.Key_Space);
            compare(ApplicationClipboard.lastCopiedText, "example()");
        }

        function test_semanticRendererKeepsOneContinuousUserMessageSurface() {
            suite.timelineView.destroy();
            timelineData.rows = [answerRow()];
            timelineData.rows[0].fromUser = true;
            timelineData.rows[0].finalAnswer = false;
            ++timelineData.revision;
            suite.timelineView = createTemporaryObject(timelineViewComponent, suite, {
                timelineRenderBenchmarkRenderer: "semantic"
            });
            verify(suite.timelineView !== null);
            const scrollViewport = findChild(suite.timelineView, "codexTimelineScrollViewport");
            verify(scrollViewport !== null);
            tryCompare(scrollViewport, "count", 2);
            compare(scrollViewport.spacing, 0);
            const viewport = scrollViewport.parent;
            const firstEntryId = "message:turn-1:answer-1";
            const lastEntryId = firstEntryId + "/markup/example";
            tryVerify(() => viewport.delegateForEntry(firstEntryId) !== null);
            tryVerify(() => viewport.delegateForEntry(lastEntryId) !== null);
            const firstRow = viewport.delegateForEntry(firstEntryId);
            const lastRow = viewport.delegateForEntry(lastEntryId);
            const firstSurface = findChild(firstRow, "codexUserMessageSurface");
            const lastSurface = findChild(lastRow, "codexUserMessageSurface");
            const firstRenderer = findChild(firstRow, "codexMessageRenderer");
            const lastRenderer = findChild(lastRow, "codexMessageRenderer");
            verify(firstSurface !== null);
            verify(lastSurface !== null);
            verify(firstRenderer !== null);
            verify(lastRenderer !== null);
            const firstBottom = firstSurface.mapToItem(suite.timelineView, 0, firstSurface.height).y;
            const lastTop = lastSurface.mapToItem(suite.timelineView, 0, 0).y;
            verify(Math.abs(firstBottom - lastTop) <= 0.5);
            verify(firstRenderer.y > 0);
            compare(lastRenderer.y, 0);
        }

        function test_rendererAdaptersKeepIndependentHeightCaches() {
            const entryId = "message:turn-1:answer-1";
            const scrollViewport = findChild(suite.timelineView, "codexTimelineScrollViewport");
            verify(scrollViewport !== null);
            const viewport = scrollViewport.parent;
            compare(viewport.heightCacheNamespace, "current");
            tryVerify(() => viewport.delegateForEntry(entryId) !== null);
            compare(viewport.delegateForEntry(entryId).heightCacheKey, "current:" + entryId);

            suite.timelineView.timelineRenderBenchmarkRenderer = "semantic";

            tryCompare(scrollViewport, "count", 2);
            compare(viewport.heightCacheNamespace, "semantic");
            tryVerify(() => viewport.delegateForEntry(entryId) !== null);
            compare(viewport.delegateForEntry(entryId).heightCacheKey, "semantic:" + entryId);
        }

        function test_rendererAdaptersExposeTheirMaterializationContract() {
            const entryId = "message:turn-1:answer-1";
            const scrollViewport = findChild(suite.timelineView, "codexTimelineScrollViewport");
            verify(scrollViewport !== null);
            const viewport = scrollViewport.parent;
            tryVerify(() => viewport.delegateForEntry(entryId) !== null);

            const currentRow = viewport.delegateForEntry(entryId);
            compare(currentRow.contentMaterializationRequested, false);
            compare(currentRow.contentMaterializationReady, true);
            compare(currentRow.contentMeasurementReady, true);

            suite.timelineView.timelineRenderBenchmarkRenderer = "semantic";

            tryCompare(scrollViewport, "count", 2);
            tryVerify(() => viewport.delegateForEntry(entryId) !== null);
            const semanticRow = viewport.delegateForEntry(entryId);
            compare(semanticRow.contentMaterializationRequested, false);
            compare(semanticRow.contentMaterializationReady, true);
            compare(semanticRow.contentMeasurementReady, true);
        }

        function test_semanticRendererKeepsMessageActionsOnTheLastBlock() {
            suite.timelineView.timelineRenderBenchmarkRenderer = "semantic";
            const scrollViewport = findChild(suite.timelineView, "codexTimelineScrollViewport");
            verify(scrollViewport !== null);
            tryCompare(scrollViewport, "count", 2);
            const viewport = scrollViewport.parent;
            const firstEntryId = "message:turn-1:answer-1";
            const lastEntryId = firstEntryId + "/markup/example";
            tryVerify(() => viewport.delegateForEntry(firstEntryId) !== null);
            tryVerify(() => viewport.delegateForEntry(lastEntryId) !== null);
            const firstActions = findChild(viewport.delegateForEntry(firstEntryId), "codexMessageActions");
            const lastActions = findChild(viewport.delegateForEntry(lastEntryId), "codexMessageActions");
            verify(firstActions !== null);
            verify(lastActions !== null);
            verify(!firstActions.available);
            verify(lastActions.available);
        }

        function test_semanticDetailExpansionKeepsOneHeaderAndOneBlockPerRow() {
            suite.timelineView.destroy();
            timelineData.rows = [detailRow()];
            ++timelineData.revision;
            suite.timelineView = createTemporaryObject(timelineViewComponent, suite, {
                timelineRenderBenchmarkRenderer: "semantic"
            });
            verify(suite.timelineView !== null);
            const scrollViewport = findChild(suite.timelineView, "codexTimelineScrollViewport");
            verify(scrollViewport !== null);
            tryCompare(scrollViewport, "count", 1);

            suite.timelineView.toggleTurn("turn-1");

            tryCompare(scrollViewport, "count", 2);
            const viewport = scrollViewport.parent;
            const firstEntryId = "message:turn-1:detail-1";
            const secondEntryId = firstEntryId + "/markup/example";
            tryVerify(() => viewport.delegateForEntry(firstEntryId) !== null);
            tryVerify(() => viewport.delegateForEntry(secondEntryId) !== null);
            const proseRow = viewport.delegateForEntry(firstEntryId);
            const codeRow = viewport.delegateForEntry(secondEntryId);
            verify(findChild(proseRow, "codexTimelineDisclosureRow") !== null);
            verify(findChild(codeRow, "codexTimelineDisclosureRow") === null);
            verify(findChild(proseRow, "markupCodeSurface") === null);
            tryVerify(() => findChild(codeRow, "markupCodeText") !== null);
            const codeText = findChild(codeRow, "markupCodeText");
            compare(codeText.text, "example()");
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
