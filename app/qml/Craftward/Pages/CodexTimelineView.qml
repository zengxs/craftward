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

    TimelineMotionDiagnostics {
        id: motionDiagnostics

        targetViewport: timelineViewport
        active: root.motionDiagnosticsEnabled && root.visible
    }

    contentItem: Item {
        CodexTimelineViewport {
            id: timelineViewport

            anchors.fill: parent
            timelineModel: presentationModel
            rowDelegate: timelineRowComponent
            bottomContentInset: root.bottomContentInset
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
            visible: presentationModel.totalRowCount === 0
        }
    }

    Component {
        id: timelineRowComponent

        CodexTimelineRow {
            width: parent ? parent.width : 0
            timelineModel: presentationModel
            turnExpanded: {
                const currentRevision = dataRevision;
                return currentRevision >= 0 ? Boolean(presentationModel.valueAt(sourceRow, "turnExpanded")) : false;
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
