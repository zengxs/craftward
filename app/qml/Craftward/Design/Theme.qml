// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma Singleton

import QtQuick

QtObject {
    readonly property SystemPalette system: SystemPalette {
        colorGroup: SystemPalette.Active
    }

    readonly property bool dark: system.window.hslLightness < system.windowText.hslLightness

    readonly property color accent: system.accent
    readonly property color dangerForeground: dark ? TailwindColors.red400 : TailwindColors.red700
    readonly property color dangerSurface: Qt.rgba(dangerForeground.r, dangerForeground.g, dangerForeground.b, dark ? 0.14 : 0.08)
    readonly property color dangerBorder: Qt.rgba(dangerForeground.r, dangerForeground.g, dangerForeground.b, dark ? 0.36 : 0.24)
    readonly property color modalScrim: Qt.rgba(TailwindColors.black.r, TailwindColors.black.g, TailwindColors.black.b, dark ? 0.32 : 0.18)
    readonly property color navigationSelectionBackground: dark ? TailwindColors.zinc800 : TailwindColors.zinc100
    readonly property color navigationSelectionForeground: accent
    readonly property color navigationPressedBackground: dark ? TailwindColors.zinc700 : TailwindColors.zinc200
    readonly property color sidebarSurface: dark ? TailwindColors.zinc900 : TailwindColors.zinc50
}
