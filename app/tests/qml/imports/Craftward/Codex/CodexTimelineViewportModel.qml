// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml.Models

ListModel {
    id: root

    property var sourceModel
    property var rows: []
    property int viewportRevision: 0
    readonly property int sourceRevision: sourceModel ? Number(sourceModel.revision) : 0
    readonly property int revision: sourceRevision + viewportRevision
    readonly property int totalRowCount: count

    function rebuild() {
        const nextRows = [];
        const sourceRowCount = sourceModel ? Number(sourceModel.totalRowCount) : 0;
        for (let sourceRow = 0; sourceRow < sourceRowCount; ++sourceRow) {
            const sourceEntryId = String(sourceModel.entryIdAt(sourceRow));
            const document = sourceModel.valueAt(sourceRow, "markupDocument");
            const renderModel = document ? document["renderModel"] : null;
            const detailRow = Boolean(sourceModel.valueAt(sourceRow, "detailRow"));
            const turnExpanded = Boolean(sourceModel.valueAt(sourceRow, "turnExpanded"));
            const blockCount = renderModel ? Number(renderModel.count) : 0;
            if (!renderModel || blockCount === 0 || (detailRow && !turnExpanded)) {
                nextRows.push({
                    entryId: sourceEntryId,
                    sourceRow: sourceRow,
                    blockRow: -1
                });
                continue;
            }
            for (let blockRow = 0; blockRow < blockCount; ++blockRow) {
                const block = renderModel.get(blockRow);
                const blockId = String(block.segmentId ?? ("row:" + blockRow));
                nextRows.push({
                    entryId: blockRow === 0 ? sourceEntryId : sourceEntryId + "/markup/" + blockId,
                    sourceRow: sourceRow,
                    blockRow: blockRow
                });
            }
        }

        root.rows = nextRows;
        root.clear();
        for (const row of nextRows)
            root.append({
                entryId: row.entryId
            });
        ++root.viewportRevision;
    }

    function entryIdAt(row) {
        const viewportRow = rows[row];
        return viewportRow ? String(viewportRow.entryId) : "";
    }

    function indexOfEntryId(entryId) {
        const target = String(entryId);
        for (let row = 0; row < rows.length; ++row) {
            if (entryIdAt(row) === target)
                return row;
        }
        return -1;
    }

    function valueAt(row, roleName) {
        const viewportRow = rows[row];
        if (!viewportRow || !sourceModel)
            return undefined;
        if (roleName === "entryId")
            return viewportRow.entryId;
        if (roleName === "sourceEntryId")
            return sourceModel.entryIdAt(viewportRow.sourceRow);
        if (roleName === "semanticBlock")
            return viewportRow.blockRow >= 0;
        if (roleName === "blockIndex")
            return viewportRow.blockRow;

        const document = sourceModel.valueAt(viewportRow.sourceRow, "markupDocument");
        const renderModel = document ? document["renderModel"] : null;
        if (viewportRow.blockRow >= 0 && renderModel) {
            const block = renderModel.get(viewportRow.blockRow);
            if (roleName === "blockCount")
                return renderModel.count;
            if (roleName === "firstBlockInEntry")
                return viewportRow.blockRow === 0;
            if (roleName === "lastBlockInEntry")
                return viewportRow.blockRow + 1 === renderModel.count;
            if (roleName === "blockId")
                return block.segmentId;
            if (roleName === "blockText")
                return block.segmentText;
            if (roleName === "codeBlock" || roleName === "language" || roleName === "markdown")
                return block[roleName];
        }
        return sourceModel.valueAt(viewportRow.sourceRow, roleName);
    }

    onSourceModelChanged: rebuild()
    onSourceRevisionChanged: rebuild()
}
