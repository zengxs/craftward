// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import Craftward.Realm

ApplicationWindow {
    id: root

    required property RealmController controller

    function present() {
        if (!root.controller.attachDisplay())
            return;

        root.show();
        root.raise();
        root.requestActivate();
    }

    width: 1120
    height: 760
    minimumWidth: 720
    minimumHeight: 520
    transientParent: null
    visible: false
    title: root.controller.displayName ? /*% "%1 — Realm Display" */ qsTrId("craftward.realm.display.named_title").arg(root.controller.displayName) : /*% "Realm Display" */ qsTrId("craftward.realm.display.title")
    onClosing: close => {
        close.accepted = false;
        root.hide();
        root.controller.detachDisplay();
    }

    Component.onDestruction: root.controller.detachDisplay()

    Connections {
        target: root.controller

        function onDisplayWindowChanged() {
            if (!root.controller.displayWindow && root.visible)
                root.hide();
        }
    }

    Rectangle {
        anchors.fill: parent
        color: "black"

        WindowContainer {
            anchors.fill: parent
            window: root.controller.displayWindow
        }

        Label {
            anchors.centerIn: parent
            text: /*% "The Realm display is detached." */ qsTrId("craftward.realm.display.detached")
            color: "white"
            visible: !root.controller.displayWindow
        }
    }
}
