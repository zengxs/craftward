// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import Craftward.Codex

Control {
    id: root

    required property CodexConversationController controller
    required property bool forkEnabled
    required property bool showForkActions
    property real bottomContentInset: 64
    property double wallClockUnixMilliseconds: Date.now()
    property bool motionDiagnosticsEnabled: false
    property bool timelineRenderBenchmarkEnabled: false
    property string timelineRenderBenchmarkThreadId: ""
    property string timelineRenderBenchmarkRenderer: "current"
    readonly property bool semanticRendererSelected: root.timelineRenderBenchmarkRenderer === "semantic"
    readonly property var activeTimelineModel: root.semanticRendererSelected ? semanticViewportModel : presentationModel
    readonly property real contentColumnWidth: timelineViewport.contentColumnWidth
    readonly property bool activityShimmerEnabled: root.controller !== null && root.controller.hasRunningEvidence && !root.controller.waitingOnApproval && !root.controller.waitingOnUserInput
    readonly property string motionDiagnosticsText: motionDiagnostics.statisticsText

    signal forkRequested(string turnId)

    function toggleTurn(turnId) {
        const anchor = timelineViewport.captureVisibleAnchor();
        presentationModel.toggleTurn(String(turnId));
        timelineViewport.followLiveTail = false;
        timelineViewport.scheduleAnchorRestore(anchor);
    }

    function followLatest() {
        timelineViewport.followLatest();
    }

    padding: 0

    CodexTimelinePresentationModel {
        id: presentationModel

        sourceModel: root.controller ? root.controller.timeline : null
    }

    CodexTimelineViewportModel {
        id: semanticViewportModel

        sourceModel: root.semanticRendererSelected ? presentationModel : null
    }

    TimelineMotionDiagnostics {
        id: motionDiagnostics

        targetViewport: timelineViewport
        active: root.motionDiagnosticsEnabled && root.visible
    }

    TimelineRenderBenchmark {
        id: renderBenchmark

        targetViewport: timelineViewport
        targetWindow: root.Window.window
        active: root.timelineRenderBenchmarkEnabled && root.visible
        rendererName: root.timelineRenderBenchmarkRenderer
        requestedThreadId: root.timelineRenderBenchmarkThreadId
        selectedThreadId: root.controller ? root.controller.threadId : ""
        conversationLoading: root.controller ? root.controller.loading : false
        rowCount: root.activeTimelineModel ? root.activeTimelineModel.totalRowCount : 0
        frameBudgetMilliseconds: timelineViewport.frameBudgetMilliseconds
        onFinished: benchmarkResult => {
            console.log("TIMELINE_RENDER_BENCHMARK " + JSON.stringify(benchmarkResult));
            Qt.exit(benchmarkResult.passed ? 0 : 2);
        }
    }

    contentItem: Item {
        CodexTimelineViewport {
            id: timelineViewport

            anchors.fill: parent
            timelineModel: root.activeTimelineModel
            rowDelegate: timelineRowComponent
            bottomContentInset: root.bottomContentInset
            rowSpacing: root.semanticRendererSelected ? 0 : 10
            heightCacheNamespace: root.semanticRendererSelected ? "semantic" : "current"
        }

        Label {
            anchors.centerIn: parent
            width: Math.min(parent.width - 48, 360)
            text: {
                if (!root.controller)
                    return "";
                return root.controller.loading ? /*% "Loading conversation…" */ qsTrId("craftward.codex.timeline.loading") : (root.controller.threadId ? /*% "This conversation contains no displayable history." */ qsTrId("craftward.codex.timeline.empty") : /*% "Select a conversation to read it." */ qsTrId("craftward.codex.timeline.no_selection"));
            }
            color: root.palette.placeholderText
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            visible: !root.activeTimelineModel || root.activeTimelineModel.totalRowCount === 0
        }
    }

    Component {
        id: timelineRowComponent

        CodexTimelineRow {
            width: parent ? parent.width : 0
            timelineModel: root.activeTimelineModel
            rendererName: root.semanticRendererSelected ? "semantic" : "current"
            turnExpanded: {
                const currentRevision = dataRevision;
                return currentRevision >= 0 && root.activeTimelineModel ? Boolean(root.activeTimelineModel.valueAt(sourceRow, "turnExpanded")) : false;
            }
            hasRunningEvidence: root.controller !== null && root.controller.hasRunningEvidence
            activityShimmerEnabled: root.activityShimmerEnabled
            forkEnabled: root.forkEnabled
            showForkActions: root.showForkActions
            wallClockUnixMilliseconds: root.wallClockUnixMilliseconds
            font: root.font
            onToggleTurnRequested: turnId => root.toggleTurn(turnId)
            onForkRequested: turnId => root.forkRequested(turnId)
        }
    }

    Connections {
        target: root.controller

        function onSelectionChanged() {
            presentationModel.clearExpandedTurns();
            timelineViewport.resetForNewContent();
        }

        function onTurnStarted() {
            timelineViewport.followLatest();
        }
    }

    Timer {
        interval: 1000
        running: root.visible && root.controller !== null && root.controller.turnRunning
        repeat: true
        onTriggered: root.wallClockUnixMilliseconds = Date.now()
    }
}
