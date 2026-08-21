// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Craftward.Components
import Craftward.Realm

ApplicationWindow {
    id: root

    required property RealmController controller

    signal displayRequested

    function present() {
        root.show();
        root.raise();
        root.requestActivate();
    }

    function presentQuitBlocked() {
        root.present();
        Qt.callLater(quitBlockedDialog.open);
    }

    width: 900
    height: 620
    minimumWidth: 680
    minimumHeight: 480
    transientParent: null
    visible: false
    flags: Qt.Window | Qt.ExpandedClientAreaHint | Qt.NoTitleBarBackgroundHint
    title: /*% "Realm Manager" */ qsTrId("craftward.realm.manager.title")
    topPadding: 0
    leftPadding: 0
    rightPadding: 0
    bottomPadding: 0

    RealmPage {
        anchors.fill: parent
        controller: root.controller
        onDisplayRequested: root.displayRequested()
    }

    ConfirmationDialog {
        id: quitBlockedDialog

        title: /*% "Stop the Realm before quitting" */ qsTrId("craftward.realm.quit_blocked.title")
        message: /*% "Suspend or shut down the Realm before quitting Craftward. If the guest does not respond, use Force Stop from the actions menu." */ qsTrId("craftward.realm.quit_blocked.message")
        acceptText: /*% "Continue Working" */ qsTrId("craftward.realm.quit_blocked.continue")
        rejectText: ""
        primaryAction: true
    }
}
