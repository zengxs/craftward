// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexhistorycontroller.h"

#include <QtTranslation>

#include <algorithm>
#include <utility>

void
CodexHistoryController::applyHistoryEvent(ward::codex::v1::HistoryEvent event, const QString& decodingError)
{
    using HistoryEventKind = ward::codex::v1::HistoryEventKindGadget::HistoryEventKind;

    if (!decodingError.isEmpty()) {
        conversationController_.applyDecodingError(decodingError);
        if (startingThread_) {
            setThreadStartErrorMessage(decodingError);
            setStartingThread(false);
        }
        if (forkingThread_) {
            setForkingThread(false);
            conversationController_.setErrorMessage(decodingError);
        }
        if (changingThreadLifecycle_)
            setThreadErrorMessage(decodingError);
        else
            finishThreadLoading(decodingError);
        return;
    }

    if (conversationController_.applyHistoryEvent(event))
        return;

    const QString threadId = event.hasThreadId() ? event.threadId() : QString();
    const auto rejectThreadListEvent = [this](const QString& message) {
        if (changingThreadLifecycle_)
            setThreadErrorMessage(message);
        else
            finishThreadLoading(message);
    };
    switch (event.kind()) {
        case HistoryEventKind::HISTORY_EVENT_KIND_THREADS_UPDATED: {
            if (!event.hasArchived()) {
                rejectThreadListEvent(/*% "Ward Core returned a thread update without its history scope." */ qtTrId(
                  "craftward.codex.error.invalid_event.thread_scope_missing"));
                break;
            }
            if (event.archived() != showingArchived_)
                return;
            if (!event.hasThreadPage()) {
                rejectThreadListEvent(/*% "Ward Core returned a thread update without a thread page." */ qtTrId(
                  "craftward.codex.error.invalid_event.thread_page_missing"));
                break;
            }
            const auto& page = event.threadPage();
            auto threads = page.threads();
            const auto containsThread = [&threads](const QString& candidate) {
                return std::any_of(threads.cbegin(), threads.cend(), [&candidate](const auto& thread) {
                    return thread.threadId() == candidate;
                });
            };
            const bool selectedThreadRemains =
              conversationController_.threadId().isEmpty() || containsThread(conversationController_.threadId());
            const bool lifecycleTargetRemains = !changingThreadLifecycle_ || containsThread(pendingLifecycleThreadId_);
            threadModel_.reconcileThreads(std::move(threads));
            setThreadErrorMessage({});
            if (!selectedThreadRemains)
                clearSelection();
            if (!lifecycleTargetRemains)
                setChangingThreadLifecycle(false);
            if (changingThreadLifecycle_)
                break;
            finishThreadLoading({});
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREADS_RECOVERED:
            if (!event.hasArchived()) {
                rejectThreadListEvent(/*% "Ward Core returned a thread-list event without its history scope." */ qtTrId(
                  "craftward.codex.error.invalid_event.thread_list_scope_missing"));
                break;
            }
            if (event.archived() != showingArchived_)
                return;
            if (!changingThreadLifecycle_)
                finishThreadLoading({});
            break;
        case HistoryEventKind::HISTORY_EVENT_KIND_THREADS_ERROR: {
            if (!event.hasArchived()) {
                rejectThreadListEvent(/*% "Ward Core returned a thread-list event without its history scope." */ qtTrId(
                  "craftward.codex.error.invalid_event.thread_list_scope_missing"));
                break;
            }
            if (event.archived() != showingArchived_)
                return;
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            const QString normalizedMessage = message.isEmpty()
                                                ? /*% "The Codex conversation list could not be observed." */ qtTrId(
                                                    "craftward.codex.error.conversation_list_observe")
                                                : message;
            if (changingThreadLifecycle_)
                setThreadErrorMessage(normalizedMessage);
            else
                finishThreadLoading(normalizedMessage);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED:
            if (threadId.isEmpty() || !event.hasConversation()) {
                setThreadStartErrorMessage(
                  /*% "Ward Core returned a started Codex conversation without its initial state." */ qtTrId(
                    "craftward.codex.error.invalid_event.started_conversation_missing"));
                setStartingThread(false);
                break;
            }
            conversationController_.adoptConversation(threadId, event.conversation());
            setThreadStartErrorMessage({});
            setStartingThread(false);
            break;
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_START_ERROR: {
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            setThreadStartErrorMessage(message.isEmpty() ? /*% "The Codex conversation could not be started." */ qtTrId(
                                                             "craftward.codex.error.conversation_start")
                                                         : message);
            setStartingThread(false);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_FORKED:
            if (!forkingThread_)
                return;
            if (threadId.isEmpty() || !event.hasConversation()) {
                conversationController_.setErrorMessage(
                  /*% "Ward Core returned a forked Codex conversation without its initial state." */ qtTrId(
                    "craftward.codex.error.invalid_event.forked_conversation_missing"));
                setForkingThread(false);
                break;
            }
            conversationController_.adoptConversation(threadId, event.conversation());
            setForkingThread(false);
            break;
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_FORK_ERROR: {
            if (!forkingThread_)
                return;
            if (threadId.isEmpty()) {
                conversationController_.setErrorMessage(
                  /*% "Ward Core returned a thread-fork error without its source thread." */ qtTrId(
                    "craftward.codex.error.invalid_event.fork_source_missing"));
                setForkingThread(false);
                break;
            }
            if (threadId != pendingForkSourceThreadId_)
                return;
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            conversationController_.setErrorMessage(
              message.isEmpty()
                ? /*% "The Codex conversation could not be forked." */ qtTrId("craftward.codex.error.conversation_fork")
                : message);
            setForkingThread(false);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_LIFECYCLE_ERROR: {
            if (threadId != conversationController_.threadId() || threadId != pendingLifecycleThreadId_)
                return;
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            const QString normalizedMessage =
              message.isEmpty() ? /*% "The Codex conversation could not be archived or restored." */ qtTrId(
                                    "craftward.codex.error.conversation_lifecycle")
                                : message;
            if (changingThreadLifecycle_) {
                setChangingThreadLifecycle(false);
                const bool wasLoadingThreads = std::exchange(loadingThreads_, false);
                conversationController_.setErrorMessage(normalizedMessage);
                if (wasLoadingThreads)
                    emit loadingChanged();
            }
            conversationController_.finishLoading(normalizedMessage);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_UNSPECIFIED:
        default: {
            const QString message = /*% "Ward Core returned an unsupported Codex history event." */ qtTrId(
              "craftward.codex.error.invalid_event.unsupported");
            if (threadId.isEmpty())
                finishThreadLoading(message);
            else if (threadId == conversationController_.threadId())
                conversationController_.finishLoading(message);
            break;
        }
    }
}
