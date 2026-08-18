// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    id: root

    property string currentName
    property string draft
    property bool renameAllowed: true
    readonly property string normalizedName: draft.trim()
    readonly property bool canSubmit: renameAllowed && normalizedName.length > 0 && normalizedName !== currentName.trim()

    signal renameRequested(string name)

    function reset(name) {
        currentName = name;
        draft = name;
    }

    function submit() {
        if (!canSubmit)
            return false;
        renameRequested(normalizedName);
        return true;
    }
}
