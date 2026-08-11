// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Craftward.Components
import Craftward.Pages
import Craftward.Realm

ApplicationWindow {
    id: window

    property url applicationIconSource
    property string buildNumber
    property string commitHash
    required property RealmController realmController
    property var realmDisplayWindow: null
    property var settingsWindow: null

    function presentRealmDisplay() {
        if (!window.realmDisplayWindow) {
            window.realmDisplayWindow = realmDisplayWindowComponent.createObject(window, {
                "controller": window.realmController
            });
        }

        window.realmDisplayWindow.present();
    }

    function presentSettings(pageIndex) {
        if (!window.settingsWindow) {
            window.settingsWindow = settingsWindowComponent.createObject(window, {
                "applicationIconSource": window.applicationIconSource,
                "buildNumber": window.buildNumber,
                "commitHash": window.commitHash
            });
        }

        window.settingsWindow.present(pageIndex);
    }

    width: 960
    height: 640
    minimumWidth: 640
    minimumHeight: 480
    flags: Qt.Window | Qt.ExpandedClientAreaHint | Qt.NoTitleBarBackgroundHint
    visible: true
    title: qsTr("Craftward")
    onClosing: close => {
        if (window.realmController.requiresStopBeforeExit) {
            close.accepted = false;
            realmRunningDialog.open();
        }
    }

    menuBar: MenuBar {
        Menu {
            title: qsTr("File")

            Action {
                text: qsTr("Settings…")
                shortcut: StandardKey.Preferences
                onTriggered: window.presentSettings(0)
            }

            MenuSeparator {}

            Action {
                text: qsTr("Quit Craftward")
                shortcut: StandardKey.Quit
                onTriggered: Qt.quit()
            }
        }

        Menu {
            title: qsTr("Help")

            Action {
                text: qsTr("About Craftward")
                onTriggered: window.presentSettings(1)
            }
        }
    }

    background: Rectangle {
        color: window.palette.window

        WindowMoveHandler {
            targetWindow: window
        }
    }

    StackView {
        id: stackView

        anchors.fill: parent
        initialItem: RealmPage {
            controller: window.realmController
            onDisplayRequested: window.presentRealmDisplay()
        }
    }

    Component {
        id: realmDisplayWindowComponent

        RealmDisplayWindow {}
    }

    Component {
        id: settingsWindowComponent

        SettingsWindow {}
    }

    ConfirmationDialog {
        id: realmRunningDialog

        title: qsTr("Shut down the Realm before quitting")
        message: qsTr("Return to the Realm controls and use Suspend or Shut Down. If the guest does not respond, use Force Stop from the actions menu.")
        acceptText: qsTr("Return to Realm")
        rejectText: ""
        primaryAction: true
    }
}
