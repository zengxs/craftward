// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQml.Models
import Craftward.Components

WindowPopover {
    id: root

    property Item anchorItem: null
    property var anchorChain: []
    property var images: []
    property int currentIndex: 0
    property size fittedSize: Qt.size(480, 400)
    readonly property var currentImage: images[currentIndex] ?? null
    readonly property var anchorWindow: anchorItem ? anchorItem.Window.window : null
    signal openImageRequested(var image)

    objectName: "codexAttachmentPreview"
    modal: false
    focus: true
    // Native popup windows also use implicit dimensions when processing layout updates.
    implicitWidth: fittedSize.width
    implicitHeight: fittedSize.height
    width: fittedSize.width
    height: fittedSize.height
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    function dismiss() {
        close();
        anchorItem = null;
        anchorChain = [];
    }

    onAnchorItemChanged: if (!anchorItem)
        dismiss()

    function handle(action, item, items = [], index = 0) {
        if (action === "invalidate") {
            if (anchorChain.indexOf(item) >= 0)
                dismiss();
            return;
        }
        if (!item || !items.length)
            return;
        if (visible && anchorItem === item) {
            dismiss();
            return;
        }
        close();
        images = items.slice();
        currentIndex = index;
        anchorItem = item;
        const chain = [];
        for (let ancestor = item; ancestor; ancestor = ancestor.parent)
            chain.push(ancestor);
        anchorChain = chain;
        place();
        open();
    }

    function place() {
        if (!anchorItem || !parent)
            return;
        const geometry = PopupPositioner.place(anchorItem, Qt.rect(0, 0, anchorItem.width, anchorItem.height), parent, Qt.size(480, 400));
        if (geometry.width <= 0 || geometry.height <= 0)
            return;
        x = geometry.x;
        y = geometry.y;
        fittedSize = Qt.size(geometry.width, geometry.height);
    }

    function step(offset) {
        currentIndex = Math.max(0, Math.min(images.length - 1, currentIndex + offset));
    }

    onAboutToShow: place()
    onOpened: contentItem.forceActiveFocus(Qt.PopupFocusReason)
    onClosed: {
        if (anchorItem && anchorItem.visible)
            anchorItem.forceActiveFocus(Qt.OtherFocusReason);
    }

    contentItem: ColumnLayout {
        spacing: 8
        Keys.onLeftPressed: root.step(-1)
        Keys.onRightPressed: root.step(1)

        RowLayout {
            Layout.fillWidth: true
            spacing: 4
            IconButton {
                objectName: "imagePreviewPrevious"
                icon.source: "qrc:///icons/hugeicons/chevron-right.svg"
                iconRotation: 180
                enabled: root.currentIndex > 0
                toolTipText: /*% "Previous image" */ qsTrId("craftward.image.previous")
                onClicked: root.step(-1)
            }
            Label {
                text: /*% "%1 / %2" */ qsTrId("craftward.image.position").arg(root.currentIndex + 1).arg(root.images.length)
                Accessible.name: /*% "Image %1 of %2" */ qsTrId("craftward.image.accessible_position").arg(root.currentIndex + 1).arg(root.images.length)
            }
            IconButton {
                objectName: "imagePreviewNext"
                icon.source: "qrc:///icons/hugeicons/chevron-right.svg"
                enabled: root.currentIndex + 1 < root.images.length
                toolTipText: /*% "Next image" */ qsTrId("craftward.image.next")
                onClicked: root.step(1)
            }
            Item {
                Layout.fillWidth: true
            }
            IconButton {
                objectName: "imagePreviewOpenTab"
                icon.source: "qrc:///icons/hugeicons/arrow-up-to-line.svg"
                toolTipText: /*% "Open in Tab" */ qsTrId("craftward.image.open_tab")
                enabled: root.currentImage !== null
                onClicked: {
                    const image = root.currentImage;
                    root.dismiss();
                    root.openImageRequested(image);
                }
            }
        }
        Item {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Image {
                id: fullImage
                objectName: "codexAttachmentFullImage"
                readonly property real fitScale: implicitWidth > 0 && implicitHeight > 0 ? Math.min(1, parent.width / implicitWidth, parent.height / implicitHeight) : 1
                anchors.centerIn: parent
                width: implicitWidth * fitScale
                height: implicitHeight * fitScale
                source: root.visible && root.currentImage ? root.currentImage.url : ""
                sourceSize: Qt.size(960, 800)
                fillMode: Image.Stretch
                autoTransform: true
                asynchronous: true
                cache: false
            }
            BusyIndicator {
                anchors.centerIn: parent
                running: fullImage.status === Image.Loading
                visible: running
            }
            Label {
                anchors.centerIn: parent
                width: parent.width
                visible: fullImage.status === Image.Error
                text: /*% "Image preview unavailable" */ qsTrId("craftward.codex.attachment.preview_unavailable")
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
            }
        }
    }

    PopupAnchorToggle {
        anchorItem: root.anchorItem
        popupWindow: root.contentItem.Window.window
        enabled: root.visible && popupWindow !== root.anchorWindow
        onActivated: root.dismiss()
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
    Instantiator {
        active: root.visible
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
}
