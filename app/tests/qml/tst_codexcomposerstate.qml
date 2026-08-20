// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

import QtQuick
import QtQuick.Controls
import QtTest
import "../../qml/Craftward/Pages" as Pages

Item {
    id: suite

    width: 320
    height: 180

    property var state
    property int integratedAcquireCount: 0

    function attachment(url, name, kind) {
        return {
            "url": url,
            "name": name,
            "mimeType": kind === "localImage" ? "image/png" : (kind === "localAudio" ? "audio/wav" : "application/pdf"),
            "kind": kind,
            "managed": false
        };
    }

    Pages.CodexComposerState {
        id: integratedState

        capacity: 2
        onDraftChanged: {
            if (promptEditor.text !== draft)
                promptEditor.text = draft;
        }
        onEditorShouldLoseFocus: promptEditor.focus = false
    }

    ItemDelegate {
        id: threadDelegate

        width: 160
        height: 40
        text: "Thread 2"
        onClicked: integratedState.threadId = "integrated-thread-2"
    }

    TextArea {
        id: promptEditor

        y: 50
        width: 320
        height: 120
        onTextChanged: integratedState.saveDraft(text)
        onActiveFocusChanged: {
            if (activeFocus)
                suite.integratedAcquireCount += 1;
        }
    }

    Component {
        id: stateComponent

        Pages.CodexComposerState {
            capacity: 2
        }
    }

    SignalSpy {
        id: focusSpy

        signalName: "editorShouldLoseFocus"
    }

    TestCase {
        name: "CodexComposerState"
        when: windowShown

        function init() {
            suite.state = stateComponent.createObject(suite);
            verify(suite.state !== null);
            focusSpy.target = suite.state;
            focusSpy.clear();
            integratedState.threadId = "";
            promptEditor.clear();
            threadDelegate.forceActiveFocus();
            suite.integratedAcquireCount = 0;
        }

        function cleanup() {
            focusSpy.target = null;
            suite.state.destroy();
            suite.state = null;
        }

        function test_draftsFollowTheirConversation() {
            suite.state.threadId = "thread-1";
            suite.state.saveDraft("First draft");
            suite.state.threadId = "thread-2";
            compare(suite.state.draft, "");

            suite.state.saveDraft("Second draft");
            suite.state.threadId = "thread-1";
            compare(suite.state.draft, "First draft");
            suite.state.threadId = "thread-2";
            compare(suite.state.draft, "Second draft");
        }

        function test_draftIsClearedOnlyAfterSubmissionIsConfirmed() {
            suite.state.threadId = "thread-1";
            suite.state.saveDraft("Continue from here");
            suite.state.addAttachments([suite.attachment("file:///workspace/screenshot.png", "screenshot.png", "localImage")]);

            compare(suite.state.draft, "Continue from here");
            compare(suite.state.attachments.length, 1);
            compare(suite.state.draftCount(), 1);
            suite.state.confirmSubmission();
            compare(suite.state.draft, "");
            compare(suite.state.attachments.length, 0);
            compare(suite.state.draftCount(), 0);
        }

        function test_attachmentsFollowTheirConversationWithoutDuplicates() {
            const firstImage = suite.attachment("file:///workspace/first.png", "first.png", "localImage");
            const requirements = suite.attachment("file:///workspace/requirements.pdf", "requirements.pdf", "mention");
            suite.state.threadId = "thread-1";
            suite.state.addAttachments([firstImage, firstImage, requirements]);
            compare(suite.state.attachments.length, 2);
            compare(suite.state.draftCount(), 1);
            compare(suite.state.attachmentUrls(), ["file:///workspace/first.png", "file:///workspace/requirements.pdf"]);

            suite.state.threadId = "thread-2";
            compare(suite.state.attachments.length, 0);
            suite.state.addAttachments([suite.attachment("file:///workspace/note.wav", "note.wav", "localAudio")]);

            suite.state.threadId = "thread-1";
            compare(suite.state.attachments.length, 2);
            compare(String(suite.state.attachments[0].url), "file:///workspace/first.png");
            compare(suite.state.attachments[0].kind, "localImage");
            suite.state.removeAttachment(0);
            compare(suite.state.attachments.length, 1);
            compare(String(suite.state.attachments[0].url), "file:///workspace/requirements.pdf");

            suite.state.threadId = "thread-2";
            compare(suite.state.attachments.length, 1);
            compare(String(suite.state.attachments[0].url), "file:///workspace/note.wav");
        }

        function test_confirmedGuidanceKeepsAttachmentsForTheNextTurn() {
            suite.state.threadId = "thread-1";
            suite.state.saveDraft("Guide the active turn");
            suite.state.addAttachments([suite.attachment("file:///workspace/next-turn.pdf", "next-turn.pdf", "mention")]);

            suite.state.confirmTextSubmission();

            compare(suite.state.draft, "");
            compare(suite.state.attachments.length, 1);
            compare(String(suite.state.attachments[0].url), "file:///workspace/next-turn.pdf");
            compare(suite.state.draftCount(), 1);
        }

        function test_emptyDraftsAreRemoved() {
            suite.state.threadId = "thread-1";
            suite.state.saveDraft("Temporary draft");
            compare(suite.state.draftCount(), 1);

            suite.state.saveDraft("");
            compare(suite.state.draftCount(), 0);
        }

        function test_editorRechecksAfterConversationSwitch() {
            integratedState.threadId = "integrated-thread-1";
            promptEditor.text = "First draft";
            promptEditor.forceActiveFocus();
            tryCompare(suite, "integratedAcquireCount", 1);

            mouseClick(threadDelegate);
            compare(integratedState.threadId, "integrated-thread-2");
            compare(promptEditor.text, "");
            verify(!promptEditor.activeFocus);

            mouseClick(promptEditor);
            tryCompare(suite, "integratedAcquireCount", 2);
            promptEditor.text = "Second draft";
            integratedState.threadId = "integrated-thread-1";
            compare(promptEditor.text, "First draft");
        }

        function test_leastRecentlyUsedDraftIsEvictedAtCapacity() {
            suite.state.threadId = "thread-1";
            suite.state.saveDraft("First draft");
            suite.state.threadId = "thread-2";
            suite.state.saveDraft("Second draft");
            suite.state.threadId = "thread-1";
            suite.state.threadId = "thread-3";
            suite.state.saveDraft("Third draft");

            compare(suite.state.draftCount(), 2);
            suite.state.threadId = "thread-2";
            compare(suite.state.draft, "");
            suite.state.threadId = "thread-1";
            compare(suite.state.draft, "First draft");
            suite.state.threadId = "thread-3";
            compare(suite.state.draft, "Third draft");
        }

        function test_switchingConversationRequestsFocusReset() {
            suite.state.threadId = "thread-1";
            compare(focusSpy.count, 1);

            suite.state.threadId = "thread-1";
            compare(focusSpy.count, 1);
            suite.state.threadId = "thread-2";
            compare(focusSpy.count, 2);
        }
    }
}
