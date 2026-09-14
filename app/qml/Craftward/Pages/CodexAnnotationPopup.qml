// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl
import QtQuick.Layouts
import QtQml.Models
import Craftward.Design
import Craftward.Components

Popup {
    id: root

    property var resolveAnnotations: null
    property var candidates: []
    property string sourceEntryId: ""
    property double referenceIndex: 0
    property string referenceKey: ""
    property Item anchorItem: null
    property var anchorChain: []
    property rect anchorRect
    property bool pinned: false
    property int hoverDelay: 350
    property int leaveDelay: 200
    readonly property bool hasAmbiguousReference: referenceIndex > 0 && candidates.length > 1
    readonly property var anchorWindow: anchorItem ? anchorItem.Window.window : null
    readonly property real preferredWidth: Math.min(Math.ceil(details.implicitWidth + padding * 2), 520)
    readonly property real preferredHeight: Math.min(details.implicitHeight + padding * 2, 480)
    readonly property color hintColor: Theme.dark ? TailwindColors.zinc400 : TailwindColors.zinc500
    readonly property int hintPixelSize: Math.max(10, root.font.pixelSize - 1)
    readonly property real numberColumnWidth: Math.max(18, Math.ceil(numberMeasure.implicitWidth))
    // Native window dimensions update asynchronously; layout uses the requested width.
    property real fittedWidth: preferredWidth
    readonly property string heading: referenceIndex === 0 ? /*% "%n annotation(s)" */ qsTrId("craftward.codex.annotations.count", candidates.length) : hasAmbiguousReference ? /*% "Annotation %1 · %2 possible sources" */ qsTrId("craftward.codex.annotations.ambiguous").arg(referenceIndex).arg(candidates.length) : /*% "Annotation %1" */ qsTrId("craftward.codex.annotations.number").arg(referenceIndex)

    objectName: "codexAnnotationPopup"
    parent: Overlay.overlay
    width: fittedWidth
    height: preferredHeight
    padding: 18
    focus: pinned
    modal: false
    popupType: Popup.Window
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    background: Rectangle {
        radius: 16
        color: Theme.dark ? TailwindColors.zinc900 : TailwindColors.zinc50
        border.width: 0
    }

    function dismiss() {
        hoverTimer.stop();
        leaveTimer.stop();
        placementTimer.stop();
        close();
        anchorItem = null;
        anchorChain = [];
        pinned = false;
    }

    function refresh() {
        const next = resolveAnnotations ? resolveAnnotations(sourceEntryId, referenceIndex) : [];
        // Streaming updates elsewhere must not recreate selected detail text.
        if (JSON.stringify(next) !== JSON.stringify(candidates))
            candidates = next;
    }

    function place() {
        if (!parent || !anchorItem)
            return;
        let geometry = PopupPositioner.place(anchorItem, anchorRect, parent, Qt.size(preferredWidth, preferredHeight));
        if (geometry.width <= 0 || geometry.height <= 0)
            return;
        fittedWidth = geometry.width;
        // Wrapping at the fitted width determines the height and opening direction.
        details.ensurePolished();
        geometry = PopupPositioner.place(anchorItem, anchorRect, parent, Qt.size(fittedWidth, preferredHeight));
        height = geometry.height;
        x = geometry.x;
        y = geometry.y;
    }

    function showDetails() {
        if (!anchorItem || !anchorItem.visible)
            return;
        refresh();
        // Resolve the content height before the native window accepts pointer events.
        details.ensurePolished();
        detailsScroll.contentItem.contentY = 0;
        place();
        open();
    }

    // All anchors use the same popup; invalidation also accepts a recycled row.
    function handle(entryId, action, item, hit) {
        if (action === "invalidate") {
            for (let ancestor = anchorItem; ancestor; ancestor = ancestor.parent) {
                if (ancestor === item) {
                    dismiss();
                    return;
                }
            }
            return;
        }
        if (action === "leave") {
            if (item === anchorItem) {
                hoverTimer.stop();
                if (!pinned)
                    leaveTimer.restart();
            }
            return;
        }
        if (pinned && action !== "activate")
            return;
        const same = anchorItem === item && sourceEntryId === entryId && referenceKey === hit.key && referenceIndex === hit.index;
        leaveTimer.stop();
        sourceEntryId = entryId;
        referenceIndex = hit.index;
        referenceKey = hit.key;
        if (anchorItem !== item || (!visible && !hoverTimer.running)) {
            const chain = [];
            for (let ancestor = item; ancestor; ancestor = ancestor.parent)
                chain.push(ancestor);
            anchorChain = chain;
        }
        anchorItem = item;
        anchorRect = hit.rect;
        if (action === "activate") {
            hoverTimer.stop();
            pinned = true;
            showDetails();
        } else if (!same || (!visible && !hoverTimer.running)) {
            close();
            hoverTimer.restart();
        }
    }

    onAnchorItemChanged: if (!anchorItem)
        dismiss()
    onClosed: {
        leaveTimer.stop();
        pinned = false;
    }
    onPreferredHeightChanged: if (visible)
        placementTimer.restart()
    onPreferredWidthChanged: if (visible)
        placementTimer.restart()
    onAboutToShow: {
        const popupWindow = contentItem.Window.window;
        // Qt disables system shadows for QML popups; this panel uses the native shadow.
        if (popupWindow && popupWindow !== anchorWindow)
            popupWindow.flags &= ~Qt.NoDropShadowWindowHint;
        place();
    }
    onOpened: placementTimer.restart()

    contentItem: Item {
        Text {
            id: numberMeasure
            visible: false
            text: root.candidates.map(candidate => String(candidate.index) + ".").join("\n")
            textFormat: Text.PlainText
            font: root.font
        }
        Item {
            anchors.fill: parent
            z: 2
            HoverHandler {
                id: popupHover
                objectName: "codexAnnotationPopupHover"
                blocking: false
                onHoveredChanged: {
                    if (hovered)
                        leaveTimer.stop();
                    else if (!root.pinned)
                        leaveTimer.restart();
                }
            }
        }
        ScrollView {
            id: detailsScroll
            anchors.fill: parent
            clip: true
            contentWidth: availableWidth
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

            ColumnLayout {
                id: details
                width: Math.max(0, root.fittedWidth - root.leftPadding - root.rightPadding - detailsScroll.leftPadding - detailsScroll.rightPadding)
                spacing: 12

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 7

                    ControlsImpl.IconImage {
                        objectName: "codexAnnotationHeadingIcon"
                        Layout.preferredWidth: 16
                        Layout.preferredHeight: 16
                        source: "qrc:///icons/hugeicons/chat-feedback-01.svg"
                        sourceSize.width: 24
                        sourceSize.height: 24
                        color: root.palette.text
                    }

                    Label {
                        objectName: "codexAnnotationHeading"
                        Layout.fillWidth: true
                        text: root.heading
                        textFormat: Text.PlainText
                        font.weight: Font.DemiBold
                        wrapMode: Text.Wrap
                    }
                }
                Label {
                    objectName: "codexAnnotationMissing"
                    Layout.fillWidth: true
                    visible: root.candidates.length === 0
                    text: /*% "No earlier annotation with this number was found in this turn." */ qsTrId("craftward.codex.annotations.missing")
                    wrapMode: Text.Wrap
                    font.pixelSize: root.hintPixelSize
                    color: root.hintColor
                }
                Repeater {
                    model: root.candidates
                    delegate: ColumnLayout {
                        id: candidate
                        required property var modelData
                        required property int index
                        Layout.fillWidth: true
                        spacing: 8
                        Rectangle {
                            objectName: "codexAnnotationDivider"
                            Layout.fillWidth: true
                            Layout.leftMargin: root.numberColumnWidth + candidateRow.spacing
                            implicitHeight: 1
                            visible: candidate.index > 0
                            color: Theme.metadataBadgeRing
                        }
                        RowLayout {
                            id: candidateRow
                            Layout.fillWidth: true
                            spacing: 12

                            Label {
                                objectName: "codexAnnotationIndex"
                                Layout.preferredWidth: root.numberColumnWidth
                                Layout.alignment: Qt.AlignTop
                                text: String(candidate.modelData.index) + "."
                                textFormat: Text.PlainText
                                horizontalAlignment: Text.AlignRight
                                font: root.font
                                color: root.hintColor
                            }

                            ColumnLayout {
                                Layout.fillWidth: true
                                spacing: 5

                                Label {
                                    objectName: "codexAnnotationSource"
                                    Layout.fillWidth: true
                                    visible: root.hasAmbiguousReference
                                    text: /*% "Input %1 in this turn" */ qsTrId("craftward.codex.annotations.input").arg(candidate.modelData.inputNumber)
                                    textFormat: Text.PlainText
                                    wrapMode: Text.Wrap
                                    font.pixelSize: root.hintPixelSize
                                    color: root.hintColor
                                }
                                Label {
                                    Layout.fillWidth: true
                                    text: /*% "Selected text:" */ qsTrId("craftward.codex.annotations.selected_text")
                                    wrapMode: Text.Wrap
                                    font.pixelSize: root.hintPixelSize
                                    color: root.hintColor
                                }
                                TextEdit {
                                    objectName: "codexAnnotationSelection"
                                    Layout.fillWidth: true
                                    text: candidate.modelData.text
                                    font: root.font
                                    color: root.palette.text
                                    readOnly: true
                                    selectByMouse: true
                                    wrapMode: TextEdit.Wrap
                                    textFormat: TextEdit.PlainText
                                    selectionColor: Theme.textSelectionBackground
                                    selectedTextColor: Theme.textSelectionForeground
                                }
                                Label {
                                    Layout.fillWidth: true
                                    visible: candidate.modelData.comment.length > 0
                                    text: /*% "User comment:" */ qsTrId("craftward.codex.annotations.comment")
                                    wrapMode: Text.Wrap
                                    font.pixelSize: root.hintPixelSize
                                    color: root.hintColor
                                    topPadding: 6
                                }
                                TextEdit {
                                    objectName: "codexAnnotationComment"
                                    Layout.fillWidth: true
                                    visible: candidate.modelData.comment.length > 0
                                    text: candidate.modelData.comment
                                    font: root.font
                                    color: root.palette.text
                                    readOnly: true
                                    selectByMouse: true
                                    wrapMode: TextEdit.Wrap
                                    textFormat: TextEdit.PlainText
                                    selectionColor: Theme.textSelectionBackground
                                    selectedTextColor: Theme.textSelectionForeground
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Connections {
        target: root.anchorWindow
        function onXChanged() {
            root.dismiss();
        }
        function onYChanged() {
            root.dismiss();
        }
        function onScreenChanged() {
            root.dismiss();
        }
        function onVisibleChanged() {
            if (!root.anchorWindow.visible)
                root.dismiss();
        }
    }
    Timer {
        id: placementTimer
        interval: 0
        onTriggered: if (root.visible)
            root.place()
    }
    Instantiator {
        // Row layout can move an ancestor without changing the scroll offset.
        active: root.visible || hoverTimer.running
        model: root.anchorChain
        delegate: Connections {
            required property var modelData
            target: modelData
            function onXChanged() {
                root.dismiss();
            }
            function onYChanged() {
                root.dismiss();
            }
            function onParentChanged() {
                root.dismiss();
            }
        }
    }
    Connections {
        target: root.anchorItem
        function onVisibleChanged() {
            if (!root.anchorItem.visible)
                root.dismiss();
        }
        function onWidthChanged() {
            root.dismiss();
        }
        function onHeightChanged() {
            root.dismiss();
        }
    }
    Timer {
        id: hoverTimer
        interval: root.hoverDelay
        onTriggered: root.showDetails()
    }
    Timer {
        id: leaveTimer
        property double outsideSince: 0
        interval: Math.min(50, root.leaveDelay)
        // A popup window can suppress further hover events from the original window.
        repeat: true
        onRunningChanged: outsideSince = 0
        onTriggered: {
            if (root.pinned || popupHover.hovered || PopupPositioner.containsCursor(root.anchorItem, root.anchorRect) || PopupPositioner.containsCursor(root.contentItem.parent, Qt.rect(0, 0, root.width, root.height))) {
                outsideSince = 0;
                return;
            }
            const now = Date.now();
            if (!outsideSince)
                outsideSince = now;
            else if (now - outsideSince >= root.leaveDelay)
                root.dismiss();
        }
    }
}
