import QtQuick
import QtQuick.Controls

Dialog {
    id: control

    padding: 24
    modal: true
    focus: true
    closePolicy: Popup.CloseOnEscape
    header: null
    footer: null

    background: Rectangle {
        radius: 14
        color: control.palette.window
        border.color: control.palette.mid
    }

    Overlay.modal: Rectangle {
        color: Qt.rgba(0, 0, 0, 0.18)
    }
}
