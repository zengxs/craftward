// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import Craftward.Components
import Craftward.Realm

Page {
    id: root

    required property RealmController controller

    signal displayRequested

    background: Rectangle {
        color: root.palette.window

        WindowMoveHandler {
            targetWindow: root.ApplicationWindow.window
        }
    }

    FolderDialog {
        id: bundleDialog

        title: qsTr("Choose an installed Realm bundle")
        onAccepted: root.controller.bundleUrl = selectedFolder
    }

    ModalDialog {
        id: forceStopDialog

        anchors.centerIn: Overlay.overlay
        width: Math.min(420, root.width - 48)
        title: qsTr("Force stop this Realm?")
        onAccepted: root.controller.forceStop()

        contentItem: ColumnLayout {
            spacing: 12

            Label {
                Layout.fillWidth: true
                text: forceStopDialog.title
                font.pixelSize: 18
                font.weight: Font.DemiBold
                wrapMode: Text.WordWrap
            }

            Label {
                Layout.fillWidth: true
                text: qsTr("The guest will not have a chance to shut down. Unsaved data may be lost and its disk may be damaged.")
                wrapMode: Text.WordWrap
            }

            Item {
                Layout.preferredHeight: 4
            }

            RowLayout {
                Layout.fillWidth: true

                Item {
                    Layout.fillWidth: true
                }

                Button {
                    text: qsTr("Cancel")
                    onClicked: forceStopDialog.reject()
                }

                Button {
                    text: qsTr("Force Stop")
                    onClicked: forceStopDialog.accept()
                }
            }
        }
    }

    ColumnLayout {
        anchors {
            fill: parent
            topMargin: root.SafeArea.margins.top + 40
            leftMargin: 48
            rightMargin: 48
            bottomMargin: Math.max(36, root.SafeArea.margins.bottom)
        }
        spacing: 20

        ColumnLayout {
            Layout.fillWidth: true
            spacing: 6

            Label {
                Layout.fillWidth: true
                text: qsTr("Realm")
                font.pixelSize: 32
                font.weight: Font.DemiBold
            }

            Label {
                Layout.fillWidth: true
                text: qsTr("Run an isolated development environment from an installed bundle.")
                color: root.palette.placeholderText
                font.pixelSize: 15
                wrapMode: Text.WordWrap
            }
        }

        Rectangle {
            Layout.fillWidth: true
            implicitHeight: realmCardContent.implicitHeight + 48
            radius: 14
            color: root.palette.base
            border.color: root.palette.mid

            ColumnLayout {
                id: realmCardContent

                anchors {
                    fill: parent
                    margins: 24
                }
                spacing: 18

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 12

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 4

                        Label {
                            Layout.fillWidth: true
                            text: root.controller.displayName || qsTr("No Realm selected")
                            font.pixelSize: 20
                            font.weight: Font.DemiBold
                            elide: Text.ElideMiddle
                        }

                        Label {
                            Layout.fillWidth: true
                            text: root.controller.bundlePath || qsTr("Choose the bundle created by the macOS installer.")
                            color: root.palette.placeholderText
                            elide: Text.ElideMiddle
                        }
                    }

                    Button {
                        text: qsTr("Choose Bundle…")
                        enabled: root.controller.canSelectBundle
                        onClicked: bundleDialog.open()
                    }

                    Button {
                        text: root.controller.displayWindow ? qsTr("Show Display") : qsTr("Open Display")
                        enabled: root.controller.bundlePath.length > 0
                        onClicked: root.displayRequested()
                    }
                }

                Rectangle {
                    Layout.fillWidth: true
                    Layout.preferredHeight: 1
                    color: root.palette.mid
                }

                RowLayout {
                    Layout.fillWidth: true
                    spacing: 10

                    BusyIndicator {
                        Layout.preferredWidth: 22
                        Layout.preferredHeight: 22
                        running: root.controller.busy
                        visible: running
                    }

                    Label {
                        Layout.fillWidth: true
                        text: root.controller.stateText
                        font.weight: Font.Medium
                    }

                    PrimaryButton {
                        text: qsTr("Start")
                        enabled: root.controller.canStart
                        onClicked: root.controller.start()
                    }

                    Button {
                        text: root.controller.canResume ? qsTr("Resume") : qsTr("Pause")
                        enabled: root.controller.canPause || root.controller.canResume
                        onClicked: {
                            if (root.controller.canResume)
                                root.controller.resume();
                            else
                                root.controller.pause();
                        }
                    }

                    Button {
                        text: qsTr("Shut Down")
                        enabled: root.controller.canRequestStop
                        onClicked: root.controller.requestStop()
                    }

                    IconButton {
                        id: moreButton

                        icon.source: "qrc:///icons/phosphor/dots-three-circle.svg"
                        toolTipText: qsTr("More Realm actions")
                        enabled: root.controller.canForceStop
                        onClicked: realmActions.open()

                        Menu {
                            id: realmActions

                            y: moreButton.height

                            MenuItem {
                                text: qsTr("Force Stop…")
                                enabled: root.controller.canForceStop
                                onTriggered: forceStopDialog.open()
                            }
                        }
                    }
                }
            }
        }

        Rectangle {
            Layout.fillWidth: true
            implicitHeight: errorLayout.implicitHeight + 24
            radius: 10
            color: Qt.rgba(0.82, 0.12, 0.16, 0.08)
            border.color: Qt.rgba(0.82, 0.12, 0.16, 0.24)
            visible: root.controller.errorMessage.length > 0

            RowLayout {
                id: errorLayout

                anchors {
                    fill: parent
                    margins: 12
                }
                spacing: 12

                Label {
                    Layout.fillWidth: true
                    text: root.controller.errorMessage
                    color: "#B4232A"
                    wrapMode: Text.WordWrap
                }

                IconButton {
                    icon.source: "qrc:///icons/phosphor/x-circle.svg"
                    toolTipText: qsTr("Dismiss error")
                    onClicked: root.controller.clearError()
                }
            }
        }

        Label {
            Layout.fillWidth: true
            text: qsTr("Pausing stops guest execution but keeps its memory allocated. Shut Down releases the virtual machine resources after macOS exits.")
            color: root.palette.placeholderText
            wrapMode: Text.WordWrap
        }

        Item {
            Layout.fillHeight: true
        }
    }
}
