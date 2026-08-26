// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma Singleton

import QtQuick

QtObject {
    readonly property string monoFamily: "monospace"
    readonly property font codeFont: Qt.font({
        "family": monoFamily
    })
    readonly property real codeLineHeightScale: 1
}
