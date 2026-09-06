// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls

ToolTip {
    id: tip
    required property Item surface
    property var descriptor: null
    property rect area: Qt.rect(0, 0, surface.width, surface.height)
    property bool requested: false
    readonly property rect sourceRect: descriptor ? descriptor.rect : Qt.rect(0, 0, 0, 0)
    readonly property real gap: 8
    parent: surface
    popupType: Popup.Item
    visible: requested && descriptor !== null
    text: descriptor ? descriptor.hint : ""
    delay: 400
    timeout: 6000
    focus: false
    onDescriptorChanged: if (descriptor === null)
        hide()
    margins: 0
    width: Math.min(implicitWidth, Math.max(1, area.width - 12))
    x: Math.max(area.x + 6, Math.min(sourceRect.x + sourceRect.width / 2 - width / 2, area.x + area.width - width - 6))
    y: {
        const below = sourceRect.y + sourceRect.height + gap;
        const preferred = below + height <= area.y + area.height - 6 ? below : sourceRect.y - gap - height;
        return Math.max(area.y + 6, Math.min(preferred, area.y + area.height - height - 6));
    }
    enter: null
    exit: null
}
