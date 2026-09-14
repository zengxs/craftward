// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl as ControlsImpl
import QtQuick.Layouts
import Craftward.Design
import Craftward.Terminal

Pane {
    id: root
    required property TerminalController controller
    padding: 0

    background: Rectangle {
        color: TailwindColors.white
    }

    component TerminalActionButton: ToolButton {
        id: action
        implicitWidth: 26
        implicitHeight: 26
        padding: 5
        display: AbstractButton.IconOnly
        hoverEnabled: true
        icon.width: 16
        icon.height: 16
        icon.color: !enabled ? TailwindColors.zinc400 : hovered || down ? TailwindColors.zinc700 : TailwindColors.zinc500

        background: Rectangle {
            anchors.fill: parent
            anchors.margins: 3
            radius: 4
            color: action.down ? TailwindColors.zinc300 : action.hovered ? TailwindColors.zinc200 : TailwindColors.transparent
            border.width: action.visualFocus ? 1 : 0
            border.color: TailwindColors.blue600
        }

        ToolTip.visible: hovered && !down
        ToolTip.delay: 500
        ToolTip.text: Accessible.name
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        Rectangle {
            Layout.fillWidth: true
            Layout.preferredHeight: 32
            color: TailwindColors.zinc100

            Rectangle {
                anchors {
                    left: parent.left
                    right: parent.right
                    bottom: parent.bottom
                }
                height: 1
                color: TailwindColors.zinc200
            }

            RowLayout {
                anchors.fill: parent
                spacing: 0

                ListView {
                    id: tabList
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    orientation: ListView.Horizontal
                    clip: true
                    model: root.controller.tabs
                    currentIndex: root.controller.activeTabIndex
                    boundsBehavior: Flickable.StopAtBounds
                    highlightMoveDuration: 0
                    onCurrentIndexChanged: positionViewAtIndex(currentIndex, ListView.Contain)

                    delegate: Rectangle {
                        id: tab
                        required property int index
                        required property string title
                        readonly property bool selected: tab.index === root.controller.activeTabIndex
                        property string displayedTitle: ""
                        property bool titleInitialized: false
                        height: tabList.height
                        width: 180
                        color: selected ? TailwindColors.white : TailwindColors.zinc100

                        // Tab titles are navigation hints: brief shell/command changes should not
                        // distract from terminal work. Only the label waits for 300 ms of stability.
                        onTitleChanged: {
                            if (!titleInitialized)
                                return;
                            if (title === displayedTitle) {
                                titleSettleTimer.stop();
                                titleMaxWaitTimer.stop();
                                return;
                            }
                            titleSettleTimer.restart();
                            if (!titleMaxWaitTimer.running)
                                titleMaxWaitTimer.start();
                        }
                        Component.onCompleted: {
                            titleInitialized = true;
                            publishTitle();
                        }
                        function publishTitle() {
                            displayedTitle = title;
                            titleSettleTimer.stop();
                            titleMaxWaitTimer.stop();
                        }

                        Timer {
                            id: titleSettleTimer
                            interval: 300
                            onTriggered: tab.publishTitle()
                        }
                        Timer {
                            id: titleMaxWaitTimer
                            // Progress titles may never settle; cap their pending wait at one second.
                            interval: 1000
                            onTriggered: tab.publishTitle()
                        }

                        HoverHandler {
                            id: tabHover
                        }
                        TapHandler {
                            onTapped: root.controller.activateTab(tab.index)
                        }

                        ControlsImpl.IconImage {
                            id: terminalIcon
                            anchors {
                                left: parent.left
                                verticalCenter: parent.verticalCenter
                                leftMargin: 12
                            }
                            width: 16
                            height: 16
                            source: "qrc:///icons/hugeicons/square-terminal.svg"
                            sourceSize.width: 16
                            sourceSize.height: 16
                            color: tab.selected ? TailwindColors.zinc700 : TailwindColors.zinc500
                        }
                        Label {
                            id: tabTitle
                            anchors {
                                left: terminalIcon.right
                                right: closeButton.left
                                verticalCenter: parent.verticalCenter
                                leftMargin: 8
                                rightMargin: 4
                            }
                            text: tab.displayedTitle || /*% "Terminal" */ qsTrId("craftward.terminal.title")
                            color: tab.selected ? TailwindColors.zinc700 : TailwindColors.zinc500
                            font.pixelSize: 12
                            elide: Text.ElideRight
                        }
                        TerminalActionButton {
                            id: closeButton
                            anchors {
                                right: parent.right
                                verticalCenter: parent.verticalCenter
                                rightMargin: 3
                            }
                            width: 26
                            height: 26
                            icon.source: "qrc:///icons/hugeicons/cancel-01.svg"
                            opacity: tab.selected || tabHover.hovered || hovered || visualFocus ? 1 : 0
                            Accessible.name: /*% "Close terminal" */ qsTrId("craftward.terminal.close")
                            onClicked: root.controller.closeTab(tab.index)
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
                            visible: !tab.selected
                        }
                    }

                    Rectangle {
                        anchors {
                            right: parent.right
                            top: parent.top
                            bottom: parent.bottom
                        }
                        z: 1
                        width: 1
                        color: TailwindColors.zinc200
                    }
                }

                Item {
                    Layout.preferredWidth: 36
                    Layout.fillHeight: true

                    TerminalActionButton {
                        objectName: "newTerminalButton"
                        anchors.centerIn: parent
                        icon.source: "qrc:///icons/hugeicons/add-01.svg"
                        enabled: root.controller.available
                        Accessible.name: /*% "New terminal" */ qsTrId("craftward.terminal.new")
                        onClicked: root.controller.createTerminal()
                    }
                }
            }
        }

        TerminalView {
            id: terminalView
            objectName: "conversationTerminal"
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.leftMargin: 8
            Layout.topMargin: 5
            Layout.rightMargin: 4
            Layout.bottomMargin: 4
            session: root.controller.activeSession
            visible: session !== null
        }

        Label {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: 12
            visible: !terminalView.visible
            text: root.controller.errorMessage || /*% "Create a terminal with the + button." */ qsTrId("craftward.terminal.empty")
            color: TailwindColors.zinc500
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
        }
    }

    Connections {
        target: root.controller
        function onFocusRequested() {
            Qt.callLater(terminalView.focusTerminal);
        }
    }
}
