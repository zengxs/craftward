// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import Craftward.Components
import Craftward.Design
import Craftward.Terminal

Pane {
    id: root
    required property TerminalController controller
    padding: 0

    background: Rectangle {
        color: TailwindColors.white
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

                    delegate: PanelTab {
                        id: tab
                        required property int index
                        required property string title
                        selected: tab.index === root.controller.activeTabIndex
                        property string displayedTitle: ""
                        property bool titleInitialized: false
                        height: tabList.height
                        width: implicitWidth
                        text: displayedTitle || /*% "Terminal" */ qsTrId("craftward.terminal.title")
                        iconSource: "qrc:///icons/hugeicons/square-terminal.svg"
                        closeToolTipText: /*% "Close terminal" */ qsTrId("craftward.terminal.close")
                        onActivated: root.controller.activateTab(index)
                        onCloseRequested: root.controller.closeTab(index)

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

                    PanelActionButton {
                        objectName: "newTerminalButton"
                        anchors.centerIn: parent
                        icon.source: "qrc:///icons/hugeicons/add-01.svg"
                        enabled: root.controller.available
                        toolTipText: /*% "New terminal" */ qsTrId("craftward.terminal.new")
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
