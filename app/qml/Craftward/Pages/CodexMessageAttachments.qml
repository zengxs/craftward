// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl
import QtQuick.Effects
import Craftward.Components
import Craftward.Design

Control {
    id: root

    required property var attachments
    property var previewHandler: null
    property var attachmentItems: []
    readonly property var images: attachmentItems.filter(item => item.image)
    readonly property real imageFrameSize: 90
    readonly property bool imageMaskAvailable: GraphicsInfo.api !== GraphicsInfo.Software
    readonly property bool overflowing: attachmentRow.width > strip.width
    signal fileLocationRequested(string file, int start, int end)
    signal openImageRequested(var image)

    function invalidatePreview() {
        if (previewHandler)
            previewHandler("invalidate", root);
    }
    function refreshAttachments() {
        // Timeline revisions return new lists even when these attachments have not changed.
        // Compare display fields without serializing potentially large inline image URLs.
        const fields = ["image", "resourceId", "path", "label", "pasted", "startLine", "endLine"];
        const unchanged = attachments.length === attachmentItems.length && attachments.every((item, index) => {
            const previous = attachmentItems[index];
            return String(item.url ?? "") === String(previous.url ?? "") && fields.every(field => item[field] === previous[field]);
        });
        if (unchanged)
            return;
        invalidatePreview();
        attachmentItems = attachments;
        scrollTo(0);
    }
    function scrollTo(offset) {
        strip.contentX = Math.max(0, Math.min(Math.max(0, strip.contentWidth - strip.width), offset));
    }
    function reveal(card) {
        if (card.x < strip.contentX)
            scrollTo(card.x);
        else if (card.x + card.width > strip.contentX + strip.width)
            scrollTo(card.x + card.width - strip.width);
    }
    function focusCard(index) {
        const card = attachmentRepeater.itemAt(index);
        if (card) {
            reveal(card);
            card.forceActiveFocus(Qt.TabFocusReason);
        }
    }

    padding: 0
    implicitHeight: attachmentRow.height + (overflowing ? 30 : 0)
    onAttachmentsChanged: refreshAttachments()
    onVisibleChanged: if (!visible)
        invalidatePreview()
    onWidthChanged: {
        invalidatePreview();
        scrollTo(strip.contentX);
    }
    Component.onDestruction: invalidatePreview()

    contentItem: Item {
        Flickable {
            id: strip
            objectName: "codexAttachmentStrip"
            width: parent.width
            height: attachmentRow.height
            contentWidth: attachmentRow.width
            contentHeight: attachmentRow.height
            clip: true
            // Custom horizontal handlers leave vertical wheel input to the timeline.
            interactive: false
            boundsBehavior: Flickable.StopAtBounds
            onContentXChanged: root.invalidatePreview()
            WheelHandler {
                target: null
                orientation: Qt.Horizontal
                blocking: false
                acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
                onWheel: event => {
                    const dx = event.pixelDelta.x || event.angleDelta.x / 2;
                    const dy = event.pixelDelta.y || event.angleDelta.y / 2;
                    if (Math.abs(dx) <= Math.abs(dy)) {
                        event.accepted = false;
                        return;
                    }
                    root.scrollTo(strip.contentX - dx);
                    event.accepted = true;
                }
            }
            DragHandler {
                target: null
                yAxis.enabled: false
                property real startX: 0
                onActiveChanged: if (active)
                    startX = strip.contentX
                onActiveTranslationChanged: if (active)
                    root.scrollTo(startX - activeTranslation.x)
            }
            Row {
                id: attachmentRow
                x: Math.max(0, strip.width - width)
                spacing: 8
                Repeater {
                    id: attachmentRepeater
                    model: root.attachmentItems
                    AbstractButton {
                        id: card

                        required property var modelData
                        required property int index
                        readonly property int imageIndex: root.attachmentItems.slice(0, index).filter(item => item.image).length
                        readonly property bool imageReady: modelData.image && thumbnail.status === Image.Ready
                        readonly property real imageScale: imageReady && thumbnail.implicitWidth > 0 && thumbnail.implicitHeight > 0 ? Math.min(1, root.imageFrameSize / thumbnail.implicitWidth, root.imageFrameSize / thumbnail.implicitHeight) : 1
                        readonly property real imageRadius: root.imageMaskAvailable ? Math.min(8, width / 2, height / 2) : 0
                        objectName: "codexAttachmentCard"
                        width: modelData.image ? root.imageFrameSize : Math.min(root.availableWidth, 280)
                        height: modelData.image ? root.imageFrameSize : 52
                        padding: modelData.image ? 0 : 8
                        hoverEnabled: true
                        focusPolicy: Qt.StrongFocus
                        Accessible.name: modelData.image ? /*% "Image %1 of %2" */ qsTrId("craftward.image.accessible_position").arg(imageIndex + 1).arg(root.images.length) : modelData.label
                        ToolTip.visible: !modelData.image && hovered
                        ToolTip.delay: 500
                        ToolTip.text: modelData.label
                        onClicked: {
                            if (modelData.image) {
                                if (root.previewHandler)
                                    root.previewHandler("open", card, root.images, imageIndex);
                            } else if (modelData.path) {
                                root.fileLocationRequested(modelData.path, modelData.startLine || 0, modelData.endLine || 0);
                            }
                        }
                        onActiveFocusChanged: if (activeFocus)
                            root.reveal(card)
                        Keys.onLeftPressed: root.focusCard(index - 1)
                        Keys.onRightPressed: root.focusCard(index + 1)
                        HoverHandler {
                            cursorShape: Qt.PointingHandCursor
                        }
                        TapHandler {
                            acceptedButtons: Qt.RightButton
                            onTapped: if (card.modelData.image)
                                imageMenu.popup()
                        }
                        Menu {
                            id: imageMenu
                            MenuItem {
                                text: /*% "Open in Tab" */ qsTrId("craftward.image.open_tab")
                                onTriggered: root.openImageRequested(card.modelData)
                            }
                        }
                        background: Rectangle {
                            radius: card.modelData.image ? card.imageRadius : 10
                            color: card.modelData.image ? Theme.metadataBadgeSurface : (card.hovered ? Theme.userMessageSurface : "transparent")
                            border.width: card.modelData.image ? 0 : 1
                            border.color: Theme.metadataBadgeRing
                        }
                        contentItem: Item {
                            Item {
                                id: thumbnailContent
                                anchors.fill: parent
                                visible: card.imageReady && !root.imageMaskAvailable

                                Image {
                                    id: thumbnail
                                    objectName: "codexAttachmentThumbnail"
                                    anchors.centerIn: parent
                                    width: implicitWidth * card.imageScale
                                    height: implicitHeight * card.imageScale
                                    source: card.modelData.image ? card.modelData.url : ""
                                    sourceSize: Qt.size(root.imageFrameSize * 2, root.imageFrameSize * 2)
                                    // Fit the item inside the fixed frame. Stretch keeps
                                    // decoding bounded without fit-mode upscaling.
                                    fillMode: Image.Stretch
                                    autoTransform: true
                                    asynchronous: true
                                    cache: false
                                }
                            }
                            MultiEffect {
                                anchors.fill: parent
                                visible: card.imageReady && root.imageMaskAvailable
                                source: thumbnailContent
                                maskEnabled: true
                                maskSource: Rectangle {
                                    parent: thumbnailContent.parent
                                    width: card.width
                                    height: card.height
                                    radius: card.imageRadius
                                    color: "white"
                                    visible: false
                                    layer.enabled: true
                                    layer.smooth: true
                                }
                            }
                            Rectangle {
                                objectName: "codexAttachmentImageBorder"
                                anchors.fill: parent
                                visible: card.modelData.image
                                color: "transparent"
                                radius: card.imageRadius
                                border.width: card.activeFocus ? 2 : 1
                                border.color: card.activeFocus ? root.palette.highlight : (card.hovered ? Qt.rgba(root.palette.text.r, root.palette.text.g, root.palette.text.b, Theme.dark ? 0.28 : 0.22) : Theme.metadataBadgeRing)
                            }
                            ControlsImpl.IconImage {
                                x: card.modelData.image ? (parent.width - width) / 2 : 0
                                y: card.modelData.image ? (parent.height - height) / 2 : 2
                                width: card.modelData.image ? 24 : 16
                                height: width
                                visible: !card.modelData.image || thumbnail.status === Image.Error
                                source: card.modelData.image ? "qrc:///icons/hugeicons/image-not-found-01.svg" : "qrc:///icons/hugeicons/file-02.svg"
                                sourceSize: Qt.size(24, 24)
                                color: root.palette.placeholderText
                            }
                            BusyIndicator {
                                anchors.centerIn: parent
                                width: 28
                                height: 28
                                running: card.modelData.image && thumbnail.status === Image.Loading
                                visible: running
                            }
                            Label {
                                objectName: "codexAttachmentFileLabel"
                                x: 22
                                width: parent.width - x
                                visible: !card.modelData.image
                                text: String(card.modelData.label).replace(/\s+/g, " ")
                                textFormat: Text.PlainText
                                elide: Text.ElideMiddle
                                font: root.font
                                color: root.palette.text
                            }
                            Label {
                                x: 22
                                y: 20
                                width: parent.width - x
                                visible: !card.modelData.image
                                text: card.modelData.pasted ? /*% "Pasted text" */ qsTrId("craftward.codex.attachment.pasted_text") : ""
                                textFormat: Text.PlainText
                                elide: Text.ElideMiddle
                                font.pixelSize: 11
                                color: root.palette.placeholderText
                            }
                        }
                    }
                }
            }
        }
        Row {
            anchors.right: parent.right
            y: strip.height + 2
            height: 28
            visible: root.overflowing
            spacing: 6
            IconButton {
                objectName: "attachmentScrollPrevious"
                width: 28
                height: 28
                icon.source: "qrc:///icons/hugeicons/chevron-right.svg"
                iconRotation: 180
                enabled: strip.contentX > 0
                toolTipText: /*% "Previous attachments" */ qsTrId("craftward.attachment.previous")
                onClicked: root.scrollTo(strip.contentX - Math.max(98, strip.width - 24))
            }
            Label {
                anchors.verticalCenter: parent.verticalCenter
                text: /*% "%n attachment(s)" */ qsTrId("craftward.attachment.count", root.attachmentItems.length)
                color: root.palette.placeholderText
            }
            IconButton {
                objectName: "attachmentScrollNext"
                width: 28
                height: 28
                icon.source: "qrc:///icons/hugeicons/chevron-right.svg"
                enabled: strip.contentX < strip.contentWidth - strip.width
                toolTipText: /*% "Next attachments" */ qsTrId("craftward.attachment.next")
                onClicked: root.scrollTo(strip.contentX + Math.max(98, strip.width - 24))
            }
        }
    }
}
