// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick

Item {
    id: root

    property var viewport: null
    property var surfaces: []
    property var coordinator: null
    property bool dragging: false

    function attach(surface) {
        if (surfaces.indexOf(surface) < 0)
            surfaces.push(surface);
    }

    function detach(surface) {
        const index = surfaces.indexOf(surface);
        if (index >= 0)
            surfaces.splice(index, 1);
    }

    function clear() {
        dragging = false;
        if (coordinator)
            coordinator.clear();
        coordinator = null;
    }

    function hitAt(x, y, nearest) {
        let best = null;
        let score = Infinity;
        for (const surface of surfaces) {
            if (!surface || !surface.visible || !surface.coordinator || (nearest && surface.coordinator !== coordinator))
                continue;
            const point = surface.mapFromItem(root, x, y);
            let excluded = false;
            for (const item of surface.selectionExclusions) {
                if (!item || !item.visible)
                    continue;
                const p = item.mapFromItem(root, x, y);
                if (item.contains(p)) {
                    excluded = true;
                    break;
                }
            }
            if (excluded && !nearest)
                continue;
            const dx = Math.max(0, -point.x, point.x - surface.width);
            const dy = Math.max(0, -point.y, point.y - surface.height);
            if (!nearest && (dx > 0 || dy > 0))
                continue;
            // Clip-aware hit testing also excludes pooled and horizontally hidden text.
            let visible = true;
            for (let parent = surface.parent; parent; parent = parent.parent) {
                if (!parent.visible) {
                    visible = false;
                    break;
                }
                if (!nearest && parent.clip && !parent.contains(parent.mapFromItem(root, x, y))) {
                    visible = false;
                    break;
                }
            }
            if (!visible)
                continue;
            const distance = dy * 100000 + dx;
            if (distance < score) {
                best = {
                    surface: surface,
                    x: point.x,
                    y: point.y
                };
                score = distance;
            }
        }
        return best;
    }

    function extendAt(x, y) {
        const hit = hitAt(x, Math.max(0, Math.min(height, y)), true);
        if (hit && coordinator)
            coordinator.extend(hit.surface.bridge.endpointAt(hit.surface.positionAt(hit.x, hit.y)));
    }

    MouseArea {
        id: pointer
        objectName: "markupSelectionPointer"
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton
        preventStealing: true
        property point pressPoint
        property bool moved: false
        property bool wordSelected: false
        property var pressLink: ""

        onPressed: mouse => {
            const hit = root.hitAt(mouse.x, mouse.y, false);
            if (!hit) {
                mouse.accepted = false;
                return;
            }
            const extending = Boolean(mouse.modifiers & Qt.ShiftModifier) && root.coordinator;
            if (!extending && root.coordinator !== hit.surface.coordinator) {
                root.clear();
                root.coordinator = hit.surface.coordinator;
            }
            forceActiveFocus();
            if (root.viewport)
                root.viewport.cancelFlick();
            root.dragging = true;
            pressPoint = Qt.point(mouse.x, mouse.y);
            moved = false;
            wordSelected = false;
            pressLink = hit.surface.linkAt(hit.x, hit.y);
            const endpoint = hit.surface.bridge.endpointAt(hit.surface.positionAt(hit.x, hit.y));
            if (extending)
                root.extendAt(mouse.x, mouse.y);
            else
                root.coordinator.begin(endpoint);
        }
        onPositionChanged: mouse => {
            if (pressed) {
                if (Math.hypot(mouse.x - pressPoint.x, mouse.y - pressPoint.y) >= Qt.styleHints.startDragDistance)
                    moved = true;
                root.extendAt(mouse.x, mouse.y);
            }
        }
        onCanceled: root.dragging = false
        onReleased: mouse => {
            if (!wordSelected)
                root.extendAt(mouse.x, mouse.y);
            if (!moved && root.coordinator && !root.coordinator.hasSelection && !(mouse.modifiers & Qt.ShiftModifier)) {
                const hit = root.hitAt(mouse.x, mouse.y, false);
                if (hit && pressLink && hit.surface.linkAt(hit.x, hit.y) === pressLink)
                    Qt.openUrlExternally(pressLink);
            }
            root.dragging = false;
        }
        onDoubleClicked: mouse => {
            const hit = root.hitAt(mouse.x, mouse.y, false);
            if (!hit || !root.coordinator || hit.surface.coordinator !== root.coordinator)
                return;
            wordSelected = true;
            const word = hit.surface.bridge.wordAt(hit.surface.positionAt(hit.x, hit.y));
            root.coordinator.begin(word.start);
            root.coordinator.extend(word.end);
        }
        onWheel: wheel => wheel.accepted = false
        Keys.onPressed: event => {
            if (!root.coordinator)
                return;
            if (event.matches(StandardKey.Copy))
                root.coordinator.copy();
            else if (event.matches(StandardKey.SelectAll))
                root.coordinator.selectAll();
            else if (event.key === Qt.Key_Escape)
                root.clear();
            else
                return;
            event.accepted = true;
        }
    }

    Connections {
        target: root.viewport
        function onContentYChanged() {
            if (root.dragging)
                refresh.restart();
        }
    }
    Timer {
        id: refresh
        interval: 0
        onTriggered: if (root.dragging)
            root.extendAt(pointer.mouseX, pointer.mouseY)
    }
    Timer {
        interval: 16
        repeat: true
        running: root.viewport && root.dragging && pointer.moved && (pointer.mouseY < 30 || pointer.mouseY > root.height - 30)
        onTriggered: {
            const delta = pointer.mouseY < 30 ? -Math.min(24, (30 - pointer.mouseY) / 2) : Math.min(24, (pointer.mouseY - root.height + 30) / 2);
            const minimum = root.viewport.originY;
            const maximum = Math.max(minimum, minimum + root.viewport.contentHeight - root.viewport.height);
            root.viewport.contentY = Math.max(minimum, Math.min(maximum, root.viewport.contentY + delta));
            root.extendAt(pointer.mouseX, pointer.mouseY);
        }
    }
}
