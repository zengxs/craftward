// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

DragHandler {
    required property Window targetWindow

    target: null
    acceptedButtons: Qt.LeftButton
    grabPermissions: PointerHandler.ApprovesTakeOverByAnything
    // QTBUG-141220: Qt 6.11.1 can observe an unrelated NSApp.currentEvent here.
    // Remove the native workaround after upgrading to a Qt release containing
    // the upstream fix:
    // https://qt-project.atlassian.net/browse/QTBUG-141220
    onActiveChanged: if (active)
        WindowMoveHelper.startSystemMove(targetWindow)
}
