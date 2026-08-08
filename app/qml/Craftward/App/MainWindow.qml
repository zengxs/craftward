import QtQuick
import QtQuick.Controls
import Craftward.Components
import Craftward.Pages

ApplicationWindow {
    id: window

    width: 960
    height: 640
    minimumWidth: 640
    minimumHeight: 480
    flags: Qt.Window | Qt.ExpandedClientAreaHint | Qt.NoTitleBarBackgroundHint
    visible: true
    title: ""

    background: Rectangle {
        color: window.palette.window

        WindowMoveHandler {
            targetWindow: window
        }
    }

    StackView {
        id: stackView

        anchors.fill: parent
        initialItem: ScaffoldPage {}
    }
}
