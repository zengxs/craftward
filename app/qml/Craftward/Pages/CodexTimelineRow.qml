// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Components
import Craftward.Design

Control {
    id: root

    required property var timelineModel
    property int sourceRow: -1
    property int dataRevision: -1
    property string rendererName: "current"
    required property bool turnExpanded
    required property bool hasRunningEvidence
    required property bool activityShimmerEnabled
    required property bool forkEnabled
    required property bool showForkActions
    required property double wallClockUnixMilliseconds
    readonly property string entryId: String(root.value("entryId") ?? "")
    readonly property string heightCacheKey: root.rendererName + ":" + root.entryId
    readonly property bool contentMaterializationRequested: false
    readonly property bool contentMaterializationReady: true
    readonly property bool contentMeasurementReady: true
    readonly property string turnId: String(root.value("turnId") ?? "")
    readonly property bool turnForkable: Boolean(root.value("turnForkable"))
    readonly property bool latestTurn: Boolean(root.value("latestTurn"))
    readonly property bool activityGroup: Boolean(root.value("activityGroup"))
    readonly property bool fromUser: Boolean(root.value("fromUser"))
    readonly property bool finalAnswer: Boolean(root.value("finalAnswer"))
    readonly property bool detailRow: Boolean(root.value("detailRow"))
    readonly property bool firstDetailInTurn: Boolean(root.value("firstDetailInTurn"))
    readonly property int detailCountInTurn: Number(root.value("detailCountInTurn"))
    readonly property bool standaloneActivity: Boolean(root.value("standaloneActivity"))
    readonly property bool semanticBlock: Boolean(root.value("semanticBlock"))
    readonly property bool firstBlockInEntry: !root.semanticBlock || Boolean(root.value("firstBlockInEntry"))
    readonly property bool lastBlockInEntry: !root.semanticBlock || Boolean(root.value("lastBlockInEntry"))
    readonly property real semanticBlockSpacing: root.semanticBlock && !root.lastBlockInEntry ? 8 : 0
    readonly property real semanticEntrySpacing: root.rendererName === "semantic" && (!root.semanticBlock || root.lastBlockInEntry) ? 10 : 0
    readonly property bool presentationVisible: !root.detailRow || root.firstDetailInTurn || root.turnExpanded

    signal toggleTurnRequested(string turnId)
    signal forkRequested(string turnId)

    function prepareItemForLayout(item) {
        if (!item)
            return;
        if (typeof item.prepareForLayout === "function")
            item.prepareForLayout();
        else if (typeof item.forceLayout === "function")
            item.forceLayout();
    }

    function prepareForLayout() {
        for (const loader of [primaryMessageLoader, standaloneActivityLoader, detailHeaderLoader, detailBodyLoader])
            root.prepareItemForLayout(loader.item);
        rowColumn.forceLayout();
    }

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

    padding: 0
    background: null
    visible: presentationVisible
    implicitHeight: visible ? rowColumn.implicitHeight + root.semanticEntrySpacing + (root.detailRow ? root.semanticBlockSpacing : 0) : 0

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
            active: root.detailRow && root.firstDetailInTurn && root.firstBlockInEntry
            visible: active
            sourceComponent: detailHeaderComponent
        }

        Loader {
            id: detailBodyLoader

            width: parent.width
            active: root.detailRow && root.turnExpanded
            visible: active
            sourceComponent: root.activityGroup ? activityGroupComponent : (root.semanticBlock ? semanticBlockComponent : commentaryComponent)
        }
    }

    Component {
        id: primaryMessageComponent

        Item {
            id: messageRoot

            function prepareForLayout() {
                root.prepareItemForLayout(messageRenderer.item);
            }

            width: primaryMessageLoader.width
            readonly property real userMaximumWidth: Math.min(width * 0.72, 680)
            readonly property real messageHorizontalPadding: root.fromUser ? 14 : 0
            readonly property real messageTopPadding: root.fromUser && (!root.semanticBlock || root.firstBlockInEntry) ? 10 : 0
            readonly property real messageBottomPadding: root.semanticBlock && !root.lastBlockInEntry ? root.semanticBlockSpacing : (root.fromUser ? 10 : 0)
            readonly property real messageWidth: root.fromUser ? Math.min(userMaximumWidth, Math.max(120, userTextMetrics.advanceWidth + messageHorizontalPadding * 2)) : width
            implicitHeight: messageContent.height

            TextMetrics {
                id: userTextMetrics

                text: root.fromUser ? root.longestLine(root.textValue("text")) : ""
                font: root.font
            }

            Item {
                id: messageContent

                x: root.fromUser ? messageRoot.width - width : 0
                width: messageRoot.messageWidth
                height: messageBody.height + messageActions.implicitHeight + (messageActions.available ? 2 : 0)

                HoverHandler {
                    id: messageHover
                }

                CodexMessageActionState {
                    id: messageActionState

                    fromUser: root.fromUser && root.lastBlockInEntry
                    finalAnswer: root.finalAnswer && root.lastBlockInEntry
                    latestTurn: root.latestTurn
                    hasRunningEvidence: root.hasRunningEvidence
                    hovered: messageHover.hovered
                    copyFeedbackActive: messageActions.copied
                    turnForkable: root.turnForkable
                    showForkActions: root.showForkActions
                }

                Item {
                    id: messageBody

                    width: parent.width
                    height: messageRenderer.implicitHeight + messageRoot.messageTopPadding + messageRoot.messageBottomPadding

                    Rectangle {
                        id: userMessageSurface

                        objectName: "codexUserMessageSurface"
                        anchors.fill: parent
                        radius: 12
                        color: Theme.userMessageSurface
                        visible: root.fromUser

                        Rectangle {
                            anchors {
                                left: parent.left
                                right: parent.right
                                top: parent.top
                            }
                            height: Math.min(parent.radius, parent.height)
                            color: parent.color
                            visible: root.semanticBlock && !root.firstBlockInEntry
                        }

                        Rectangle {
                            anchors {
                                left: parent.left
                                right: parent.right
                                bottom: parent.bottom
                            }
                            height: Math.min(parent.radius, parent.height)
                            color: parent.color
                            visible: root.semanticBlock && !root.lastBlockInEntry
                        }
                    }

                    Loader {
                        id: messageRenderer

                        objectName: "codexMessageRenderer"
                        x: messageRoot.messageHorizontalPadding
                        y: messageRoot.messageTopPadding
                        width: parent.width - messageRoot.messageHorizontalPadding * 2
                        sourceComponent: root.semanticBlock ? semanticBlockComponent : messageComponent
                    }
                }

                CodexMessageActions {
                    id: messageActions

                    objectName: "codexMessageActions"
                    x: root.fromUser ? parent.width - width : 0
                    y: messageBody.height + 2
                    available: messageActionState.available
                    revealed: messageActionState.revealed
                    forkVisible: messageActionState.forkVisible
                    forkEnabled: root.forkEnabled
                    onCopyRequested: {
                        if (ApplicationClipboard.copyText(root.textValue("text")))
                            messageActions.confirmCopied();
                    }
                    onForkRequested: root.forkRequested(root.turnId)
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
        id: semanticBlockComponent

        MarkupSegmentView {
            codeBlock: Boolean(root.value("codeBlock"))
            segmentText: root.textValue("blockText")
            language: root.textValue("language")
            markdown: Boolean(root.value("markdown"))
            textColor: root.palette.text
            font: root.font
            codeFont: Typography.codeFont
        }
    }

    Component {
        id: detailHeaderComponent

        CodexTimelineDetailHeader {
            width: detailHeaderLoader.width
            durationMilliseconds: root.resolvedTurnDurationMilliseconds()
            detailCount: root.detailCountInTurn
            expanded: root.turnExpanded
            font: root.font
            onToggleRequested: root.toggleTurnRequested(root.turnId)
        }
    }

    Component {
        id: commentaryComponent

        MarkupDocumentView {
            width: detailBodyLoader.width
            documentModel: root.value("markupDocument") ?? null
            textColor: root.palette.text
            font: root.font
            codeFont: Typography.codeFont
        }
    }

    Component {
        id: standaloneActivityComponent

        CodexActivityIdentity {
            id: standaloneActivity

            width: standaloneActivityLoader.width
            presentationKind: root.textValue("activityPresentationKind")
            activityLabel: root.textValue("activityLabel")
            failed: Boolean(root.value("failed"))
            running: Boolean(root.value("running"))
            shimmerEnabled: root.activityShimmerEnabled
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

                    CodexActivityIdentity {
                        Layout.preferredWidth: implicitWidth
                        Layout.preferredHeight: implicitHeight
                        presentationKind: root.textValue("activityPresentationKind")
                        activityLabel: root.textValue("activityLabel")
                        activityCount: activityGroup.rowActivityCount
                        failed: activityGroup.rowFailed
                        running: activityGroup.rowRunning
                        shimmerEnabled: root.activityShimmerEnabled
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
