// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import Craftward.Design

Rectangle {
    id: root

    property string text: ""
    property url iconSource
    property bool selected: false
    property bool closable: true
    property url actionIconSource
    property string actionToolTipText: ""
    property string badgeText: ""
    property string toolTipText: ""
    property string closeToolTipText: ""
    signal activated
    signal actionRequested(Item anchor)
    signal closeRequested

    implicitWidth: 180
    implicitHeight: 32
    color: selected ? TailwindColors.white : TailwindColors.zinc100
    activeFocusOnTab: true
    Accessible.role: Accessible.PageTab
    Accessible.name: text
    Keys.onSpacePressed: activated()
    Keys.onReturnPressed: activated()

    HoverHandler {
        id: tabHover
    }
    TapHandler {
        onTapped: root.activated()
    }

    RowLayout {
        anchors {
            fill: parent
            leftMargin: 12
            rightMargin: root.closable || root.actionIconSource.toString().length > 0 ? 3 : 12
        }
        spacing: 0

        IconImage {
            Layout.preferredWidth: 16
            Layout.preferredHeight: 16
            Layout.rightMargin: 8
            visible: root.iconSource.toString().length > 0
            source: root.iconSource
            sourceSize: Qt.size(16, 16)
            color: root.selected ? TailwindColors.zinc700 : TailwindColors.zinc500
        }
        Label {
            Layout.fillWidth: true
            Layout.minimumWidth: 0
            Layout.rightMargin: 4
            text: root.text
            color: root.selected ? TailwindColors.zinc700 : TailwindColors.zinc500
            font.pixelSize: 12
            elide: Text.ElideRight
        }
        Label {
            Layout.rightMargin: 4
            visible: root.badgeText.length > 0
            text: root.badgeText
            color: TailwindColors.zinc500
            font.pixelSize: 10
        }
        PanelActionButton {
            id: actionButton
            objectName: "panelTabAction"
            visible: root.actionIconSource.toString().length > 0
            icon.source: root.actionIconSource
            toolTipText: root.actionToolTipText
            onClicked: root.actionRequested(actionButton)
        }
        PanelActionButton {
            objectName: "closePanelTab"
            visible: root.closable
            opacity: root.selected || tabHover.hovered || hovered || visualFocus ? 1 : 0
            icon.source: "qrc:///icons/hugeicons/cancel-01.svg"
            toolTipText: root.closeToolTipText
            onClicked: root.closeRequested()
        }
    }

    Rectangle {
        anchors.right: parent.right
        width: 1
        height: parent.height
        color: TailwindColors.zinc200
    }
    Rectangle {
        anchors {
            left: parent.left
            right: parent.right
            bottom: parent.bottom
        }
        height: 1
        color: TailwindColors.zinc200
        visible: !root.selected
    }

    ToolTip.visible: tabHover.hovered && toolTipText.length > 0
    ToolTip.text: toolTipText
    ToolTip.delay: 700
}
