// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    property bool sidebarExpanded: true
    property real sidebarWidth: 310
    property real minimumSidebarWidth: 240
    property real maximumSidebarWidth: 420
    property real titleBarLeadingInset: 78
    property real titleBarButtonWidth: 28
    property int collapsedButtonCount: 3

    readonly property real effectiveSidebarWidth: Math.max(minimumSidebarWidth, Math.min(maximumSidebarWidth, sidebarWidth))
    readonly property real collapsedNavigationWidth: titleBarLeadingInset + titleBarButtonWidth * collapsedButtonCount
    readonly property real bodySidebarWidth: sidebarExpanded ? effectiveSidebarWidth : 0
    readonly property real navigationChromeWidth: sidebarExpanded ? effectiveSidebarWidth : collapsedNavigationWidth
    readonly property real leadingActionsX: titleBarLeadingInset + (sidebarExpanded ? 0 : titleBarButtonWidth)
    readonly property real collapsedSidebarToggleX: titleBarLeadingInset

    function rememberSidebarWidth(width) {
        if (Number.isFinite(width) && width > 0)
            sidebarWidth = Math.max(minimumSidebarWidth, Math.min(maximumSidebarWidth, width));
    }

    function toggleSidebar() {
        sidebarExpanded = !sidebarExpanded;
    }
}
