import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Components

Page {
    id: root

    Item {
        anchors.fill: parent

        WindowMoveHandler {
            targetWindow: root.ApplicationWindow.window
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

    ModalDialog {
        id: readyDialog

        anchors.centerIn: Overlay.overlay
        width: Math.min(380, root.width - 48)
        title: qsTr("Scaffold ready")

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
    }
}
