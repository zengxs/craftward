// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Craftward.Components

MenuComboBox {
    id: root

    required property string selectedValue
    property bool selectionEnabled: true
    property bool choicesReady: count > 0
    property string toolTipText
    readonly property bool selectedValueIsListed: selectedValue.length > 0 && currentIndex >= 0

    signal valueSelected(string value)

    enabled: selectionEnabled && choicesReady
    currentIndex: {
        const observedCount = count;
        return observedCount > 0 ? indexOfValue(selectedValue) : -1;
    }
    onActivated: root.valueSelected(currentValue)

    ToolTip.delay: 500
    ToolTip.text: toolTipText
    ToolTip.visible: hovered
}
