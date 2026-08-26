// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Craftward.Components

Control {
    id: root

    required property real durationMilliseconds
    required property int detailCount
    required property bool expanded
    property url timerIconSource: "qrc:///icons/fluent/timer-20-regular.svg"

    signal toggleRequested

    padding: 0
    background: null
    implicitHeight: headerColumn.implicitHeight

    contentItem: Column {
        id: headerColumn

        spacing: 4

        ItemDelegate {
            width: parent.width
            implicitHeight: 30
            leftPadding: 0
            rightPadding: 8
            topPadding: 4
            bottomPadding: 4
            hoverEnabled: true
            background: null
            onClicked: root.toggleRequested()

            contentItem: Row {
                id: disclosureRow

                objectName: "codexTimelineDisclosureRow"
                spacing: 6

                Label {
                    objectName: "codexTimelineElapsedLabel"
                    anchors.verticalCenter: parent.verticalCenter
                    text: /*% "Elapsed" */ qsTrId("craftward.codex.timeline.elapsed_label")
                    color: root.palette.placeholderText
                    font.pixelSize: 12
                    font.weight: Font.DemiBold
                    visible: root.durationMilliseconds >= 0
                }

                CodexElapsedBadge {
                    objectName: "codexTimelineElapsedBadge"
                    anchors.verticalCenter: parent.verticalCenter
                    durationMilliseconds: root.durationMilliseconds
                    timerIconSource: root.timerIconSource
                    visible: root.durationMilliseconds >= 0
                }

                Label {
                    objectName: "codexTimelineDetailsLabel"
                    anchors.verticalCenter: parent.verticalCenter
                    text: /*% "Details · %1" */ qsTrId("craftward.codex.timeline.details_count").arg(root.detailCount)
                    color: root.palette.placeholderText
                    font.pixelSize: 12
                    font.weight: Font.DemiBold
                    visible: root.durationMilliseconds < 0
                }

                AnimatedChevron {
                    objectName: "codexTimelineDisclosureChevron"
                    anchors.verticalCenter: parent.verticalCenter
                    expanded: root.expanded
                    chevronColor: root.palette.placeholderText
                    onClicked: root.toggleRequested()
                }
            }
        }

        Rectangle {
            width: parent.width
            height: 0.5
            color: root.palette.windowText
            opacity: 0.1
        }
    }
}
