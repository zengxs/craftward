// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls

ComboBox {
    id: root

    required property string selectedValue
    property bool selectionEnabled: true
    property bool choicesReady: count > 0
    property var optionText: function (value) {
        return value;
    }
    property string toolTipText
    readonly property bool selectedValueIsListed: selectedValue.length > 0 && currentIndex >= 0

    signal valueSelected(string value)

    enabled: selectionEnabled && choicesReady
    delegate: ItemDelegate {
        required property int index

        width: root.width
        text: root.optionText(root.textAt(index))
        highlighted: root.highlightedIndex === index
    }
    currentIndex: {
        const observedCount = count;
        return observedCount > 0 ? indexOfValue(selectedValue) : -1;
    }
    onActivated: root.valueSelected(currentValue)

    ToolTip.delay: 500
    ToolTip.text: toolTipText
    ToolTip.visible: hovered
}
