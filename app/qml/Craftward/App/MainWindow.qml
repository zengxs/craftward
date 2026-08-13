// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Craftward.Codex
import Craftward.Pages

ApplicationWindow {
    id: window

    required property CodexHistoryController codexHistoryController

    signal closeWindowRequested
    signal quitRequested
    signal realmManagerRequested
    signal settingsRequested(int pageIndex)

    function present() {
        window.show();
        window.raise();
        window.requestActivate();
    }

    width: 960
    height: 640
    minimumWidth: 640
    minimumHeight: 480
    flags: Qt.Window | Qt.ExpandedClientAreaHint | Qt.NoTitleBarBackgroundHint
    visible: true
    title: qsTr("Craftward")
    topPadding: 0
    leftPadding: 0
    rightPadding: 0
    bottomPadding: 0

    menuBar: MenuBar {
        Menu {
            title: qsTr("File")

            Action {
                text: qsTr("Close Window")
                shortcut: StandardKey.Close
                onTriggered: window.closeWindowRequested()
            }

            MenuSeparator {}

            Action {
                text: qsTr("Manage Realms…")
                onTriggered: window.realmManagerRequested()
            }

            Action {
                text: qsTr("Settings…")
                shortcut: StandardKey.Preferences
                onTriggered: window.settingsRequested(0)
            }

            MenuSeparator {}

            Action {
                text: qsTr("Quit Craftward")
                shortcut: StandardKey.Quit
                onTriggered: window.quitRequested()
            }
        }

        Menu {
            title: qsTr("Help")

            Action {
                text: qsTr("About Craftward")
                onTriggered: window.settingsRequested(1)
            }
        }
    }

    CodexHistoryPage {
        anchors.fill: parent
        controller: window.codexHistoryController
    }
}
