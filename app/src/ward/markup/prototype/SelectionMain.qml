// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.InlinePrototype
import "SelectionFixtures.js" as Fixtures

ApplicationWindow {
    id: window
    width: probeWidth || 1180
    height: 900
    minimumWidth: 960
    minimumHeight: 720
    visible: true
    title: "Craftward Message Selection Prototype"
    color: "#f4f6f9"
    property var liveBlocks: []
    property int created: 0
    property int destroyed: 0
    property int peakLive: 0
    property string lastAction: "Drag through the text, code, and table. Copy with the platform shortcut."
    property alias coordinator: selection
    property alias viewport: timeline
    property alias hoverPopup: inlineTip

    function previewTooltip(block, position) {
        const cursor = block.editor.positionToRectangle(position);
        const point = block.editor.mapToItem(pointer, cursor.x + 1, cursor.y + cursor.height / 2);
        updateHover(point.x, point.y);
        inlineTip.show(pointer.hoveredDescriptor.hint, -1);
    }

    function attach(block) {
        liveBlocks = liveBlocks.concat([block]);
        peakLive = Math.max(peakLive, liveBlocks.length);
        return ++created;
    }
    function detach(block) {
        invalidateHover(block.blockId);
        liveBlocks = liveBlocks.filter(item => item !== block);
        ++destroyed;
    }
    function findBlock(id) {
        return liveBlocks.find(block => block.blockId === id) || null;
    }
    function snapshot() {
        return {
            selection: selection.state(),
            selectedText: selection.text(),
            lastAction,
            created,
            destroyed,
            peakLive,
            liveDocuments: liveBlocks.length,
            totalSegments: selection.segments.length,
            contentY: timeline.contentY,
            blocks: liveBlocks.map(block => ({
                        id: block.blockId,
                        instance: block.instanceNumber,
                        start: block.editor.selectionStart,
                        end: block.editor.selectionEnd,
                        text: block.editor.selectedText
                    }))
        };
    }
    function exampleSelection() {
        selection.begin({
            nodeId: "intro:code",
            offset: 0
        });
        selection.extend({
            nodeId: "cell:8:text",
            offset: "第二行 👋".length
        });
        timeline.positionViewAtIndex(0, ListView.Beginning);
        pointer.forceActiveFocus();
        lastAction = "Selected from inline code through the last table cell.";
    }
    function jump(index) {
        timeline.positionViewAtIndex(index, ListView.Beginning);
        pointer.forceActiveFocus();
    }
    function copySelection() {
        const text = selection.copy();
        lastAction = "Copied " + text.length + " UTF-16 units from the message's semantic data.";
    }

    // Only materialized text items participate in geometry lookup.
    function hitAt(x, y) {
        let best = null;
        let vertical = Infinity;
        let horizontal = Infinity;
        for (const block of liveBlocks) {
            const point = block.mapToItem(pointer, 0, 0);
            const dy = Math.max(point.y - y, y - point.y - block.height, 0);
            const dx = Math.max(point.x - x, x - point.x - block.width, 0);
            if (dy < vertical || (dy === vertical && dx < horizontal)) {
                best = {
                    block,
                    x: x - point.x,
                    y: y - point.y
                };
                vertical = dy;
                horizontal = dx;
            }
        }
        return best;
    }
    function extendAt(x, y) {
        const hit = hitAt(x, Math.max(0, Math.min(pointer.height, y)));
        if (hit)
            selection.extend(hit.block.endpointAt(hit.x, hit.y));
    }
    function targetAt(hit) {
        if (!hit || hit.x < 0 || hit.y < 0 || hit.x > hit.block.width || hit.y > hit.block.height)
            return "";
        const point = hit.block.editor.mapFromItem(hit.block, hit.x, hit.y);
        return hit.block.editor.linkAt(point.x, point.y);
    }
    function updateHover(x, y) {
        const hit = hitAt(x, y);
        pointer.hoveredDescriptor = targetAt(hit) ? hit.block.linkDescriptorAt(hit.x, hit.y, window.contentItem) : null;
    }
    function invalidateHover(blockId) {
        if (pointer && pointer.hoveredDescriptor && (!blockId || pointer.hoveredDescriptor.blockId === blockId))
            pointer.hoveredDescriptor = null;
    }

    InlineHelpTip {
        id: inlineTip
        surface: window.contentItem
        descriptor: pointer.hoveredDescriptor
        requested: pointer.containsMouse && descriptor !== null && !pointer.pressed
        area: {
            const point = timeline.mapToItem(window.contentItem, 0, 0);
            return Qt.rect(point.x, point.y, timeline.width - 18, timeline.height);
        }
    }

    MessageSelection {
        id: selection
        segments: Fixtures.segments()
    }
    SelectionProbe {
        host: window
        running: selectionProbeMode
    }
    TooltipProbe {
        host: window
        running: tooltipProbeMode
    }

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: 22
        spacing: 12
        Label {
            text: "One message / continuous selection"
            font.pixelSize: 23
            font.bold: true
        }
        Label {
            text: "Can selection cross paragraphs, code, and tables while offscreen text items are destroyed?"
            color: "#657080"
        }
        RowLayout {
            Button {
                text: "Select example"
                onClicked: window.exampleSelection()
            }
            Button {
                text: "Copy selection"
                enabled: selection.hasSelection
                onClicked: window.copySelection()
            }
            Button {
                text: "Clear"
                onClicked: selection.clear()
            }
            Label {
                text: "Font"
            }
            Slider {
                id: fontSize
                from: 14
                to: 24
                stepSize: 1
                value: probeFontSize || 17
                Layout.preferredWidth: 100
            }
            Label {
                text: fontSize.value + " px"
            }
            Item {
                Layout.fillWidth: true
            }
            Button {
                text: "Capture state"
                onClicked: prototypeCapture.save(window, window.snapshot())
            }
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 20
            Item {
                Layout.fillWidth: true
                Layout.fillHeight: true
                ListView {
                    id: timeline
                    anchors.fill: parent
                    clip: true
                    spacing: 16
                    cacheBuffer: 100
                    reuseItems: false
                    currentIndex: -1
                    keyNavigationEnabled: false
                    boundsBehavior: Flickable.StopAtBounds
                    interactive: !pointer.pressed
                    model: selection.segments
                    delegate: SelectionSegment {
                        required property var modelData
                        width: timeline.width - 18
                        segment: modelData
                        host: window
                        coordinator: selection
                        pixelSize: fontSize.value
                    }
                    ScrollBar.vertical: ScrollBar {
                        policy: ScrollBar.AlwaysOn
                    }
                    onContentYChanged: {
                        window.invalidateHover();
                        if (pointer && pointer.pressed)
                            scrollSelectionRefresh.restart();
                    }
                    onWidthChanged: window.invalidateHover()
                    onHeightChanged: window.invalidateHover()
                }
                MouseArea {
                    id: pointer
                    anchors.fill: parent
                    anchors.rightMargin: 18
                    acceptedButtons: Qt.LeftButton
                    preventStealing: true
                    property bool selectedWord: false
                    property bool dragged: false
                    property point pressPoint
                    property var hoveredDescriptor: null
                    hoverEnabled: true
                    cursorShape: hoveredDescriptor ? Qt.PointingHandCursor : Qt.IBeamCursor
                    onPressed: mouse => {
                        selectedWord = false;
                        dragged = false;
                        pressPoint = Qt.point(mouse.x, mouse.y);
                        forceActiveFocus();
                        const hit = window.hitAt(mouse.x, mouse.y);
                        if (!hit)
                            return;
                        const endpoint = hit.block.endpointAt(hit.x, hit.y);
                        if (mouse.modifiers & Qt.ShiftModifier)
                            selection.extend(endpoint);
                        else
                            selection.begin(endpoint);
                    }
                    onPositionChanged: mouse => {
                        window.updateHover(mouse.x, mouse.y);
                        if (pressed) {
                            if (Math.hypot(mouse.x - pressPoint.x, mouse.y - pressPoint.y) >= Qt.styleHints.startDragDistance)
                                dragged = true;
                            window.extendAt(mouse.x, mouse.y);
                        }
                    }
                    onReleased: mouse => {
                        if (!selectedWord)
                            window.extendAt(mouse.x, mouse.y);
                        if (!dragged && !selection.hasSelection && !(mouse.modifiers & Qt.ShiftModifier)) {
                            const target = window.targetAt(window.hitAt(mouse.x, mouse.y));
                            if (target)
                                window.lastAction = "Activated " + target;
                        }
                    }
                    onDoubleClicked: mouse => {
                        const hit = window.hitAt(mouse.x, mouse.y);
                        if (!hit)
                            return;
                        selectedWord = true;
                        const word = hit.block.bridge.wordAt(hit.block.editor.positionAt(hit.x, hit.y));
                        selection.begin(word.start);
                        selection.extend(word.end);
                    }
                    onWheel: wheel => wheel.accepted = false
                    onExited: hoveredDescriptor = null
                    Keys.onPressed: event => {
                        if (event.matches(StandardKey.Copy)) {
                            window.copySelection();
                        } else if (event.matches(StandardKey.SelectAll)) {
                            selection.selectMessage();
                        } else if (event.key === Qt.Key_Escape) {
                            selection.clear();
                        } else {
                            return;
                        }
                        event.accepted = true;
                    }
                }
                Timer {
                    id: scrollSelectionRefresh
                    interval: 0
                    onTriggered: if (pointer.pressed)
                        window.extendAt(pointer.mouseX, pointer.mouseY)
                }
                Timer {
                    interval: 16
                    repeat: true
                    running: pointer.pressed && (pointer.mouseY < 30 || pointer.mouseY > pointer.height - 30)
                    onTriggered: {
                        if (selection.state().clampedToMessage)
                            return;
                        const delta = pointer.mouseY < 30 ? -Math.min(24, (30 - pointer.mouseY) / 2) : Math.min(24, (pointer.mouseY - pointer.height + 30) / 2);
                        const min = timeline.originY;
                        const max = Math.max(min, timeline.originY + timeline.contentHeight - timeline.height);
                        timeline.contentY = Math.max(min, Math.min(max, timeline.contentY + delta));
                        window.extendAt(pointer.mouseX, pointer.mouseY);
                    }
                }
            }
            ScrollView {
                Layout.preferredWidth: 300
                Layout.fillHeight: true
                contentWidth: availableWidth
                Column {
                    width: parent.width
                    spacing: 14
                    Label {
                        text: "WALKTHROUGH"
                        font.bold: true
                        color: "#657080"
                    }
                    Label {
                        width: parent.width
                        wrapMode: Text.Wrap
                        text: "1. Select across content.\n2. Jump away so selected items are destroyed.\n3. Return and check the highlight and copied text."
                    }
                    Button {
                        text: "Jump away"
                        onClicked: window.jump(20)
                    }
                    Button {
                        text: "Show message boundary"
                        onClicked: window.jump(28)
                    }
                    Button {
                        text: "Back to start"
                        onClicked: window.jump(0)
                    }
                    Label {
                        text: "Live text documents: " + window.liveBlocks.length + "\nCreated: " + window.created + " · destroyed: " + window.destroyed
                        font.bold: true
                    }
                    Label {
                        width: parent.width
                        wrapMode: Text.Wrap
                        text: window.lastAction
                    }
                    Label {
                        text: "SELECTED TEXT / FIRST 600 UNITS"
                        font.bold: true
                        color: "#657080"
                    }
                    TextArea {
                        width: parent.width
                        readOnly: true
                        wrapMode: TextEdit.Wrap
                        text: selection.preview || "No selection"
                        font.pixelSize: 13
                    }
                    Label {
                        text: "LOGICAL ENDPOINTS"
                        font.bold: true
                        color: "#657080"
                    }
                    TextArea {
                        id: stateText
                        width: parent.width
                        readOnly: true
                        wrapMode: TextEdit.Wrap
                        font.pixelSize: 12
                        text: JSON.stringify(selection.state(), null, 2)
                        Connections {
                            target: selection
                            function onChanged() {
                                stateText.text = JSON.stringify(selection.state(), null, 2);
                            }
                        }
                    }
                }
            }
        }
        Label {
            text: "Native Qt experiment · one message per selection · fixture content · no production timeline integration"
            color: "#657080"
            font.pixelSize: 11
        }
    }
}
