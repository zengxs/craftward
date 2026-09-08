// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import Craftward.Design
import Craftward.Markup

TextEdit {
    id: root

    property var surface: null
    property var coordinator: null
    property var selectionHost: null
    property var selectionExclusions: []
    property bool preserveSelectionColors: false
    property font codeFont: font
    property color linkColor: "blue"
    property color codeBackground: Theme.dark ? TailwindColors.zinc800 : TailwindColors.zinc100
    property var registeredHost: null
    readonly property alias bridge: adapter

    objectName: "markupProseText"
    readOnly: true
    selectByMouse: !coordinator
    selectByKeyboard: !coordinator
    activeFocusOnPress: !coordinator
    persistentSelection: true
    selectedTextColor: preserveSelectionColors ? "transparent" : Theme.textSelectionForeground
    selectionColor: preserveSelectionColors ? "transparent" : Theme.textSelectionBackground
    wrapMode: TextEdit.Wrap
    textFormat: TextEdit.RichText

    function registerHost() {
        if (registeredHost === selectionHost)
            return;
        if (registeredHost && typeof registeredHost.detach === "function")
            registeredHost.detach(root);
        registeredHost = selectionHost;
        if (registeredHost)
            registeredHost.attach(root);
    }

    function applySelection() {
        if (!coordinator || !surface)
            return;
        const range = coordinator.range(surface);
        select(adapter.documentPosition(range.start), adapter.documentPosition(range.end));
    }

    onSelectionHostChanged: registerHost()
    onCoordinatorChanged: selectionRefresh.restart()
    Component.onCompleted: {
        registerHost();
        selectionRefresh.restart();
    }
    Component.onDestruction: if (registeredHost && typeof registeredHost.detach === "function")
        registeredHost.detach(root)

    HoverHandler {
        blocking: false
        cursorShape: root.linkAt(point.position.x, point.position.y) ? Qt.PointingHandCursor : Qt.IBeamCursor
    }

    Loader {
        anchors.fill: parent
        z: -1
        active: root.preserveSelectionColors
        sourceComponent: MarkupSelectionBackground {
            objectName: "markupSelectionBackground"
            textEdit: root
            color: Theme.dark ? TailwindColors.zinc700 : Theme.textSelectionBackground
        }
    }

    MarkupTextDocument {
        id: adapter
        objectName: "markupNativeAdapter"
        textDocument: root.textDocument
        surface: root.surface
        font: root.font
        codeFont: root.codeFont
        textColor: root.color
        linkColor: root.linkColor
        codeBackground: root.codeBackground
        onRendered: selectionRefresh.restart()
    }

    Connections {
        target: root.coordinator || null
        function onChanged() {
            root.applySelection();
        }
    }

    Timer {
        id: selectionRefresh
        interval: 0
        onTriggered: root.applySelection()
    }
}
