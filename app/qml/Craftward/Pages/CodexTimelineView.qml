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
    property var expandedTurns: ({})
    property double wallClockUnixMilliseconds: Date.now()
    readonly property real contentColumnWidth: timelineViewport.contentColumnWidth
    readonly property bool activityShimmerEnabled: root.controller !== null && root.controller.hasRunningEvidence && !root.controller.waitingOnApproval && !root.controller.waitingOnUserInput

    signal forkRequested(string turnId)

    function turnExpanded(turnId) {
        return root.expandedTurns[String(turnId)] === true;
    }

    function toggleTurn(turnId) {
        const anchor = timelineViewport.captureVisibleAnchor();
        const next = Object.assign({}, root.expandedTurns);
        const key = String(turnId);
        next[key] = !root.turnExpanded(key);
        root.expandedTurns = next;
        timelineViewport.followLiveTail = false;
        timelineViewport.scheduleAnchorRestore(anchor);
    }

    function followLatest() {
        timelineViewport.followLatest();
    }

    padding: 0

    CodexTimelinePageModel {
        id: pageModel

        sourceModel: root.controller ? root.controller.timeline : null
        turnsPerPage: 8
    }

    contentItem: Item {
        CodexTimelineViewport {
            id: timelineViewport

            anchors.fill: parent
            pageModel: pageModel
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
            visible: pageModel.totalRowCount === 0
        }
    }

    Component {
        id: timelineRowComponent

        CodexTimelineRow {
            width: parent ? parent.width : 0
            timelineModel: pageModel
            turnExpanded: root.turnExpanded(turnId)
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
            root.expandedTurns = {};
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
