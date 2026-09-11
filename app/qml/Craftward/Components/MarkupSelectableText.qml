// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import Craftward.Design
import Craftward.Markup
import Craftward.Components as Components

TextEdit {
    id: root

    property var surface: null
    property var coordinator: null
    property var selectionHost: null
    property var selectionExclusions: []
    property bool preserveSelectionColors: false
    property font codeFont: font
    property real lineHeightScale: Components.Typography.proseLineHeightScale
    property real listIndentWidth: MarkupListMetrics.indentWidth(surface, font)
    property color linkColor: "blue"
    property color codeBackground: Theme.inlineCodeSurface
    property real codeVerticalPadding: 1
    property real codeRadius: 4
    property var registeredHost: null
    readonly property alias bridge: adapter
    property real firstLineBaseline: baselineOffset
    property var linkHandler: null

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
    topPadding: adapter.hasInlineCode ? codeVerticalPadding : 0
    bottomPadding: adapter.hasInlineCode ? codeVerticalPadding : 0

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

    function activateLink(target) {
        if (linkHandler)
            linkHandler(target);
        else
            Qt.openUrlExternally(target);
    }

    onSelectionHostChanged: registerHost()
    onCoordinatorChanged: selectionRefresh.restart()
    onContentHeightChanged: selectionRefresh.restart()
    onWidthChanged: selectionRefresh.restart()
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

    MarkupListMarkers {
        objectName: "markupListMarkers"
        anchors.fill: parent
        textEdit: root
    }

    MarkupCodeBackground {
        id: codeBackgrounds
        objectName: "markupCodeBackgrounds"
        anchors.fill: parent
        z: -2
        textEdit: root
        verticalPadding: root.codeVerticalPadding

        Repeater {
            model: codeBackgrounds.rectangles
            Rectangle {
                required property rect modelData
                objectName: "markupCodeBackground"
                x: modelData.x
                y: modelData.y
                width: modelData.width
                height: modelData.height
                radius: root.codeRadius
                color: root.codeBackground
            }
        }
    }

    Loader {
        anchors.fill: parent
        z: -1
        active: root.preserveSelectionColors || root.selectionStart < root.selectionEnd
        sourceComponent: MarkupSelectionBackground {
            objectName: "markupSelectionBackground"
            textEdit: root
            nativeSelection: !root.preserveSelectionColors
            // Each source line in a code block is a separate QTextBlock.
            joinParagraphs: root.preserveSelectionColors
            color: root.preserveSelectionColors ? (Theme.dark ? TailwindColors.zinc700 : Theme.textSelectionBackground) : root.selectionColor
        }
    }

    MarkupTextDocument {
        id: adapter
        objectName: "markupNativeAdapter"
        textDocument: root.textDocument
        surface: root.surface
        font: root.font
        codeFont: root.codeFont
        lineHeightScale: root.lineHeightScale
        listIndentWidth: root.listIndentWidth
        textColor: root.color
        linkColor: root.linkColor
        // Annotation labels use the document's native character background.
        annotationBackground: Theme.dark ? TailwindColors.zinc800 : TailwindColors.zinc100
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
        onTriggered: {
            root.applySelection();
            root.firstLineBaseline = root.positionToRectangle(0).y + adapter.firstLineAscent();
        }
    }
}
