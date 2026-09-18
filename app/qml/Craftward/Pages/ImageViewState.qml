// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    // Each view owns its framing; the resource can be shared by future split views.
    property bool fit: true
    property real zoom: 1
    property real centerX: 0.5
    property real centerY: 0.5
}
