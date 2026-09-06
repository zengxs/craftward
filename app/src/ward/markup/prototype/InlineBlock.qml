// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Craftward.InlinePrototype

Item {
    id: root
    required property string blockId
    required property var nodes
    property bool dark: false
    property int pixelSize: 17
    property int layoutRevision: 0
    property int overlayCreations: 0
    property alias editor: edit
    property alias bridge: bridge
    readonly property var controlNodes: nodes.filter(node => node.kind === "control")
    readonly property string semanticSelection: {
        bridge.revision;
        return bridge.selectionText(edit.selectionStart, edit.selectionEnd);
    }
    signal interaction(string description)
    implicitHeight: edit.contentHeight

    function snapshot() {
        const state = bridge.snapshot();
        state.blockId = blockId;
        state.width = width;
        state.height = height;
        state.overlayCreations = overlayCreations;
        state.selectionStart = edit.selectionStart;
        state.selectionEnd = edit.selectionEnd;
        state.selectionText = semanticSelection;
        state.controls = controlNodes.map((node, index) => {
            const box = bridge.controlRect(node.id);
            const item = controlRepeater.itemAt(index);
            const baseline = bridge.baselineAt(bridge.nodeState(node.id).start);
            return {
                id: node.id,
                textBaseline: baseline,
                controlBaseline: item ? item.y + item.baselineOffset : NaN,
                baselineError: item ? item.y + item.baselineOffset - baseline : NaN,
                rect: {
                    x: box.x,
                    y: box.y,
                    width: box.width,
                    height: box.height
                }
            };
        });
        return state;
    }

    TextEdit {
        id: edit
        width: root.width
        height: contentHeight
        padding: 0
        textMargin: 0
        textFormat: TextEdit.RichText
        wrapMode: TextEdit.Wrap
        horizontalAlignment: TextEdit.AlignLeft
        color: root.dark ? "#e7e9ee" : "#20252e"
        font.pixelSize: root.pixelSize
        onFontChanged: Qt.callLater(() => bridge.refreshControlMetrics())
        readOnly: true
        selectByMouse: true
        selectByKeyboard: true
        persistentSelection: true
        selectionColor: "#426da5"
        selectedTextColor: "white"
        Accessible.name: bridge.selectionText(0, length)
        onWidthChanged: Qt.callLater(() => root.layoutRevision++)
        onContentHeightChanged: Qt.callLater(() => root.layoutRevision++)
        onLinkActivated: link => root.interaction("Activated " + link)
        onLinkHovered: link => {
            if (link.length > 0)
                root.interaction("Hovered " + link);
        }
        Keys.onPressed: event => {
            if (event.matches(StandardKey.Copy)) {
                const copied = bridge.copySelection(selectionStart, selectionEnd);
                root.interaction("Copied: " + copied);
                event.accepted = true;
            }
        }
        ToolTip.visible: hoveredLink.length > 0
        ToolTip.text: hoveredLink.startsWith("codex-annotation:") ? "Annotation 4: review this statement" : hoveredLink
    }

    InlineDocument {
        id: bridge
        document: edit.textDocument
        nodes: root.nodes
        dark: root.dark
        onChanged: Qt.callLater(() => root.layoutRevision++)
    }

    Repeater {
        id: controlRepeater
        model: root.controlNodes
        delegate: Button {
            id: control
            required property var modelData
            readonly property var state: {
                bridge.revision;
                return bridge.nodeState(modelData.id);
            }
            readonly property rect box: {
                root.layoutRevision;
                bridge.revision;
                return bridge.controlRect(modelData.id);
            }
            x: box.x
            y: box.y
            width: box.width
            height: box.height
            padding: 0
            leftPadding: 14
            rightPadding: 14
            text: state.text || ""
            font: edit.font
            checkable: true
            checked: state.expanded || false
            Accessible.name: "Inline review control: " + text
            Component.onCompleted: root.overlayCreations++
            onClicked: {
                bridge.toggleControl(modelData.id);
                root.interaction("Toggled " + modelData.id);
            }
        }
    }
}
