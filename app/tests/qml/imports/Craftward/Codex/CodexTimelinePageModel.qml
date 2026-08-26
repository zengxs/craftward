// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    property var sourceModel
    property int turnsPerPage: 8
    readonly property int revision: sourceModel ? Number(sourceModel.revision) : 0
    readonly property int totalRowCount: sourceModel ? sourceModel.rows.length : 0
    readonly property int pageCount: totalRowCount > 0 ? 1 : 0

    function pageId(page) {
        return page === 0 ? "page:timeline" : "";
    }

    function pageFirstRow(page) {
        return page === 0 ? 0 : -1;
    }

    function pageRowCount(page) {
        return page === 0 ? totalRowCount : 0;
    }

    function valueAt(sourceRow, roleName) {
        return sourceModel ? sourceModel.valueAt(sourceRow, roleName) : undefined;
    }
}
