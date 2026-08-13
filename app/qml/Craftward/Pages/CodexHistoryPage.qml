// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Codex
import Craftward.Components

Page {
    id: root

    required property CodexHistoryController controller

    background: Rectangle {
        color: root.palette.window
    }

    WindowMoveHandler {
        targetWindow: root.ApplicationWindow.window
    }

    SplitView {
        anchors {
            fill: parent
            topMargin: root.SafeArea.margins.top + 20
            leftMargin: Math.max(24, root.SafeArea.margins.left)
            rightMargin: Math.max(24, root.SafeArea.margins.right)
            bottomMargin: Math.max(24, root.SafeArea.margins.bottom)
        }
        orientation: Qt.Horizontal

        Rectangle {
            SplitView.minimumWidth: 240
            SplitView.preferredWidth: 310
            SplitView.maximumWidth: 420
            radius: 12
            color: root.palette.base
            border.color: root.palette.mid

            ColumnLayout {
                anchors {
                    fill: parent
                    margins: 14
                }
                spacing: 10

                RowLayout {
                    Layout.fillWidth: true

                    Label {
                        Layout.fillWidth: true
                        text: qsTr("Codex History")
                        font.pixelSize: 20
                        font.weight: Font.DemiBold
                    }

                    BusyIndicator {
                        Layout.preferredWidth: 20
                        Layout.preferredHeight: 20
                        running: root.controller.loadingThreads
                        visible: running
                    }

                    Button {
                        text: qsTr("Refresh")
                        enabled: !root.controller.loadingThreads
                        onClicked: root.controller.refresh()
                    }
                }

                Label {
                    Layout.fillWidth: true
                    text: qsTr("Persisted conversations from the local Codex app-server.")
                    color: root.palette.placeholderText
                    wrapMode: Text.WordWrap
                }

                ListView {
                    id: threadList

                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 4
                    model: root.controller.threads
                    ScrollBar.vertical: OverlayScrollBar {}

                    delegate: ItemDelegate {
                        id: threadDelegate

                        required property string threadId
                        required property string title
                        required property string preview
                        required property string workingDirectory
                        required property date updatedAt

                        width: ListView.view.width
                        checkable: true
                        checked: root.controller.selectedThreadId === threadId
                        hoverEnabled: true
                        leftPadding: 12
                        rightPadding: 12
                        topPadding: 10
                        bottomPadding: 10
                        onClicked: root.controller.selectThread(threadId, title)

                        contentItem: ColumnLayout {
                            spacing: 3

                            Label {
                                Layout.fillWidth: true
                                text: threadDelegate.title || qsTr("Untitled conversation")
                                font.weight: Font.DemiBold
                                elide: Text.ElideRight
                            }

                            Label {
                                Layout.fillWidth: true
                                text: threadDelegate.preview
                                color: root.palette.placeholderText
                                maximumLineCount: 2
                                elide: Text.ElideRight
                                wrapMode: Text.WordWrap
                            }

                            Label {
                                Layout.fillWidth: true
                                text: threadDelegate.workingDirectory
                                color: root.palette.placeholderText
                                font.pixelSize: 11
                                elide: Text.ElideMiddle
                            }
                        }
                    }

                    Label {
                        anchors.centerIn: parent
                        width: Math.min(parent.width - 32, 240)
                        text: root.controller.loadingThreads ? qsTr("Loading conversations…") : qsTr("No persisted conversations were found.")
                        color: root.palette.placeholderText
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        visible: threadList.count === 0
                    }
                }
            }
        }

        Item {
            SplitView.minimumWidth: 360
            SplitView.fillWidth: true

            ColumnLayout {
                anchors {
                    fill: parent
                    leftMargin: 22
                }
                spacing: 12

                RowLayout {
                    Layout.fillWidth: true

                    Label {
                        Layout.fillWidth: true
                        text: root.controller.selectedThreadTitle || qsTr("Conversation")
                        font.pixelSize: 24
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                    }

                    BusyIndicator {
                        Layout.preferredWidth: 22
                        Layout.preferredHeight: 22
                        running: root.controller.loadingConversation
                        visible: running
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    implicitHeight: errorLayout.implicitHeight + 20
                    radius: 9
                    color: Qt.rgba(0.82, 0.12, 0.16, 0.08)
                    border.color: Qt.rgba(0.82, 0.12, 0.16, 0.24)
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
                            color: "#B4232A"
                            wrapMode: Text.WordWrap
                        }

                        IconButton {
                            icon.source: "qrc:///icons/phosphor/x-circle.svg"
                            toolTipText: qsTr("Dismiss error")
                            onClicked: root.controller.clearError()
                        }
                    }
                }

                Label {
                    Layout.fillWidth: true
                    text: qsTr("Some runtime activity may be unavailable in persisted history.")
                    color: root.palette.placeholderText
                    font.pixelSize: 11
                    wrapMode: Text.WordWrap
                    visible: root.controller.selectedThreadId.length > 0 && root.controller.activityHistoryPartial
                }

                ListView {
                    id: timelineList

                    readonly property bool conversationLoading: root.controller.loadingConversation
                    readonly property string selectedThreadId: root.controller.selectedThreadId
                    property bool initialPositionActive: false
                    property bool initialPositionScheduled: false
                    property string pendingInitialPositionThreadId
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

                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    clip: true
                    spacing: 10
                    model: root.controller.timeline
                    // Temporary mitigation for ListView's variable-height content estimate.
                    // Replace this with cached row heights and anchored scrolling.
                    cacheBuffer: Math.max(height * 4, 2048)
                    ScrollBar.vertical: OverlayScrollBar {}

                    onContentHeightChanged: {
                        if (!initialPositionActive)
                            return;
                        initialPositionStabilityTimer.restart();
                        scheduleInitialPosition();
                    }
                    onConversationLoadingChanged: scheduleInitialPosition()
                    onDraggingChanged: {
                        if (dragging)
                            cancelInitialPosition();
                    }
                    onSelectedThreadIdChanged: {
                        cancelInitialPosition();
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
                        property bool groupExpanded: false

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
                                        property bool detailsExpanded: false

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
                                                    text: activityItemDelegate.modelData.statusLabel
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
            }
        }
    }
}
