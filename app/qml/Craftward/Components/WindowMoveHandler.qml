import QtQuick

DragHandler {
    required property Window targetWindow

    target: null
    acceptedButtons: Qt.LeftButton
    grabPermissions: PointerHandler.ApprovesTakeOverByAnything
    // QTBUG-141220: The Qt Cocoa backend can make startSystemMove() fail
    // intermittently because NSApp.currentEvent may not be a mouse event when
    // this callback runs. Upgrade Qt as soon as a release containing the fix is
    // available:
    // https://qt-project.atlassian.net/browse/QTBUG-141220
    onActiveChanged: if (active)
        targetWindow.startSystemMove()
}
