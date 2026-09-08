// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Shapes

Shape {
    id: root

    property color color: "transparent"
    preferredRendererType: Shape.CurveRenderer

    ShapePath {
        strokeColor: root.color
        strokeWidth: root.height
        fillColor: "transparent"
        capStyle: ShapePath.FlatCap
        startX: 0
        startY: root.height / 2

        PathLine {
            x: root.width
            y: root.height / 2
        }
    }
}
