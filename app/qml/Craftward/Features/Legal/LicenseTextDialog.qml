import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Components

ModalDialog {
    id: root

    property string documentTitle
    property url documentUri
    property string documentText
    property string errorMessage

    function loadDocument() {
        const result = ResourceTextReader.read(root.documentUri);
        root.documentText = result.text;
        root.errorMessage = result.errorMessage;
    }

    anchors.centerIn: Overlay.overlay
    width: Math.min(720, Overlay.overlay.width - 48)
    height: Math.min(520, Overlay.overlay.height - 48)
    visible: false
    title: root.documentTitle
    onOpened: root.loadDocument()

    contentItem: ColumnLayout {
        spacing: 12

        Label {
            Layout.fillWidth: true
            text: root.title
            font.pixelSize: 18
            font.weight: Font.DemiBold
            wrapMode: Text.WordWrap
        }

        LegalTextView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            text: root.documentText
            errorMessage: root.errorMessage
        }

        RowLayout {
            Layout.fillWidth: true

            Item {
                Layout.fillWidth: true
            }

            Button {
                text: /*% "Close" */ qsTrId("craftward.action.close")
                onClicked: root.close()
            }
        }
    }
}
