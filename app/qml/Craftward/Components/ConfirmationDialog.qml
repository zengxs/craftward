import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

ModalDialog {
    id: control

    required property string message
    required property string acceptText
    property string rejectText: /*% "Cancel" */ qsTrId("craftward.action.cancel")
    property bool primaryAction: false

    anchors.centerIn: Overlay.overlay
    width: Math.min(420, Overlay.overlay.width - 48)

    contentItem: ColumnLayout {
        spacing: 12

        Label {
            Layout.fillWidth: true
            text: control.title
            font.pixelSize: 18
            font.weight: Font.DemiBold
            wrapMode: Text.WordWrap
        }

        Label {
            Layout.fillWidth: true
            text: control.message
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

            Button {
                text: control.rejectText
                visible: text.length > 0
                onClicked: control.reject()
            }

            Button {
                text: control.acceptText
                visible: !control.primaryAction
                onClicked: control.accept()
            }

            PrimaryButton {
                text: control.acceptText
                visible: control.primaryAction
                onClicked: control.accept()
            }
        }
    }
}
