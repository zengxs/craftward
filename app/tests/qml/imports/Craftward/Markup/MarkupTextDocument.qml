// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

// QML shell tests use this seam stub. CraftwardMarkupSemanticTest exercises the
// real C++ adapter with Qt Quick and the production semantic parser.
QtObject {
    property var textDocument
    property var segment
    property font font
    property font codeFont
    property color textColor
    property color linkColor
    property color codeBackground
}
