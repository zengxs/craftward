// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

.pragma library

function standaloneActivityRow(overrides) {
    const row = {
        entryId: "activity:turn-1:compaction-1",
        turnId: "turn-1",
        turnForkable: false,
        latestTurn: true,
        activityGroup: true,
        fromUser: false,
        finalAnswer: false,
        detailRow: false,
        firstDetailInTurn: false,
        detailCountInTurn: 0,
        standaloneActivity: true,
        activityLabel: "Context compacted",
        activityPresentationKind: "contextCompaction",
        activityCount: 1,
        failed: false,
        running: true
    };
    if (overrides) {
        for (const key in overrides)
            row[key] = overrides[key];
    }
    return row;
}
