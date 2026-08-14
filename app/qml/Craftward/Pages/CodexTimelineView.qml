// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Codex
import Craftward.Components

Control {
    id: root

    required property CodexHistoryController controller
    property double wallClockUnixMilliseconds: Date.now()

    function activityStatusText(activity) {
        if (!activity.reasoning)
            return activity.statusLabel;

        const startedAt = Number(activity.startedAtUnixMilliseconds);
        if (startedAt <= 0)
            return activity.statusLabel;

        const completedAt = Number(activity.completedAtUnixMilliseconds);
        const endAt = completedAt > 0 ? completedAt : root.wallClockUnixMilliseconds;
        const elapsedSeconds = Math.max(0, Math.floor((endAt - startedAt) / 1000));
        return qsTr("Processed %1 s").arg(elapsedSeconds);
    }

    function followLatest() {
        timelineList.followLiveTail = true;
        Qt.callLater(timelineList.positionViewAtEnd);
    }

    padding: 0

    contentItem: ListView {
        id: timelineList

        readonly property bool conversationLoading: root.controller.loadingConversation
        readonly property string selectedThreadId: root.controller.selectedThreadId
        property bool initialPositionActive: false
        property bool initialPositionScheduled: false
        property string pendingInitialPositionThreadId
        property bool followLiveTail: true

        function cancelInitialPosition() {
            initialPositionActive = false;
            initialPositionScheduled = false;
            pendingInitialPositionThreadId = "";
            initialPositionStabilityTimer.stop();
        }

        function applyInitialPosition() {
            initialPositionScheduled = false;
            if (conversationLoading || pendingInitialPositionThreadId !== selectedThreadId)
                return;
            if (count === 0) {
                cancelInitialPosition();
                return;
            }

            initialPositionActive = true;
            forceLayout();
            positionViewAtEnd();
            initialPositionStabilityTimer.restart();
        }

        function scheduleInitialPosition() {
            if (conversationLoading || !pendingInitialPositionThreadId || pendingInitialPositionThreadId !== selectedThreadId || initialPositionScheduled)
                return;
            initialPositionScheduled = true;
            Qt.callLater(timelineList.applyInitialPosition);
        }

        function finishInitialPosition() {
            if (!initialPositionActive || conversationLoading || pendingInitialPositionThreadId !== selectedThreadId)
                return;

            forceLayout();
            positionViewAtEnd();
            Qt.callLater(timelineList.completeInitialPosition);
        }

        function completeInitialPosition() {
            if (!initialPositionActive || conversationLoading || pendingInitialPositionThreadId !== selectedThreadId)
                return;
            if (initialPositionStabilityTimer.running)
                return;

            forceLayout();
            positionViewAtEnd();
            if (initialPositionStabilityTimer.running)
                return;

            initialPositionActive = false;
            pendingInitialPositionThreadId = "";
        }

        clip: true
        spacing: 10
        model: root.controller.timeline
        // Temporary mitigation for ListView's variable-height content estimate.
        // Replace this with cached row heights and anchored scrolling.
        cacheBuffer: Math.max(height * 4, 2048)
        ScrollBar.vertical: OverlayScrollBar {}

        onContentHeightChanged: {
            if (initialPositionActive) {
                initialPositionStabilityTimer.restart();
                scheduleInitialPosition();
            } else if (followLiveTail) {
                Qt.callLater(timelineList.positionViewAtEnd);
            }
        }
        onConversationLoadingChanged: scheduleInitialPosition()
        onDraggingChanged: {
            if (dragging) {
                cancelInitialPosition();
                followLiveTail = false;
            }
        }
        onMovementEnded: followLiveTail = atYEnd
        onSelectedThreadIdChanged: {
            cancelInitialPosition();
            followLiveTail = true;
            pendingInitialPositionThreadId = selectedThreadId;
            scheduleInitialPosition();
        }

        Timer {
            id: initialPositionStabilityTimer

            interval: 100
            onTriggered: timelineList.finishInitialPosition()
        }

        delegate: Item {
            id: timelineDelegate

            required property string entryId
            required property string turnId
            required property bool activityGroup
            required property bool fromUser
            required property bool commentary
            required property bool finalAnswer
            required property string text
            required property string activityLabel
            required property int activityCount
            required property var activityItems
            required property bool failed
            required property bool running
            property bool groupExpanded: activityItems.length > 0 && activityItems[0].reasoning

            width: ListView.view.width
            implicitHeight: activityGroup ? activityCard.implicitHeight : messageCard.implicitHeight

            Rectangle {
                id: messageCard

                anchors.right: timelineDelegate.fromUser ? parent.right : undefined
                anchors.left: timelineDelegate.fromUser ? undefined : parent.left
                width: Math.min(implicitWidth, parent.width * (timelineDelegate.commentary ? 0.92 : 0.86))
                implicitWidth: Math.max(220, messageContent.implicitWidth + 28)
                implicitHeight: visible ? messageContent.implicitHeight + 24 : 0
                radius: 12
                color: timelineDelegate.commentary ? "transparent" : (timelineDelegate.fromUser ? root.palette.alternateBase : root.palette.base)
                border.color: timelineDelegate.commentary ? "transparent" : root.palette.mid
                visible: !timelineDelegate.activityGroup

                ColumnLayout {
                    id: messageContent

                    anchors {
                        fill: parent
                        margins: 12
                    }
                    spacing: 6

                    Label {
                        text: timelineDelegate.fromUser ? qsTr("You") : (timelineDelegate.commentary ? qsTr("Codex · Commentary") : qsTr("Codex"))
                        color: root.palette.placeholderText
                        font.pixelSize: 11
                        font.weight: Font.DemiBold
                    }

                    TextEdit {
                        Layout.fillWidth: true
                        text: timelineDelegate.text
                        color: root.palette.text
                        font: root.font
                        readOnly: true
                        selectByMouse: true
                        wrapMode: TextEdit.Wrap
                        textFormat: TextEdit.MarkdownText
                    }
                }
            }

            Item {
                id: activityCard

                anchors.left: parent.left
                width: Math.min(parent.width * 0.92, 820)
                implicitHeight: visible ? activityColumn.implicitHeight : 0
                visible: timelineDelegate.activityGroup

                ColumnLayout {
                    id: activityColumn

                    width: parent.width
                    spacing: 2

                    ItemDelegate {
                        Layout.fillWidth: true
                        leftPadding: 6
                        rightPadding: 8
                        topPadding: 5
                        bottomPadding: 5
                        hoverEnabled: true
                        onClicked: timelineDelegate.groupExpanded = !timelineDelegate.groupExpanded

                        contentItem: RowLayout {
                            spacing: 8

                            Label {
                                text: timelineDelegate.groupExpanded ? "▾" : "›"
                                color: root.palette.placeholderText
                                font.pixelSize: 13
                            }

                            Rectangle {
                                Layout.preferredWidth: 8
                                Layout.preferredHeight: 8
                                radius: width / 2
                                color: timelineDelegate.failed ? "#B4232A" : (timelineDelegate.running ? root.palette.highlight : root.palette.mid)
                            }

                            Label {
                                Layout.fillWidth: true
                                text: timelineDelegate.activityLabel
                                color: root.palette.placeholderText
                                font.weight: Font.DemiBold
                            }

                            Label {
                                text: qsTr("× %1").arg(timelineDelegate.activityCount)
                                color: root.palette.placeholderText
                                font.pixelSize: 11
                                visible: timelineDelegate.activityCount > 1
                            }
                        }
                    }

                    Repeater {
                        model: timelineDelegate.groupExpanded ? timelineDelegate.activityItems : []

                        delegate: ItemDelegate {
                            id: activityItemDelegate

                            required property var modelData
                            property bool detailsExpanded: modelData.reasoning

                            Layout.fillWidth: true
                            leftPadding: 28
                            rightPadding: 8
                            topPadding: 6
                            bottomPadding: 6
                            hoverEnabled: modelData.expandable
                            onClicked: {
                                if (modelData.expandable)
                                    detailsExpanded = !detailsExpanded;
                            }

                            contentItem: ColumnLayout {
                                spacing: 4

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 7

                                    Rectangle {
                                        Layout.preferredWidth: 7
                                        Layout.preferredHeight: 7
                                        radius: width / 2
                                        color: activityItemDelegate.modelData.failed ? "#B4232A" : (activityItemDelegate.modelData.running ? root.palette.highlight : root.palette.mid)
                                    }

                                    Label {
                                        Layout.fillWidth: true
                                        text: activityItemDelegate.modelData.summary
                                        textFormat: Text.PlainText
                                        color: root.palette.text
                                        wrapMode: Text.Wrap
                                        maximumLineCount: activityItemDelegate.detailsExpanded ? 1000 : 2
                                        elide: Text.ElideRight
                                    }

                                    Label {
                                        text: root.activityStatusText(activityItemDelegate.modelData)
                                        color: activityItemDelegate.modelData.failed ? "#B4232A" : root.palette.placeholderText
                                        font.pixelSize: 10
                                        visible: text.length > 0
                                    }

                                    Label {
                                        text: activityItemDelegate.detailsExpanded ? "▾" : "›"
                                        color: root.palette.placeholderText
                                        visible: activityItemDelegate.modelData.expandable
                                    }
                                }

                                Label {
                                    Layout.fillWidth: true
                                    text: activityItemDelegate.modelData.context
                                    color: root.palette.placeholderText
                                    font.pixelSize: 11
                                    elide: Text.ElideMiddle
                                    visible: activityItemDelegate.detailsExpanded && text.length > 0
                                }

                                TextEdit {
                                    Layout.fillWidth: true
                                    text: activityItemDelegate.modelData.command
                                    color: root.palette.placeholderText
                                    font.family: Typography.monoFamily
                                    font.pixelSize: 11
                                    readOnly: true
                                    selectByMouse: true
                                    wrapMode: TextEdit.Wrap
                                    textFormat: TextEdit.PlainText
                                    visible: activityItemDelegate.detailsExpanded && text.length > 0
                                }

                                TextEdit {
                                    Layout.fillWidth: true
                                    text: activityItemDelegate.modelData.detail
                                    color: root.palette.text
                                    font.family: Typography.monoFamily
                                    font.pixelSize: 11
                                    readOnly: true
                                    selectByMouse: true
                                    wrapMode: TextEdit.Wrap
                                    textFormat: TextEdit.PlainText
                                    visible: activityItemDelegate.detailsExpanded && text.length > 0
                                }
                            }
                        }
                    }
                }
            }
        }

        Label {
            anchors.centerIn: parent
            width: Math.min(parent.width - 48, 360)
            text: root.controller.loadingConversation ? qsTr("Loading conversation…") : (root.controller.selectedThreadId ? qsTr("This conversation contains no displayable history.") : qsTr("Select a conversation to read it."))
            color: root.palette.placeholderText
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
            visible: timelineList.count === 0
        }
    }

    Timer {
        interval: 1000
        running: root.visible && root.controller.turnRunning
        repeat: true
        onTriggered: root.wallClockUnixMilliseconds = Date.now()
    }
}
