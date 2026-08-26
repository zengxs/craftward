// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls

ToolButton {
    id: control

    property string toolTipText
    property real backgroundInset: 0
    property real iconRotation: 0
    property bool forceToolTipVisible: false

    contentItem: Item {
        rotation: control.iconRotation
    }
}
