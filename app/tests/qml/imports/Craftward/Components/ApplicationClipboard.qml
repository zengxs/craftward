// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

pragma Singleton

import QtQml

QtObject {
    property string lastCopiedText
    property int copyCount: 0

    function copyText(text) {
        lastCopiedText = String(text);
        ++copyCount;
        return true;
    }

    function reset() {
        lastCopiedText = "";
        copyCount = 0;
    }
}
