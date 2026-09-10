// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma Singleton

import QtQuick

// Native regression tests cover font measurement and numbered-list gutters.
QtObject {
    function indentWidth(content, font) {
        return 2 * (font.pixelSize > 0 ? font.pixelSize : font.pointSize);
    }
}
