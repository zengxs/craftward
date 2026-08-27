// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

Item {
    id: root

    required property bool active
    required property color shimmerColor

    clip: true
    visible: active

    Rectangle {
        id: highlightBand

        readonly property real bandWidth: Math.max(30, root.width * 0.32)

        x: -bandWidth
        width: bandWidth
        height: parent.height
        gradient: Gradient {
            orientation: Gradient.Horizontal
            GradientStop {
                position: 0
                color: "transparent"
            }
            GradientStop {
                position: 0.5
                color: Qt.rgba(root.shimmerColor.r, root.shimmerColor.g, root.shimmerColor.b, 0.72)
            }
            GradientStop {
                position: 1
                color: "transparent"
            }
        }

        NumberAnimation on x {
            running: root.active && root.width > 0
            loops: Animation.Infinite
            from: -highlightBand.bandWidth
            to: root.width
            duration: 1450
            easing.type: Easing.Linear
        }
    }
}
