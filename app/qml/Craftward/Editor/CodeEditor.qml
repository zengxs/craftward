import QtQuick
import QtQuick.Controls
import Craftward.Components

Control {
    id: root

    property alias text: backend.text
    property alias readOnly: backend.readOnly
    property alias wordWrap: backend.wordWrap
    property alias showLineNumbers: backend.showLineNumbers
    property color lineNumberColor: Qt.tint(palette.base, palette.placeholderText)
    property real lineHeightScale: Typography.codeLineHeightScale
    readonly property alias canUndo: backend.canUndo
    readonly property alias canRedo: backend.canRedo

    function undo() {
        backend.undo();
    }
    function redo() {
        backend.redo();
    }
    function cut() {
        backend.cut();
    }
    function copy() {
        backend.copy();
    }
    function paste() {
        backend.paste();
    }
    function selectAll() {
        backend.selectAll();
    }

    function revealLocation(start, end) {
        backend.revealLocation(start, end);
    }

    implicitWidth: 320
    implicitHeight: 240
    focusPolicy: Qt.StrongFocus
    font: Typography.codeFont

    background: null

    onActiveFocusChanged: {
        if (activeFocus)
            backend.forceActiveFocus();
    }

    contentItem: ScintillaEditorBackend {
        id: backend

        activeFocusOnTab: true
        fontFamily: root.font.family
        fontPointSize: root.font.pointSize > 0 ? root.font.pointSize : 13
        fontWeight: root.font.weight
        lineHeightScale: root.lineHeightScale
        foregroundColor: root.palette.text
        backgroundColor: root.palette.base
        lineNumberColor: root.lineNumberColor
        selectionForegroundColor: root.palette.text
        selectionBackgroundColor: root.palette.highlight

        onContextMenuRequested: function (position, entries) {
            const point = backend.mapToItem(root, position.x, position.y);
            editorMenu.popup(point.x, point.y);
        }
    }

    HoverHandler {
        id: editorHover
    }

    Timer {
        id: scrollActivity
        interval: 650
    }

    QtObject {
        readonly property real verticalPosition: backend.verticalPosition
        readonly property real horizontalPosition: backend.horizontalPosition
        onVerticalPositionChanged: scrollActivity.restart()
        onHorizontalPositionChanged: scrollActivity.restart()
    }

    OverlayScrollBar {
        id: verticalBar
        z: 1
        anchors.top: parent.top
        anchors.bottom: horizontalBar.visible ? horizontalBar.top : parent.bottom
        anchors.right: parent.right
        orientation: Qt.Vertical
        size: backend.verticalSize
        active: hovered || pressed || editorHover.hovered || backend.activeFocus || scrollActivity.running
        onPositionChanged: {
            if (pressed)
                backend.verticalPosition = position;
        }
        Binding {
            target: verticalBar
            property: "position"
            value: backend.verticalPosition
            when: !verticalBar.pressed
        }
    }

    OverlayScrollBar {
        id: horizontalBar
        z: 1
        anchors.left: parent.left
        anchors.right: verticalBar.visible ? verticalBar.left : parent.right
        anchors.bottom: parent.bottom
        orientation: Qt.Horizontal
        size: backend.horizontalSize
        active: hovered || pressed || editorHover.hovered || backend.activeFocus || scrollActivity.running
        onPositionChanged: {
            if (pressed)
                backend.horizontalPosition = position;
        }
        Binding {
            target: horizontalBar
            property: "position"
            value: backend.horizontalPosition
            when: !horizontalBar.pressed
        }
    }

    Menu {
        id: editorMenu
        popupType: Popup.Item

        MenuItem {
            text: /*% "Undo" */ qsTrId("craftward.editor.undo")
            enabled: !root.readOnly && backend.canUndo
            onTriggered: backend.undo()
        }
        MenuItem {
            text: /*% "Redo" */ qsTrId("craftward.editor.redo")
            enabled: !root.readOnly && backend.canRedo
            onTriggered: backend.redo()
        }
        MenuSeparator {}
        MenuItem {
            text: /*% "Cut" */ qsTrId("craftward.editor.cut")
            enabled: !root.readOnly && backend.hasSelection
            onTriggered: backend.cut()
        }
        MenuItem {
            text: /*% "Copy" */ qsTrId("craftward.editor.copy")
            enabled: backend.hasSelection
            onTriggered: backend.copy()
        }
        MenuItem {
            text: /*% "Paste" */ qsTrId("craftward.editor.paste")
            enabled: backend.canPaste
            onTriggered: backend.paste()
        }
        MenuItem {
            text: /*% "Delete" */ qsTrId("craftward.editor.delete")
            enabled: !root.readOnly && backend.hasSelection
            onTriggered: backend.deleteSelection()
        }
        MenuSeparator {}
        MenuItem {
            text: /*% "Select All" */ qsTrId("craftward.editor.select_all")
            onTriggered: backend.selectAll()
        }
    }
}
