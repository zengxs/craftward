// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Components

Pane {
    id: root

    required property var resource
    required property ImageViewState viewState
    property bool restoring: true
    readonly property real fitScale: image.implicitWidth > 0 && image.implicitHeight > 0 ? Math.min(1, viewport.width / image.implicitWidth, viewport.height / image.implicitHeight) : 1
    readonly property real imageScale: !viewState || viewState.fit ? fitScale : viewState.zoom
    padding: 0
    background: Rectangle {
        color: root.palette.base
    }

    function restorePosition() {
        if (image.status !== Image.Ready || !viewState)
            return;
        viewport.contentX = Math.max(0, Math.min(viewport.contentWidth - viewport.width, viewState.centerX * viewport.contentWidth - viewport.width / 2));
        viewport.contentY = Math.max(0, Math.min(viewport.contentHeight - viewport.height, viewState.centerY * viewport.contentHeight - viewport.height / 2));
        restoring = false;
    }
    function scheduleRestore() {
        restoring = true;
        Qt.callLater(root.restorePosition);
    }
    function setZoom(value) {
        if (!viewState)
            return;
        restoring = true;
        viewState.zoom = Math.max(0.05, Math.min(16, value));
        viewState.fit = false;
        scheduleRestore();
    }
    function fitImage() {
        if (!viewState)
            return;
        viewState.centerX = 0.5;
        viewState.centerY = 0.5;
        viewState.fit = true;
        scheduleRestore();
    }

    onViewStateChanged: scheduleRestore()
    onResourceChanged: scheduleRestore()
    onImageScaleChanged: scheduleRestore()

    ColumnLayout {
        anchors.fill: parent
        spacing: 0
        RowLayout {
            Layout.fillWidth: true
            Layout.margins: 8
            IconButton {
                objectName: "imageZoomOut"
                icon.source: "qrc:///icons/hugeicons/zoom-out.svg"
                enabled: image.status === Image.Ready && root.imageScale > 0.05
                toolTipText: /*% "Zoom out" */ qsTrId("craftward.image.zoom_out")
                onClicked: root.setZoom(root.imageScale / 1.25)
            }
            Label {
                text: Math.round(root.imageScale * 100) + "%"
            }
            IconButton {
                objectName: "imageZoomIn"
                icon.source: "qrc:///icons/hugeicons/zoom-in.svg"
                enabled: image.status === Image.Ready && root.imageScale < 16
                toolTipText: /*% "Zoom in" */ qsTrId("craftward.image.zoom_in")
                onClicked: root.setZoom(root.imageScale * 1.25)
            }
            IconButton {
                objectName: "imageActualSize"
                icon.source: "qrc:///icons/hugeicons/image-02.svg"
                enabled: image.status === Image.Ready
                toolTipText: /*% "Actual size" */ qsTrId("craftward.image.actual_size")
                onClicked: root.setZoom(1)
            }
            IconButton {
                objectName: "imageFit"
                icon.source: "qrc:///icons/hugeicons/scan-image.svg"
                toolTipText: /*% "Fit" */ qsTrId("craftward.image.fit")
                enabled: image.status === Image.Ready
                checked: root.viewState ? root.viewState.fit : true
                onClicked: root.fitImage()
            }
            Item {
                Layout.fillWidth: true
            }
        }
        Flickable {
            id: viewport
            objectName: "imageViewViewport"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            contentWidth: Math.max(width, image.width)
            contentHeight: Math.max(height, image.height)
            onWidthChanged: root.scheduleRestore()
            onHeightChanged: root.scheduleRestore()
            onContentWidthChanged: root.scheduleRestore()
            onContentHeightChanged: root.scheduleRestore()
            onContentXChanged: if (root.viewState && !root.restoring && contentWidth > 0)
                root.viewState.centerX = (contentX + width / 2) / contentWidth
            onContentYChanged: if (root.viewState && !root.restoring && contentHeight > 0)
                root.viewState.centerY = (contentY + height / 2) / contentHeight
            ScrollBar.horizontal: ScrollBar {}
            ScrollBar.vertical: ScrollBar {}

            Image {
                id: image
                objectName: "imageViewImage"
                x: (viewport.contentWidth - width) / 2
                y: (viewport.contentHeight - height) / 2
                width: implicitWidth * root.imageScale
                height: implicitHeight * root.imageScale
                source: root.resource ? root.resource.url : ""
                fillMode: Image.Stretch
                autoTransform: true
                asynchronous: true
                cache: false
                onStatusChanged: root.scheduleRestore()
            }
            WheelHandler {
                target: null
                acceptedModifiers: Qt.ControlModifier
                onWheel: event => {
                    root.setZoom(root.imageScale * Math.pow(1.0015, event.angleDelta.y));
                    event.accepted = true;
                }
            }
        }
    }
    BusyIndicator {
        anchors.centerIn: parent
        running: image.status === Image.Loading
        visible: running
    }
    Label {
        anchors.centerIn: parent
        width: Math.max(0, parent.width - 40)
        visible: image.status === Image.Error
        text: /*% "Image preview unavailable" */ qsTrId("craftward.codex.attachment.preview_unavailable")
        wrapMode: Text.WordWrap
        horizontalAlignment: Text.AlignHCenter
    }
}
