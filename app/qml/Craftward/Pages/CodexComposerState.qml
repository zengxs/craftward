// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    id: root

    property int capacity: 20
    property string draft
    property string threadId
    property string activeThreadId
    property var drafts: []

    signal editorShouldLoseFocus

    onCapacityChanged: trimDrafts()
    onThreadIdChanged: activateThread()

    function draftCount() {
        return root.drafts.length;
    }

    function confirmTurnStarted() {
        root.saveDraft("");
    }

    function saveDraft(text) {
        if (root.activeThreadId !== root.threadId)
            root.activateThread();

        const normalizedText = String(text);
        if (root.draft !== normalizedText)
            root.draft = normalizedText;
        root.removeDraft(root.activeThreadId);
        if (root.activeThreadId.length === 0 || normalizedText.length === 0)
            return;

        root.drafts.push({
            "threadId": root.activeThreadId,
            "text": normalizedText
        });
        root.trimDrafts();
    }

    function activateThread() {
        if (root.activeThreadId === root.threadId)
            return;

        root.activeThreadId = root.threadId;
        const index = root.draftIndex(root.activeThreadId);
        let restoredDraft = "";
        if (index >= 0) {
            const entry = root.drafts.splice(index, 1)[0];
            root.drafts.push(entry);
            restoredDraft = entry.text;
        }
        root.draft = restoredDraft;
        root.editorShouldLoseFocus();
    }

    function draftIndex(threadId) {
        for (let index = 0; index < root.drafts.length; ++index) {
            if (root.drafts[index].threadId === threadId)
                return index;
        }
        return -1;
    }

    function removeDraft(threadId) {
        const index = root.draftIndex(threadId);
        if (index >= 0)
            root.drafts.splice(index, 1);
    }

    function trimDrafts() {
        const limit = Math.max(0, Math.floor(root.capacity));
        while (root.drafts.length > limit)
            root.drafts.shift();
    }
}
