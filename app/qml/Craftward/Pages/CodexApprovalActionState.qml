// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    required property var advertisedDecisions
    required property var defaultDecisions
    readonly property var effectiveDecisions: advertisedDecisions.length > 0 ? advertisedDecisions : defaultDecisions

    function offers(decision) {
        return effectiveDecisions.indexOf(decision) >= 0;
    }
}
