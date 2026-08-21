// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtQuick.Dialogs
import QtQuick.Layouts
import Craftward.Components
import Craftward.Design
import Craftward.Realm

Page {
    id: root

    required property RealmController controller

    signal displayRequested

    background: Rectangle {
        color: root.palette.window
    }

    WindowMoveHandler {
        targetWindow: root.ApplicationWindow.window
    }

    FolderDialog {
        id: bundleDialog

        title: /*% "Choose an installed Realm bundle" */ qsTrId("craftward.realm.bundle_dialog.title")
        onAccepted: root.controller.bundleUrl = selectedFolder
    }

    ConfirmationDialog {
        id: forceStopDialog

        title: /*% "Force stop this Realm?" */ qsTrId("craftward.realm.force_stop_confirmation.title")
        message: /*% "The guest will not have a chance to shut down. Unsaved data may be lost and its disk may be damaged." */ qsTrId("craftward.realm.force_stop_confirmation.message")
        acceptText: /*% "Force Stop" */ qsTrId("craftward.realm.force_stop_confirmation.action")
        onAccepted: root.controller.forceStop()
    }

    ConfirmationDialog {
        id: discardSavedStateDialog

        title: /*% "Discard this Realm's suspended state?" */ qsTrId("craftward.realm.discard_state_confirmation.title")
        message: /*% "The next start will boot macOS normally. Work that existed only in the guest's memory will be lost." */ qsTrId("craftward.realm.discard_state_confirmation.message")
        acceptText: /*% "Discard State" */ qsTrId("craftward.realm.discard_state_confirmation.action")
        onAccepted: root.controller.discardSavedState()
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
                text: /*% "Realm" */ qsTrId("craftward.realm.name")
                font.pixelSize: 32
                font.weight: Font.DemiBold
            }

            Label {
                Layout.fillWidth: true
                text: /*% "Run an isolated development environment from an installed bundle." */ qsTrId("craftward.realm.description")
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
                            text: root.controller.displayName || /*% "No Realm selected" */ qsTrId("craftward.realm.no_selection")
                            font.pixelSize: 20
                            font.weight: Font.DemiBold
                            elide: Text.ElideMiddle
                        }

                        Label {
                            Layout.fillWidth: true
                            text: root.controller.bundlePath || /*% "Choose the bundle created by the macOS installer." */ qsTrId("craftward.realm.bundle_hint")
                            color: root.palette.placeholderText
                            elide: Text.ElideMiddle
                        }
                    }

                    Button {
                        text: /*% "Choose Bundle…" */ qsTrId("craftward.realm.choose_bundle.action")
                        enabled: root.controller.canSelectBundle
                        onClicked: bundleDialog.open()
                    }

                    Button {
                        text: root.controller.displayWindow ? /*% "Show Display" */ qsTrId("craftward.realm.display.show") : /*% "Open Display" */ qsTrId("craftward.realm.display.open")
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
                        text: root.controller.canRestore ? /*% "Resume" */ qsTrId("craftward.realm.action.resume") : /*% "Start" */ qsTrId("craftward.realm.action.start")
                        enabled: root.controller.canStart || root.controller.canRestore
                        onClicked: {
                            if (root.controller.canRestore)
                                root.controller.restore();
                            else
                                root.controller.start();
                        }
                    }

                    Button {
                        text: root.controller.canResume ? /*% "Resume" */ qsTrId("craftward.realm.action.resume") : /*% "Pause" */ qsTrId("craftward.realm.action.pause")
                        enabled: root.controller.canPause || root.controller.canResume
                        onClicked: {
                            if (root.controller.canResume)
                                root.controller.resume();
                            else
                                root.controller.pause();
                        }
                    }

                    Button {
                        text: /*% "Suspend" */ qsTrId("craftward.realm.action.suspend")
                        enabled: root.controller.canSuspend
                        onClicked: root.controller.suspend()
                    }

                    Button {
                        text: /*% "Shut Down" */ qsTrId("craftward.realm.action.shut_down")
                        enabled: root.controller.canRequestStop
                        onClicked: root.controller.requestStop()
                    }

                    IconButton {
                        id: moreButton

                        icon.source: "qrc:///icons/phosphor/dots-three-circle.svg"
                        toolTipText: /*% "More Realm actions" */ qsTrId("craftward.realm.more_actions")
                        enabled: root.controller.canForceStop || root.controller.canDiscardSavedState
                        onClicked: realmActions.open()

                        Menu {
                            id: realmActions

                            y: moreButton.height

                            MenuItem {
                                text: /*% "Force Stop…" */ qsTrId("craftward.realm.action.force_stop")
                                enabled: root.controller.canForceStop
                                visible: enabled
                                onTriggered: forceStopDialog.open()
                            }

                            MenuItem {
                                text: /*% "Discard Suspended State…" */ qsTrId("craftward.realm.action.discard_state")
                                enabled: root.controller.canDiscardSavedState
                                visible: enabled
                                onTriggered: discardSavedStateDialog.open()
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
            color: Theme.dangerSurface
            border.color: Theme.dangerBorder
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
                    color: Theme.dangerForeground
                    wrapMode: Text.WordWrap
                }

                IconButton {
                    icon.source: "qrc:///icons/phosphor/x-circle.svg"
                    toolTipText: /*% "Dismiss error" */ qsTrId("craftward.error.dismiss")
                    onClicked: root.controller.clearError()
                }
            }
        }

        Label {
            Layout.fillWidth: true
            text: /*% "Pause keeps guest memory allocated. Suspend saves a host-bound runtime state and releases the virtual machine resources. Shut Down exits macOS normally." */ qsTrId("craftward.realm.lifecycle_help")
            color: root.palette.placeholderText
            wrapMode: Text.WordWrap
        }

        Item {
            Layout.fillHeight: true
        }
    }
}
