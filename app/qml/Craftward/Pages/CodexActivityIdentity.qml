// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Design

Control {
    id: root

    required property string presentationKind
    required property string activityLabel
    required property bool failed
    required property bool running
    required property bool shimmerEnabled
    property int activityCount: 1

    padding: 0

    contentItem: Item {
        implicitWidth: identityLayout.implicitWidth
        implicitHeight: identityLayout.implicitHeight

        RowLayout {
            id: identityLayout

            spacing: 7

            CodexActivityGlyph {
                Layout.preferredWidth: 16
                Layout.preferredHeight: 16
                presentationKind: root.presentationKind
                glyphColor: root.failed ? Theme.dangerForeground : (root.running ? root.palette.highlight : root.palette.mid)
            }

            Label {
                text: root.activityLabel
                color: root.palette.placeholderText
                font.weight: Font.DemiBold
            }

            Label {
                text: /*% "× %1" */ qsTrId("craftward.codex.timeline.activity_count").arg(root.activityCount)
                color: root.palette.placeholderText
                font.pixelSize: 11
                visible: root.activityCount > 1
            }
        }

        CodexActivityShimmer {
            objectName: "codexActivityShimmer"
            anchors.fill: identityLayout
            active: root.running && root.shimmerEnabled
            shimmerColor: root.palette.base
        }
    }
}
