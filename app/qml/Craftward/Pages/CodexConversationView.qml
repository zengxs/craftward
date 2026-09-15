// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Codex
import Craftward.Components
import Craftward.Design

Control {
    id: root

    required property CodexHistoryController controller
    required property CodexHistoryActionState actionState
    readonly property CodexConversationController conversation: controller.conversation
    property bool timelineMotionDiagnosticsEnabled: false
    property bool timelineRenderBenchmarkEnabled: false
    property string timelineRenderBenchmarkThreadId: ""
    readonly property string timelineMotionDiagnosticsText: timelineView.motionDiagnosticsText
    signal fileLocationRequested(string file, int start, int end)

    ColumnLayout {
        anchors {
            fill: parent
            topMargin: 12
            leftMargin: 20
            rightMargin: 20
        }
        spacing: 12

        ListView {
            id: interactionList

            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(contentHeight, 360)
            Layout.maximumHeight: 360
            clip: true
            spacing: 8
            model: root.conversation.interactions
            enabled: !root.controller.startingThread
            visible: count > 0 && !root.controller.showingArchived
            ScrollBar.vertical: OverlayScrollBar {}

            delegate: CodexInteractionCard {
                id: interactionCard

                width: ListView.view.width
                onApprovalSubmitted: decision => root.conversation.respondToApproval(interactionId, decision)
                onUserInputSubmitted: answers => root.conversation.respondToUserInput(interactionId, answers)
            }
        }

        Rectangle {
            Layout.fillWidth: true
            implicitHeight: errorLayout.implicitHeight + 20
            radius: 9
            color: Theme.dangerSurface
            border.color: Theme.dangerBorder
            visible: root.controller.errorMessage.length > 0

            RowLayout {
                id: errorLayout

                anchors {
                    fill: parent
                    margins: 10
                }

                Label {
                    Layout.fillWidth: true
                    text: root.controller.errorMessage
                    color: Theme.dangerForeground
                    wrapMode: Text.WordWrap
                }

                IconButton {
                    icon.source: "qrc:///icons/hugeicons/cancel-01.svg"
                    toolTipText: /*% "Dismiss error" */ qsTrId("craftward.error.dismiss")
                    onClicked: root.controller.clearError()
                }
            }
        }

        Item {
            id: conversationSurface

            Layout.fillWidth: true
            Layout.fillHeight: true
            readonly property real composerBottomMargin: 14
            readonly property real composerContentGap: 24

            CodexTimelineView {
                id: timelineView

                anchors.fill: parent
                bottomContentInset: composer.visible ? composer.height + conversationSurface.composerBottomMargin + conversationSurface.composerContentGap : 64
                controller: root.conversation
                forkEnabled: root.actionState.canFork
                showForkActions: root.conversation.threadId.length > 0 && !root.controller.showingArchived
                motionDiagnosticsEnabled: root.timelineMotionDiagnosticsEnabled
                timelineRenderBenchmarkEnabled: root.timelineRenderBenchmarkEnabled
                timelineRenderBenchmarkThreadId: root.timelineRenderBenchmarkThreadId
                onForkRequested: turnId => root.controller.forkSelectedThread(turnId)
                onFileLocationRequested: (file, start, end) => root.fileLocationRequested(file, start, end)
            }

            CodexComposer {
                id: composer

                anchors {
                    horizontalCenter: parent.horizontalCenter
                    bottom: parent.bottom
                    bottomMargin: conversationSurface.composerBottomMargin
                }
                width: timelineView.contentColumnWidth
                z: 1
                controller: root.conversation
                readOnly: root.controller.showingArchived
                startingThread: root.controller.startingThread
                enabled: !root.controller.startingThread && !root.controller.forkingThread
                visible: root.actionState.composerVisible
                onTurnSubmitted: timelineView.followLatest()
            }
        }
    }
}
