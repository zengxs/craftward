// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQml

QtObject {
    id: root

    property int capacity: 20
    property string draft
    property var attachments: []
    property string threadId
    property string activeThreadId
    property var drafts: []

    signal editorShouldLoseFocus

    onCapacityChanged: trimDrafts()
    onThreadIdChanged: activateThread()

    function draftCount() {
        return root.drafts.length;
    }

    function attachmentUrls() {
        return root.attachments.map(attachment => attachment.url);
    }

    function confirmSubmission() {
        root.attachments = [];
        root.saveDraft("");
    }

    function confirmTextSubmission() {
        root.saveDraft("");
    }

    function addAttachments(candidates) {
        if (root.activeThreadId !== root.threadId)
            root.activateThread();

        const nextAttachments = root.attachments.slice();
        for (let index = 0; index < candidates.length; ++index) {
            const candidate = candidates[index];
            const candidateKey = String(candidate.url);
            if (!nextAttachments.some(attachment => String(attachment.url) === candidateKey))
                nextAttachments.push(candidate);
        }
        root.attachments = nextAttachments;
        root.persistActiveDraft();
    }

    function removeAttachment(index) {
        if (index < 0 || index >= root.attachments.length)
            return;
        const nextAttachments = root.attachments.slice();
        nextAttachments.splice(index, 1);
        root.attachments = nextAttachments;
        root.persistActiveDraft();
    }

    function saveDraft(text) {
        if (root.activeThreadId !== root.threadId)
            root.activateThread();

        const normalizedText = String(text);
        if (root.draft !== normalizedText)
            root.draft = normalizedText;
        root.persistActiveDraft();
    }

    function persistActiveDraft() {
        root.removeDraft(root.activeThreadId);
        if (root.activeThreadId.length === 0 || (root.draft.length === 0 && root.attachments.length === 0))
            return;

        root.drafts.push({
            "threadId": root.activeThreadId,
            "text": root.draft,
            "attachments": root.attachments.slice()
        });
        root.trimDrafts();
    }

    function activateThread() {
        if (root.activeThreadId === root.threadId)
            return;

        root.activeThreadId = root.threadId;
        const index = root.draftIndex(root.activeThreadId);
        let restoredDraft = "";
        let restoredAttachments = [];
        if (index >= 0) {
            const entry = root.drafts.splice(index, 1)[0];
            root.drafts.push(entry);
            restoredDraft = entry.text;
            restoredAttachments = entry.attachments ? entry.attachments.slice() : [];
        }
        root.attachments = restoredAttachments;
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
