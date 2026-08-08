import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Components

Page {
    id: root

    Item {
        anchors.fill: parent

        WindowMoveHandler {
            targetWindow: root.Window.window
        }
    }

    ColumnLayout {
        anchors.centerIn: parent
        width: Math.min(parent.width - 64, 560)
        spacing: 16

        Label {
            Layout.fillWidth: true
            text: qsTr("Craftward")
            font.pixelSize: 36
            font.weight: Font.DemiBold
            horizontalAlignment: Text.AlignHCenter
        }

        Label {
            Layout.fillWidth: true
            text: qsTr("From intent to artifact.")
            font.pixelSize: 17
            horizontalAlignment: Text.AlignHCenter
        }

        Item {
            Layout.preferredHeight: 8
        }

        PrimaryButton {
            Layout.alignment: Qt.AlignHCenter
            text: qsTr("Create something")
            onClicked: readyDialog.open()
        }
    }

    Dialog {
        id: readyDialog

        anchors.centerIn: Overlay.overlay
        width: Math.min(380, root.width - 48)
        padding: 24
        modal: true
        focus: true
        title: qsTr("Scaffold ready")
        closePolicy: Popup.CloseOnEscape
        header: null
        footer: null

        background: Item {
            Rectangle {
                x: 0
                y: 3
                width: parent.width
                height: parent.height
                radius: 14
                color: readyDialog.palette.shadow
                opacity: 0.18
            }

            Rectangle {
                anchors.fill: parent
                radius: 14
                color: readyDialog.palette.window
                border.color: readyDialog.palette.mid
            }
        }

        contentItem: ColumnLayout {
            spacing: 12

            Label {
                Layout.fillWidth: true
                text: readyDialog.title
                font.pixelSize: 18
                font.weight: Font.DemiBold
                wrapMode: Text.WordWrap
            }

            Label {
                Layout.fillWidth: true
                text: qsTr("Craftward is ready for its first feature.")
                wrapMode: Text.WordWrap
            }

            Item {
                Layout.preferredHeight: 4
            }

            RowLayout {
                Layout.fillWidth: true

                Item {
                    Layout.fillWidth: true
                }

                PrimaryButton {
                    text: qsTr("OK")
                    onClicked: readyDialog.accept()
                }
            }
        }

        Overlay.modal: Rectangle {
            color: Qt.rgba(0, 0, 0, 0.18)
        }
    }
}
