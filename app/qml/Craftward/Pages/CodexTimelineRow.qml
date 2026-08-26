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

    required property CodexTimelinePageModel timelineModel
    property int sourceRow: -1
    property int dataRevision: -1
    required property bool turnExpanded
    required property bool forkEnabled
    required property bool showForkActions
    required property double wallClockUnixMilliseconds
    readonly property string entryId: String(root.value("entryId") ?? "")
    readonly property string turnId: String(root.value("turnId") ?? "")
    readonly property bool forkBoundary: Boolean(root.value("forkBoundary"))
    readonly property bool activityGroup: Boolean(root.value("activityGroup"))
    readonly property bool fromUser: Boolean(root.value("fromUser"))
    readonly property bool detailRow: Boolean(root.value("detailRow"))
    readonly property bool firstDetailInTurn: Boolean(root.value("firstDetailInTurn"))
    readonly property int detailCountInTurn: Number(root.value("detailCountInTurn"))
    readonly property bool standaloneActivity: Boolean(root.value("standaloneActivity"))
    readonly property bool presentationVisible: !root.detailRow || root.firstDetailInTurn || root.turnExpanded

    signal toggleTurnRequested(string turnId)
    signal forkRequested(string turnId)

    function value(roleName) {
        // Keep imperative valueAt() reads reactive without retaining every source row as a delegate.
        const currentRevision = root.dataRevision;
        return currentRevision >= 0 ? root.timelineModel.valueAt(root.sourceRow, roleName) : undefined;
    }

    function textValue(roleName) {
        const value = root.value(roleName);
        return value === undefined || value === null ? "" : String(value);
    }

    function activityStatusText(activity) {
        if (!activity.reasoning)
            return activity.statusLabel;

        const startedAt = Number(activity.startedAtUnixMilliseconds);
        if (startedAt <= 0)
            return activity.statusLabel;

        const completedAt = Number(activity.completedAtUnixMilliseconds);
        const endAt = completedAt > 0 ? completedAt : root.wallClockUnixMilliseconds;
        const elapsedSeconds = Math.max(0, Math.floor((endAt - startedAt) / 1000));
        return /*% "Processed %1 s" */ qsTrId("craftward.codex.timeline.processed_seconds").arg(elapsedSeconds);
    }

    function longestLine(text) {
        const lines = String(text).split("\n");
        let longest = "";
        for (const line of lines) {
            if (line.length > longest.length)
                longest = line;
        }
        return longest;
    }

    function resolvedTurnDurationMilliseconds() {
        const duration = Number(root.value("turnDurationMilliseconds"));
        if (Number.isFinite(duration) && duration >= 0)
            return duration;

        const startedAt = Number(root.value("turnStartedAtUnixSeconds"));
        const completedAt = Number(root.value("turnCompletedAtUnixSeconds"));
        return Number.isFinite(startedAt) && Number.isFinite(completedAt) && completedAt >= startedAt ? (completedAt - startedAt) * 1000 : -1;
    }

    function formattedDuration(durationMilliseconds) {
        const totalSeconds = Math.max(0, Math.floor(durationMilliseconds / 1000));
        const hours = Math.floor(totalSeconds / 3600);
        const minutes = Math.floor((totalSeconds % 3600) / 60);
        const seconds = totalSeconds % 60;
        if (hours > 0)
            return /*% "%1 h %2 min %3 s" */ qsTrId("craftward.codex.timeline.duration.hours_minutes_seconds").arg(hours).arg(minutes).arg(seconds);
        if (minutes > 0)
            return /*% "%1 min %2 s" */ qsTrId("craftward.codex.timeline.duration.minutes_seconds").arg(minutes).arg(seconds);
        return /*% "%1 s" */ qsTrId("craftward.codex.timeline.duration.seconds").arg(seconds);
    }

    padding: 0
    background: null
    visible: presentationVisible
    implicitHeight: visible ? rowColumn.implicitHeight : 0

    contentItem: Column {
        id: rowColumn

        spacing: 6

        Loader {
            id: primaryMessageLoader

            width: parent.width
            active: !root.detailRow && !root.activityGroup
            visible: active
            sourceComponent: primaryMessageComponent
        }

        Loader {
            id: standaloneActivityLoader

            width: parent.width
            active: root.standaloneActivity
            visible: active
            sourceComponent: standaloneActivityComponent
        }

        Loader {
            id: detailHeaderLoader

            width: parent.width
            active: root.detailRow && root.firstDetailInTurn
            visible: active
            sourceComponent: detailHeaderComponent
        }

        Loader {
            id: detailBodyLoader

            width: parent.width
            active: root.detailRow && root.turnExpanded
            visible: active
            sourceComponent: root.activityGroup ? activityGroupComponent : commentaryComponent
        }

        ToolButton {
            enabled: root.forkEnabled
            font.pixelSize: 11
            implicitHeight: visible ? 24 : 0
            text: /*% "Fork from here" */ qsTrId("craftward.codex.timeline.fork.action")
            visible: root.forkBoundary && root.showForkActions && (!root.detailRow || root.turnExpanded)
            onClicked: root.forkRequested(root.turnId)
        }
    }

    Component {
        id: primaryMessageComponent

        Item {
            id: messageRoot

            width: primaryMessageLoader.width
            readonly property real maximumWidth: root.fromUser ? Math.min(width * 0.72, 680) : Math.min(width, 820)
            readonly property real messageHorizontalPadding: root.fromUser ? 14 : 0
            readonly property real messageVerticalPadding: root.fromUser ? 10 : 0
            readonly property real messageWidth: root.fromUser ? Math.min(maximumWidth, Math.max(120, userTextMetrics.advanceWidth + messageHorizontalPadding * 2)) : maximumWidth
            implicitHeight: messageBody.height

            TextMetrics {
                id: userTextMetrics

                text: root.fromUser ? root.longestLine(root.textValue("text")) : ""
                font: root.font
            }

            Item {
                id: messageBody

                x: root.fromUser ? messageRoot.width - width - 6 : 0
                width: messageRoot.messageWidth
                height: messageRenderer.implicitHeight + messageRoot.messageVerticalPadding * 2

                Rectangle {
                    anchors.fill: parent
                    radius: 12
                    color: Theme.userMessageSurface
                    visible: root.fromUser
                }

                Loader {
                    id: messageRenderer

                    x: messageRoot.messageHorizontalPadding
                    y: messageRoot.messageVerticalPadding
                    width: parent.width - messageRoot.messageHorizontalPadding * 2
                    sourceComponent: messageComponent
                }
            }
        }
    }

    Component {
        id: messageComponent

        MarkupDocumentView {
            documentModel: root.value("markupDocument") ?? null
            textColor: root.palette.text
            font: root.font
            codeFont: Typography.codeFont
        }
    }

    Component {
        id: detailHeaderComponent

        ItemDelegate {
            id: detailHeader

            readonly property real turnDurationMilliseconds: root.resolvedTurnDurationMilliseconds()

            width: detailHeaderLoader.width
            implicitHeight: 32
            leftPadding: 0
            rightPadding: 8
            topPadding: 4
            bottomPadding: 4
            hoverEnabled: true
            background: null
            onClicked: root.toggleTurnRequested(root.turnId)

            contentItem: Row {
                spacing: 6

                Label {
                    text: detailHeader.turnDurationMilliseconds >= 0 ? /*% "Elapsed %1" */ qsTrId("craftward.codex.timeline.elapsed").arg(root.formattedDuration(detailHeader.turnDurationMilliseconds)) : /*% "Details · %1" */ qsTrId("craftward.codex.timeline.details_count").arg(root.detailCountInTurn)
                    color: root.palette.placeholderText
                    font.pixelSize: 12
                    font.weight: Font.DemiBold
                }

                Label {
                    text: /*% "× %1" */ qsTrId("craftward.codex.timeline.activity_count").arg(root.detailCountInTurn)
                    color: root.palette.placeholderText
                    font.pixelSize: 11
                    visible: detailHeader.turnDurationMilliseconds >= 0 && root.detailCountInTurn > 0
                }

                AnimatedChevron {
                    expanded: root.turnExpanded
                    chevronColor: root.palette.placeholderText
                    onClicked: root.toggleTurnRequested(root.turnId)
                }
            }
        }
    }

    Component {
        id: commentaryComponent

        MarkupDocumentView {
            width: Math.min(detailBodyLoader.width, 820)
            documentModel: root.value("markupDocument") ?? null
            textColor: root.palette.text
            font: root.font
            codeFont: Typography.codeFont
        }
    }

    Component {
        id: standaloneActivityComponent

        RowLayout {
            width: standaloneActivityLoader.width
            readonly property bool rowFailed: Boolean(root.value("failed"))
            readonly property bool rowRunning: Boolean(root.value("running"))
            spacing: 7

            Rectangle {
                Layout.preferredWidth: 7
                Layout.preferredHeight: 7
                radius: width / 2
                color: parent.rowFailed ? Theme.dangerForeground : (parent.rowRunning ? root.palette.highlight : root.palette.mid)
            }

            Label {
                text: root.textValue("activityLabel")
                color: root.palette.placeholderText
                font.weight: Font.DemiBold
            }
        }
    }

    Component {
        id: activityGroupComponent

        Column {
            id: activityGroup

            width: detailBodyLoader.width
            property bool groupExpanded: false
            readonly property bool rowFailed: Boolean(root.value("failed"))
            readonly property bool rowRunning: Boolean(root.value("running"))
            readonly property int rowActivityCount: Number(root.value("activityCount"))
            spacing: 2

            ItemDelegate {
                width: parent.width
                leftPadding: 0
                rightPadding: 8
                topPadding: 5
                bottomPadding: 5
                hoverEnabled: true
                background: null
                onClicked: activityGroup.groupExpanded = !activityGroup.groupExpanded

                contentItem: RowLayout {
                    spacing: 7

                    Rectangle {
                        Layout.preferredWidth: 7
                        Layout.preferredHeight: 7
                        radius: width / 2
                        color: activityGroup.rowFailed ? Theme.dangerForeground : (activityGroup.rowRunning ? root.palette.highlight : root.palette.mid)
                    }

                    Label {
                        text: root.textValue("activityLabel")
                        color: root.palette.placeholderText
                        font.weight: Font.DemiBold
                    }

                    Label {
                        text: /*% "× %1" */ qsTrId("craftward.codex.timeline.activity_count").arg(activityGroup.rowActivityCount)
                        color: root.palette.placeholderText
                        font.pixelSize: 11
                        visible: activityGroup.rowActivityCount > 1
                    }

                    AnimatedChevron {
                        expanded: activityGroup.groupExpanded
                        chevronColor: root.palette.placeholderText
                        onClicked: activityGroup.groupExpanded = !activityGroup.groupExpanded
                    }

                    Item {
                        Layout.fillWidth: true
                    }
                }
            }

            Loader {
                id: activityItemsLoader

                width: parent.width
                active: activityGroup.groupExpanded
                visible: active

                sourceComponent: Column {
                    width: activityItemsLoader.width
                    property var activityItems: root.value("activityItems") ?? []

                    Repeater {
                        model: parent.activityItems

                        delegate: ItemDelegate {
                            id: activityItemDelegate

                            required property var modelData
                            property bool detailsExpanded: false

                            width: parent.width
                            leftPadding: 18
                            rightPadding: 8
                            topPadding: 5
                            bottomPadding: 5
                            hoverEnabled: modelData.expandable
                            background: null
                            onClicked: {
                                if (modelData.expandable)
                                    detailsExpanded = !detailsExpanded;
                            }

                            contentItem: ColumnLayout {
                                spacing: 4

                                RowLayout {
                                    Layout.fillWidth: true
                                    spacing: 7

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
                                        color: activityItemDelegate.modelData.failed ? Theme.dangerForeground : root.palette.placeholderText
                                        font.pixelSize: 10
                                        visible: text.length > 0
                                    }

                                    AnimatedChevron {
                                        expanded: activityItemDelegate.detailsExpanded
                                        chevronColor: root.palette.placeholderText
                                        visible: activityItemDelegate.modelData.expandable
                                        onClicked: activityItemDelegate.detailsExpanded = !activityItemDelegate.detailsExpanded
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
    }
}
