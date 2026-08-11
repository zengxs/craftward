import QtQuick
import QtQuick.Controls
import Craftward.Editor

Control {
    id: root

    property alias text: documentEditor.text
    property string errorMessage

    implicitWidth: 320
    implicitHeight: 240
    padding: 1

    background: Rectangle {
        radius: 5
        color: root.palette.base
        border.width: 1
        border.color: Qt.rgba(root.palette.windowText.r, root.palette.windowText.g, root.palette.windowText.b, 0.16)
    }

    contentItem: Item {
        Label {
            anchors.centerIn: parent
            width: Math.max(0, Math.min(parent.width - 48, 440))
            visible: root.errorMessage.length > 0
            text: root.errorMessage
            horizontalAlignment: Text.AlignHCenter
            wrapMode: Text.WordWrap
        }

        CodeEditor {
            id: documentEditor

            anchors.fill: parent
            visible: root.errorMessage.length === 0
            leftPadding: 8
            rightPadding: 2
            topPadding: 6
            bottomPadding: 6
            readOnly: true
            wordWrap: true
        }
    }
}
