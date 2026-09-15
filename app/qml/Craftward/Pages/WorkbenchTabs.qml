// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import Craftward.Components
import Craftward.Design

Pane {
    id: root

    required property ThreadWorkspaceState workspace
    readonly property var openTabs: workspace ? workspace.tabs : []
    readonly property int selectedIndex: workspace ? workspace.activeIndex : 0
    signal conversationActionsRequested(Item anchor)

    function synchronizeSelection() {
        // Array replacement can reset the view's current index without changing the selected tab.
        additionalTabs.currentIndex = selectedIndex - 1;
        if (additionalTabs.currentIndex >= 0)
            additionalTabs.positionViewAtIndex(additionalTabs.currentIndex, ListView.Contain);
    }

    onSelectedIndexChanged: Qt.callLater(synchronizeSelection)

    implicitHeight: 32
    padding: 0
    background: Rectangle {
        color: TailwindColors.zinc100
    }

    Rectangle {
        anchors {
            left: parent.left
            right: parent.right
            bottom: parent.bottom
        }
        height: 1
        color: TailwindColors.zinc200
    }

    PanelTab {
        id: conversationTab
        objectName: "conversationTab"
        implicitWidth: 150
        width: Math.min(implicitWidth, root.width)
        closable: false
        actionIconSource: "qrc:///icons/hugeicons/more-horizontal-circle-02.svg"
        actionToolTipText: /*% "More Actions" */ qsTrId("craftward.actions.more")
        iconSource: "qrc:///icons/hugeicons/chat-01.svg"
        text: /*% "Conversation" */ qsTrId("craftward.codex.history.conversation.title")
        selected: root.selectedIndex === 0
        onActivated: root.workspace.activeIndex = 0
        onActionRequested: anchor => {
            root.workspace.activeIndex = 0;
            root.conversationActionsRequested(anchor);
        }
    }

    ListView {
        id: additionalTabs
        objectName: "additionalTabs"
        anchors {
            left: conversationTab.right
            right: parent.right
            top: parent.top
            bottom: parent.bottom
        }
        orientation: ListView.Horizontal
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: root.openTabs
        currentIndex: -1
        highlightMoveDuration: 0
        onModelChanged: Qt.callLater(root.synchronizeSelection)
        Component.onCompleted: Qt.callLater(root.synchronizeSelection)

        delegate: PanelTab {
            id: documentTab
            required property int index
            required property var modelData
            width: implicitWidth
            text: modelData.title
            toolTipText: modelData.path || ""
            iconSource: "qrc:///icons/hugeicons/file-02.svg"
            badgeText: modelData.external ? /*% "External" */ qsTrId("craftward.file.external") : ""
            closeToolTipText: /*% "Close Tab" */ qsTrId("craftward.tab.close")
            selected: root.selectedIndex === index + 1
            closable: true
            onActivated: root.workspace.activeIndex = index + 1
            onCloseRequested: root.workspace.closeTab(index + 1)

            DragHandler {
                id: tabDrag
                target: null
                yAxis.enabled: false
                onActiveChanged: {
                    if (active)
                        return;
                    const position = documentTab.mapToItem(additionalTabs.contentItem, centroid.position.x, centroid.position.y);
                    const destination = additionalTabs.indexAt(position.x, position.y);
                    if (destination >= 0)
                        root.workspace.moveTab(documentTab.index + 1, destination + 1);
                }
            }
            TapHandler {
                acceptedButtons: Qt.RightButton
                onTapped: tabMenu.popup()
            }
            Menu {
                id: tabMenu
                popupType: Popup.Window
                MenuItem {
                    text: /*% "Move Tab Left" */ qsTrId("craftward.tab.move_left")
                    enabled: documentTab.index > 0
                    onTriggered: root.workspace.moveTab(documentTab.index + 1, documentTab.index)
                }
                MenuItem {
                    text: /*% "Move Tab Right" */ qsTrId("craftward.tab.move_right")
                    enabled: documentTab.index < root.openTabs.length - 1
                    onTriggered: root.workspace.moveTab(documentTab.index + 1, documentTab.index + 2)
                }
                MenuSeparator {}
                MenuItem {
                    text: /*% "Close Tab" */ qsTrId("craftward.tab.close")
                    onTriggered: documentTab.closeRequested()
                }
            }
        }
    }
}
