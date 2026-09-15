// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Components
import Craftward.Design

Pane {
    id: root

    property string directory: ""
    property string selectedPath: ""
    property string pendingRevealPath: ""
    signal fileRequested(string path)
    signal openFileRequested

    padding: 0
    background: Rectangle {
        color: Theme.sidebarSurface
    }

    function reveal(path) {
        pendingRevealPath = path;
        scheduleReveal();
    }

    function scheduleReveal() {
        if (pendingRevealPath.length > 0)
            Qt.callLater(completeReveal);
    }

    function completeReveal() {
        if (!pendingRevealPath || !visible || !filesModel.available || tree.width <= 0 || tree.height <= 0)
            return;
        const index = filesModel.indexForPath(pendingRevealPath);
        if (!index.valid) {
            pendingRevealPath = "";
            return;
        }
        // Ancestor contents can still be arriving after the path's index becomes valid.
        if (!filesModel.loadAncestors(index))
            return;
        tree.expandToIndex(index);
        tree.forceLayout();
        const row = tree.rowAtIndex(index);
        if (row < 0)
            return;
        tree.positionViewAtRow(row, TableView.Contain);
        const item = tree.itemAtCell(Qt.point(0, row));
        if (item && item.y >= tree.contentY - 0.5 && item.y + item.height <= tree.contentY + tree.height + 0.5)
            pendingRevealPath = "";
    }

    onDirectoryChanged: pendingRevealPath = ""
    onVisibleChanged: scheduleReveal()

    ProjectFilesModel {
        id: filesModel
        directory: root.directory
        onDirectoryLoaded: root.scheduleReveal()
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        RowLayout {
            Layout.fillWidth: true
            Layout.preferredHeight: 32
            Layout.leftMargin: 12
            Layout.rightMargin: 3
            spacing: 4

            Label {
                Layout.fillWidth: true
                Layout.minimumWidth: 0
                text: root.directory.split("/").filter(part => part.length > 0).pop() || ""
                font.pixelSize: 12
                font.weight: Font.DemiBold
                elide: Text.ElideMiddle
                ToolTip.visible: directoryHover.hovered
                ToolTip.text: root.directory
                ToolTip.delay: 500
                HoverHandler {
                    id: directoryHover
                }
            }
            PanelActionButton {
                icon.source: "qrc:///icons/hugeicons/folder-02.svg"
                toolTipText: /*% "Open File…" */ qsTrId("craftward.file.open")
                onClicked: root.openFileRequested()
            }
        }

        TreeView {
            id: tree
            objectName: "projectFileTree"
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true
            visible: filesModel.available
            model: filesModel.available ? filesModel : null
            rootIndex: filesModel.rootIndex
            boundsBehavior: Flickable.StopAtBounds
            columnWidthProvider: column => column === 0 ? width : 0
            onRowsChanged: root.scheduleReveal()
            onLayoutChanged: root.scheduleReveal()
            onWidthChanged: root.scheduleReveal()
            onHeightChanged: root.scheduleReveal()
            ScrollBar.vertical: OverlayScrollBar {}

            delegate: TreeViewDelegate {
                id: fileRow
                required property string filePath
                required property string fileName
                implicitWidth: tree.width
                implicitHeight: 28
                indentation: 14
                leftMargin: 8
                rightMargin: 8
                highlighted: filePath === root.selectedPath
                text: fileName
                font.pixelSize: 12
                onClicked: {
                    if (filesModel.isDirectory(tree.index(row, 0)))
                        tree.toggleExpanded(row);
                    else
                        root.fileRequested(filePath);
                }
                ToolTip.visible: hovered
                ToolTip.text: filePath
                ToolTip.delay: 800
            }
        }

        Label {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: 16
            visible: !filesModel.available
            text: /*% "This conversation has no accessible project directory." */ qsTrId("craftward.files.unavailable")
            color: root.palette.placeholderText
            font.pixelSize: 12
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
    }
}
