import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Page {
    id: root

    background: Rectangle {
        color: root.palette.window
    }

    ColumnLayout {
        anchors {
            top: parent.top
            left: parent.left
            right: parent.right
            topMargin: 28
            leftMargin: 32
            rightMargin: 32
        }
        spacing: 8

        Label {
            Layout.fillWidth: true
            text: qsTr("General")
            font.pixelSize: 20
            font.weight: Font.DemiBold
        }

        Label {
            Layout.fillWidth: true
            text: qsTr("No settings are available yet.")
            font.pixelSize: 13
            color: root.palette.placeholderText
            wrapMode: Text.WordWrap
        }
    }
}
