// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls

// Exercise the actual popup at the table link, without synthesizing mouse input.
Timer {
    required property var host
    interval: 20
    repeat: true
    property int ticks: 0
    property int stage: 0
    property bool positioned: false
    property rect anchor
    property var checks: []
    property var placements: []

    function showAt(id, firstPosition, lastPosition) {
        const block = host.findBlock(id);
        if (!block)
            return false;
        const first = block.editor.positionToRectangle(firstPosition);
        const last = block.editor.positionToRectangle(lastPosition);
        const point = block.editor.mapToItem(host.contentItem, first.x, first.y);
        anchor = Qt.rect(point.x, point.y, last.x - first.x, first.height);
        host.previewTooltip(block, firstPosition + 1);
        return true;
    }
    function checkPlacement(label) {
        const tip = host.hoverPopup;
        const point = tip.contentItem.mapToItem(host.contentItem, -tip.leftPadding, -tip.topPadding);
        const dx = Math.max(anchor.x - point.x - tip.width, point.x - anchor.x - anchor.width, 0);
        const dy = Math.max(anchor.y - point.y - tip.height, point.y - anchor.y - anchor.height, 0);
        const gap = Math.hypot(dx, dy);
        checks.push({
            name: label + " stays within 12 px of its link",
            passed: gap <= 12 && gap > 0
        });
        checks.push({
            name: label + " stays within the viewport",
            passed: point.x >= tip.area.x && point.y >= tip.area.y && point.x + tip.width <= tip.area.x + tip.area.width && point.y + tip.height <= tip.area.y + tip.area.height
        });
        placements.push({
            label,
            anchor: {
                x: anchor.x,
                y: anchor.y,
                width: anchor.width,
                height: anchor.height
            },
            tooltip: {
                x: point.x,
                y: point.y,
                width: tip.width,
                height: tip.height
            },
            gap
        });
        return point;
    }
    onTriggered: {
        if (++ticks > 200) {
            stop();
            prototypeCapture.reportSelection({
                checks: [
                    {
                        name: "tooltip appeared",
                        passed: false
                    }
                ]
            });
            return;
        }
        if (stage === 0) {
            if (!positioned) {
                host.viewport.positionViewAtIndex(3, ListView.Contain);
                positioned = true;
                return;
            }
            if (!showAt("cell:4", 0, 5))
                return;
            stage = 1;
        } else if (stage === 1 && host.hoverPopup.opened) {
            checkPlacement("table status tooltip");
            showAt("cell:5", 10, 13);
            stage = 2;
        } else if (stage === 2 && host.hoverPopup.opened) {
            checkPlacement("table annotation tooltip");
            const area = host.hoverPopup.area;
            host.hoverPopup.area = Qt.rect(area.x, area.y, area.width, anchor.y + anchor.height + 2 - area.y);
            stage = 3;
        } else if (stage === 3 && host.hoverPopup.opened) {
            const point = checkPlacement("tooltip near the bottom edge");
            checks.push({
                name: "tooltip flips above the source",
                passed: point.y + host.hoverPopup.height <= anchor.y
            });
            host.jump(20);
            stage = 4;
        } else if (stage === 4 && !host.findBlock("cell:5")) {
            checks.push({
                name: "scroll and source destruction dismiss the tooltip",
                passed: !host.hoverPopup.visible && host.hoverPopup.descriptor === null
            });
            stop();
            prototypeCapture.reportSelection({
                checks,
                placements,
                dismissed: {
                    visible: host.hoverPopup.visible,
                    opened: host.hoverPopup.opened,
                    descriptor: host.hoverPopup.descriptor
                }
            });
        }
    }
}
