// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls

Item {
    id: root

    property bool expanded: true
    property real expandedWidth: 260
    property real minimumExpandedWidth: 220
    property real maximumExpandedWidth: 420
    property bool resizing: false
    property bool ready: false
    property real animatedWidth: expanded ? expandedWidth : 0
    signal resized(real width)

    visible: expanded || animatedWidth > 0
    enabled: expanded
    clip: true
    SplitView.minimumWidth: Math.min(minimumExpandedWidth, animatedWidth)
    SplitView.maximumWidth: maximumExpandedWidth

    // SplitView owns the preferred width during a drag; restore the binding afterwards.
    Binding {
        target: root.SplitView
        property: "preferredWidth"
        value: root.animatedWidth
        when: !root.resizing
        restoreMode: Binding.RestoreNone
    }

    Behavior on animatedWidth {
        enabled: root.ready && !root.resizing
        NumberAnimation {
            duration: 160
            easing.type: Easing.OutCubic
        }
    }

    onWidthChanged: {
        if (resizing && expanded)
            resized(width);
    }
    Component.onCompleted: ready = true
}
