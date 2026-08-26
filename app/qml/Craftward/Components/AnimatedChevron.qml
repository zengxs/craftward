// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls

ToolButton {
    id: control

    required property bool expanded
    property color chevronColor: palette.placeholderText

    implicitWidth: 16
    implicitHeight: 16
    padding: 0
    display: AbstractButton.IconOnly
    hoverEnabled: false
    focusPolicy: Qt.NoFocus
    rotation: expanded ? 90 : 0
    icon.source: "qrc:///icons/fluent/chevron-right-20-regular.svg"
    icon.width: 16
    icon.height: 16
    icon.color: chevronColor
    background: null
    Accessible.ignored: true

    Behavior on rotation {
        NumberAnimation {
            duration: 140
            easing.type: Easing.OutCubic
        }
    }
}
