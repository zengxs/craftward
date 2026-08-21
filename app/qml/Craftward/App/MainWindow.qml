// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Craftward.Codex
import Craftward.Pages

ApplicationWindow {
    id: window

    required property CodexHistoryController codexHistoryController

    signal bringAllWindowsToFrontRequested
    signal closeWindowRequested
    signal minimizeActiveWindowRequested
    signal quitRequested
    signal realmManagerRequested
    signal settingsRequested(int pageIndex)
    signal zoomActiveWindowRequested

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
    title: /*% "Craftward" */ qsTrId("craftward.app.name")
    topPadding: 0
    leftPadding: 0
    rightPadding: 0
    bottomPadding: 0

    menuBar: MenuBar {
        Menu {
            title: /*% "File" */ qsTrId("craftward.menu.file")

            Action {
                text: /*% "Close Window" */ qsTrId("craftward.window.close")
                shortcut: StandardKey.Close
                onTriggered: window.closeWindowRequested()
            }

            MenuSeparator {}

            Action {
                text: /*% "Manage Realms…" */ qsTrId("craftward.realm.manager.open")
                onTriggered: window.realmManagerRequested()
            }

            Action {
                text: /*% "Settings…" */ qsTrId("craftward.settings.open")
                shortcut: StandardKey.Preferences
                onTriggered: window.settingsRequested(0)
            }

            MenuSeparator {}

            Action {
                text: /*% "Quit Craftward" */ qsTrId("craftward.app.quit")
                shortcut: StandardKey.Quit
                onTriggered: window.quitRequested()
            }
        }

        Menu {
            title: /*% "Window" */ qsTrId("craftward.menu.window")

            Action {
                text: /*% "Minimize" */ qsTrId("craftward.window.minimize")
                shortcut: "Ctrl+M"
                onTriggered: window.minimizeActiveWindowRequested()
            }

            Action {
                text: /*% "Zoom" */ qsTrId("craftward.window.zoom")
                onTriggered: window.zoomActiveWindowRequested()
            }

            MenuSeparator {}

            Action {
                text: /*% "Bring All to Front" */ qsTrId("craftward.window.bring_all_to_front")
                onTriggered: window.bringAllWindowsToFrontRequested()
            }
        }

        Menu {
            title: /*% "Help" */ qsTrId("craftward.menu.help")

            Action {
                text: /*% "Craftward on GitHub" */ qsTrId("craftward.help.github")
                onTriggered: Qt.openUrlExternally("https://github.com/zengxs/craftward")
            }

            Action {
                text: /*% "Report an Issue…" */ qsTrId("craftward.help.report_issue")
                onTriggered: Qt.openUrlExternally("https://github.com/zengxs/craftward/issues/new")
            }

            MenuSeparator {}

            Action {
                text: /*% "About Craftward" */ qsTrId("craftward.about.open")
                onTriggered: window.settingsRequested(1)
            }
        }
    }

    CodexHistoryPage {
        anchors.fill: parent
        controller: window.codexHistoryController
    }
}
