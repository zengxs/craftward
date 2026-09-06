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
    property bool coordinatedSelection: false
    property var hoveredDescriptor: null
    property alias editor: edit
    property alias bridge: bridge
    readonly property var controlNodes: nodes.filter(node => node.kind === "control")
    readonly property string semanticSelection: {
        bridge.revision;
        return bridge.selectionText(edit.selectionStart, edit.selectionEnd);
    }
    signal interaction(string description)
    implicitHeight: edit.contentHeight

    function linkDescriptorAt(x, y, surface) {
        const point = edit.mapFromItem(root, x, y);
        const target = edit.linkAt(point.x, point.y);
        const fragment = bridge.linkFragmentAt(target, point.x, point.y);
        if (!fragment.nodeId)
            return null;
        const mapped = edit.mapToItem(surface, fragment.rect.x, fragment.rect.y);
        return {
            blockId,
            nodeId: fragment.nodeId,
            target,
            hint: fragment.hint,
            rect: Qt.rect(mapped.x, mapped.y, fragment.rect.width, fragment.rect.height)
        };
    }

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

    Timer {
        id: layoutRefresh
        interval: 0
        onTriggered: {
            root.hoveredDescriptor = null;
            root.layoutRevision++;
        }
    }
    Timer {
        id: metricsRefresh
        interval: 0
        onTriggered: bridge.refreshControlMetrics()
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
        onFontChanged: metricsRefresh.restart()
        readOnly: true
        selectByMouse: !root.coordinatedSelection
        selectByKeyboard: !root.coordinatedSelection
        activeFocusOnPress: !root.coordinatedSelection
        persistentSelection: true
        selectionColor: "#426da5"
        selectedTextColor: "white"
        Accessible.name: bridge.selectionText(0, length)
        onWidthChanged: layoutRefresh.restart()
        onContentHeightChanged: layoutRefresh.restart()
        onLinkActivated: link => root.interaction("Activated " + link)
        onLinkHovered: link => {
            if (link.length > 0)
                root.interaction("Hovered " + link);
        }
        Keys.onPressed: event => {
            if (!root.coordinatedSelection && event.matches(StandardKey.Copy)) {
                const copied = bridge.copySelection(selectionStart, selectionEnd);
                root.interaction("Copied: " + copied);
                event.accepted = true;
            }
        }
        HoverHandler {
            id: localHover
            enabled: !root.coordinatedSelection
            onPointChanged: {
                if (root.Window.window)
                    root.hoveredDescriptor = root.linkDescriptorAt(point.position.x, point.position.y, root.Window.window.contentItem);
            }
            onHoveredChanged: if (!hovered)
                root.hoveredDescriptor = null
        }
    }

    Loader {
        active: !root.coordinatedSelection && root.Window.window !== null
        sourceComponent: InlineHelpTip {
            surface: root.Window.window.contentItem
            descriptor: root.hoveredDescriptor
            requested: localHover.hovered && localHover.point.pressedButtons === Qt.NoButton && descriptor !== null
        }
    }

    InlineDocument {
        id: bridge
        document: edit.textDocument
        nodes: root.nodes
        dark: root.dark
        onChanged: layoutRefresh.restart()
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
