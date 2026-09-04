// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml.Models

ListModel {
    id: root

    property var sourceModel
    property var expandedTurns: ({})
    property int expansionRevision: 0
    readonly property int sourceRevision: sourceModel ? Number(sourceModel.revision) : 0
    readonly property int revision: sourceRevision + expansionRevision
    readonly property int totalRowCount: count

    function rebuild() {
        clear();
        if (!sourceModel)
            return;
        const declaredCount = sourceModel && sourceModel["totalRowCount"] !== undefined ? Number(sourceModel["totalRowCount"]) : Number.NaN;
        const sourceRowCount = Number.isFinite(declaredCount) ? declaredCount : (sourceModel.rows ? sourceModel.rows.length : 0);
        for (let sourceRow = 0; sourceRow < sourceRowCount; ++sourceRow) {
            const detail = Boolean(sourceModel.valueAt(sourceRow, "detailRow"));
            const firstDetail = Boolean(sourceModel.valueAt(sourceRow, "firstDetailInTurn"));
            const turnId = String(sourceModel.valueAt(sourceRow, "turnId") ?? "");
            if (!detail || firstDetail || root.expandedTurns[turnId] === true) {
                append({
                    entryId: String(sourceModel.valueAt(sourceRow, "entryId") ?? ""),
                    sourceRow: sourceRow
                });
            }
        }
    }

    function sourceRowAt(row) {
        return row >= 0 && row < count ? Number(get(row).sourceRow) : -1;
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
        return row >= 0 && row < count ? String(get(row).entryId) : "";
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
        rebuild();
    }

    function clearExpandedTurns() {
        expandedTurns = {};
        ++expansionRevision;
        rebuild();
    }

    onSourceModelChanged: rebuild()
    onSourceRevisionChanged: rebuild()
}
