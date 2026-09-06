// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick

InlineBlock {
    id: root
    required property var host
    required property var coordinator
    property int instanceNumber: 0
    coordinatedSelection: true
    onLayoutRevisionChanged: if (host)
        host.invalidateHover(blockId)

    function applySelection() {
        const range = coordinator.range(nodes);
        editor.select(range.start, range.end);
    }

    function endpointAt(x, y) {
        const point = editor.mapFromItem(root, x, y);
        return bridge.endpointAt(editor.positionAt(point.x, point.y));
    }

    Component.onCompleted: {
        instanceNumber = host.attach(root);
        selectionRefresh.restart();
    }
    Component.onDestruction: host.detach(root)
    Timer {
        id: selectionRefresh
        interval: 0
        onTriggered: root.applySelection()
    }
    Connections {
        target: root.coordinator
        function onChanged() {
            root.applySelection();
        }
    }
    Connections {
        target: root.bridge
        function onChanged() {
            selectionRefresh.restart();
        }
    }
}
