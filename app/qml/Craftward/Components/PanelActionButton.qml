// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Craftward.Design

ToolButton {
    id: control

    property string toolTipText: ""

    implicitWidth: 26
    implicitHeight: 26
    padding: 5
    display: AbstractButton.IconOnly
    hoverEnabled: true
    icon.width: 16
    icon.height: 16
    icon.color: !enabled ? TailwindColors.zinc400 : hovered || down ? TailwindColors.zinc700 : TailwindColors.zinc500
    Accessible.name: toolTipText

    background: Rectangle {
        anchors.fill: parent
        anchors.margins: 3
        radius: 4
        color: control.down ? TailwindColors.zinc300 : control.hovered ? TailwindColors.zinc200 : TailwindColors.transparent
        border.width: control.visualFocus ? 1 : 0
        border.color: TailwindColors.blue600
    }

    ToolTip.visible: hovered && !down && toolTipText.length > 0
    ToolTip.delay: 500
    ToolTip.text: toolTipText
}
