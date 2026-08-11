import QtQuick
import QtQuick.Controls
import Craftward.Components

Control {
    id: root

    property alias text: backend.text
    property alias readOnly: backend.readOnly
    property alias wordWrap: backend.wordWrap
    property real lineHeightScale: Typography.codeLineHeightScale

    implicitWidth: 320
    implicitHeight: 240
    focusPolicy: Qt.StrongFocus
    font: Typography.codeFont

    background: null

    contentItem: WindowContainer {
        activeFocusOnTab: true
        window: backend.window
    }

    ScintillaEditorBackend {
        id: backend

        fontFamily: root.font.family
        fontPointSize: root.font.pointSize > 0 ? root.font.pointSize : 13
        fontWeight: root.font.weight
        lineHeightScale: root.lineHeightScale
        foregroundColor: root.palette.text
        backgroundColor: root.palette.base
        selectionForegroundColor: root.palette.text
        selectionBackgroundColor: root.palette.highlight
    }
}
