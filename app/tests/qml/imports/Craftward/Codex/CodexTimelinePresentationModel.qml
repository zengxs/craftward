// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    id: root

    property var sourceModel
    property var expandedTurns: ({})
    property int expansionRevision: 0
    readonly property int revision: (sourceModel ? Number(sourceModel.revision) : 0) + expansionRevision
    readonly property var visibleSourceRows: {
        const currentRevision = revision;
        const rows = [];
        const declaredCount = sourceModel && sourceModel["totalRowCount"] !== undefined ? Number(sourceModel["totalRowCount"]) : Number.NaN;
        const count = Number.isFinite(declaredCount) ? declaredCount : (sourceModel && sourceModel.rows ? sourceModel.rows.length : 0);
        for (let sourceRow = 0; sourceRow < count; ++sourceRow) {
            const detail = Boolean(sourceModel.valueAt(sourceRow, "detailRow"));
            const firstDetail = Boolean(sourceModel.valueAt(sourceRow, "firstDetailInTurn"));
            const turnId = String(sourceModel.valueAt(sourceRow, "turnId") ?? "");
            if (!detail || firstDetail || root.expandedTurns[turnId] === true)
                rows.push(sourceRow);
        }
        return currentRevision >= 0 ? rows : [];
    }
    readonly property int totalRowCount: visibleSourceRows.length

    function sourceRowAt(row) {
        return row >= 0 && row < visibleSourceRows.length ? visibleSourceRows[row] : -1;
    }

    function valueAt(row, roleName) {
        const sourceRow = sourceRowAt(row);
        if (sourceRow < 0 || !sourceModel)
            return undefined;
        if (roleName === "turnExpanded") {
            const turnId = String(sourceModel.valueAt(sourceRow, "turnId") ?? "");
            return root.expandedTurns[turnId] === true;
        }
        return sourceModel.valueAt(sourceRow, roleName);
    }

    function entryIdAt(row) {
        return String(valueAt(row, "entryId") ?? "");
    }

    function indexOfEntryId(entryId) {
        const target = String(entryId);
        for (let row = 0; row < totalRowCount; ++row) {
            if (entryIdAt(row) === target)
                return row;
        }
        return -1;
    }

    function toggleTurn(turnId) {
        const next = Object.assign({}, expandedTurns);
        const key = String(turnId);
        next[key] = next[key] !== true;
        expandedTurns = next;
        ++expansionRevision;
    }

    function clearExpandedTurns() {
        expandedTurns = {};
        ++expansionRevision;
    }
}
