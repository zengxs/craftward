// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import Craftward.Design

Popup {
    id: root

    parent: Overlay.overlay
    popupType: Popup.Window
    padding: 18
    background: Rectangle {
        radius: 16
        color: Theme.dark ? TailwindColors.zinc900 : TailwindColors.zinc50
        border.width: 0
    }

    Connections {
        target: root
        function onAboutToShow() {
            const window = root.contentItem ? root.contentItem.Window.window : null;
            const ownerWindow = root.parent ? root.parent.Window.window : null;
            // Enable the system shadow without changing an inline fallback's owner window.
            if (window && window !== ownerWindow)
                window.flags &= ~Qt.NoDropShadowWindowHint;
        }
    }
}
