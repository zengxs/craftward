// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl
import QtQuick.Effects
import Craftward.Components
import Craftward.Design

Control {
    id: root

    required property real durationMilliseconds
    property url timerIconSource: "qrc:///icons/fluent/timer-20-regular.svg"
    readonly property string clockText: root.formatClockDuration(root.durationMilliseconds)
    readonly property string durationDescription: root.formatDurationDescription(root.durationMilliseconds)
    readonly property string description: /*% "Elapsed %1" */ qsTrId("craftward.codex.timeline.elapsed").arg(root.durationDescription)

    function paddedTwoDigit(value) {
        return String(value).padStart(2, "0");
    }

    function formatClockDuration(durationMilliseconds) {
        const totalSeconds = Math.max(0, Math.floor(durationMilliseconds / 1000));
        const hours = Math.floor(totalSeconds / 3600);
        const minutes = Math.floor((totalSeconds % 3600) / 60);
        const seconds = totalSeconds % 60;
        const minuteAndSecondText = root.paddedTwoDigit(minutes) + ":" + root.paddedTwoDigit(seconds);
        return hours > 0 ? root.paddedTwoDigit(hours) + ":" + minuteAndSecondText : minuteAndSecondText;
    }

    function formatDurationDescription(durationMilliseconds) {
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

    implicitWidth: badgeContent.implicitWidth + leftPadding + rightPadding
    implicitHeight: 22
    leftPadding: 7
    rightPadding: 7
    topPadding: 0
    bottomPadding: 0
    Accessible.name: description

    background: Item {
        RectangularShadow {
            anchors.fill: badgeSurface
            radius: badgeSurface.radius
            blur: 2
            cached: true
            color: Theme.metadataBadgeShadow
            offset: Qt.vector2d(0, 1)
        }

        Rectangle {
            id: badgeSurface

            anchors.fill: parent
            radius: height / 2
            color: Theme.metadataBadgeSurface
            border.color: Theme.metadataBadgeRing
            border.width: 0.5
            border.pixelAligned: false
        }
    }

    contentItem: Row {
        id: badgeContent

        spacing: 4

        ControlsImpl.IconImage {
            objectName: "codexElapsedBadgeTimerIcon"
            anchors.verticalCenter: parent.verticalCenter
            width: 15
            height: 15
            source: root.timerIconSource
            sourceSize.width: 20
            sourceSize.height: 20
            color: root.palette.placeholderText
        }

        Label {
            objectName: "codexElapsedBadgeClockText"
            anchors.verticalCenter: parent.verticalCenter
            text: root.clockText
            color: root.palette.placeholderText
            font.family: Typography.monoFamily
            font.pixelSize: 11
            font.weight: Font.Medium
        }
    }

    HoverHandler {
        id: badgeHover
    }

    ToolTip.visible: badgeHover.hovered
    ToolTip.text: root.description
}
