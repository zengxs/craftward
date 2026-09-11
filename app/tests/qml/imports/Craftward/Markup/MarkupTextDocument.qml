// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

// QML shell tests use this seam stub. CraftwardMarkupSemanticTest exercises the
// real C++ adapter with Qt Quick and the production semantic parser.
QtObject {
    signal rendered
    property var textDocument
    property var surface
    readonly property bool hasInlineCode: false
    property font font
    property font codeFont
    property real lineHeightScale: 1
    property real listIndentWidth: 0
    property color textColor
    property color linkColor
    property color annotationBackground

    function documentPosition(surfacePosition) {
        return surfacePosition;
    }

    function firstLineAscent() {
        return 0;
    }
}
