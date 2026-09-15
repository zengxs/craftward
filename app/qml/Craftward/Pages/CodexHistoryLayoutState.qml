// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    property bool sidebarExpanded: true
    property bool filesExpanded: true
    property real availableWidth: 960
    property real sidebarWidth: 260
    property real filesWidth: 240
    readonly property real minimumSidebarWidth: 220
    readonly property real maximumSidebarWidth: 420
    readonly property real minimumFilesWidth: 200
    readonly property real maximumFilesWidth: 360
    readonly property real minimumContentWidth: 360
    readonly property real tabBarHeight: 32
    readonly property real contentHeaderHeight: 36
    readonly property real statusBarHeight: 28

    readonly property bool filesVisible: filesExpanded && availableWidth >= minimumContentWidth + minimumFilesWidth + (sidebarExpanded ? minimumSidebarWidth : 0) + 2
    readonly property real bodySidebarWidth: sidebarExpanded ? sidebarWidth : 0

    function rememberSidebarWidth(width) {
        if (Number.isFinite(width) && width > 0)
            sidebarWidth = Math.max(minimumSidebarWidth, Math.min(maximumSidebarWidth, width));
    }

    function rememberFilesWidth(width) {
        if (Number.isFinite(width) && width > 0)
            filesWidth = Math.max(minimumFilesWidth, Math.min(maximumFilesWidth, width));
    }

    function toggleSidebar() {
        sidebarExpanded = !sidebarExpanded;
    }
}
