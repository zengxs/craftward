import QtQuick
import QtQuick.Controls

Control {
    id: root

    property alias text: backend.text
    property alias readOnly: backend.readOnly
    property alias wordWrap: backend.wordWrap

    implicitWidth: 320
    implicitHeight: 240
    focusPolicy: Qt.StrongFocus

    background: null

    contentItem: WindowContainer {
        activeFocusOnTab: true
        window: backend.window
    }

    ScintillaEditorBackend {
        id: backend

        fontFamily: root.font.family.length > 0 ? root.font.family : "Menlo"
        fontPointSize: root.font.pointSize > 0 ? root.font.pointSize : 13
        foregroundColor: root.palette.text
        backgroundColor: root.palette.base
        selectionForegroundColor: root.palette.text
        selectionBackgroundColor: root.palette.highlight
    }
}
