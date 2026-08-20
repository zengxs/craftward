// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import Craftward.Features.Realm
import Craftward.Realm

MainWindow {
    id: root

    property url applicationIconSource
    property string buildNumber
    property string commitHash
    required property ApplicationController applicationController
    required property RealmController realmController

    readonly property RealmManagerWindow realmManagerWindow: RealmManagerWindow {
        controller: root.realmController
        onDisplayRequested: root.realmDisplayWindow.present()
    }

    readonly property RealmDisplayWindow realmDisplayWindow: RealmDisplayWindow {
        controller: root.realmController
    }

    readonly property SettingsWindow settingsWindow: SettingsWindow {
        applicationIconSource: root.applicationIconSource
        buildNumber: root.buildNumber
        commitHash: root.commitHash
        transientParent: null
    }

    onBringAllWindowsToFrontRequested: root.applicationController.requestBringAllWindowsToFront()
    onCloseWindowRequested: root.applicationController.requestCloseActiveWindow()
    onMinimizeActiveWindowRequested: root.applicationController.requestMinimizeActiveWindow()
    onQuitRequested: root.applicationController.requestQuit()
    onRealmManagerRequested: root.realmManagerWindow.present()
    onSettingsRequested: pageIndex => root.settingsWindow.present(pageIndex)
    onZoomActiveWindowRequested: root.applicationController.requestZoomActiveWindow()

    Connections {
        target: root.applicationController

        function onQuitBlocked() {
            root.realmManagerWindow.presentQuitBlocked();
        }

        function onReopenRequested() {
            root.present();
        }
    }
}
