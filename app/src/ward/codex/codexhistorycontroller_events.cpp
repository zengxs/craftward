// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexhistorycontroller.h"

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
                rejectThreadListEvent(tr("Ward Core returned a thread update without its history scope."));
                break;
            }
            if (event.archived() != showingArchived_)
                return;
            if (!event.hasThreadPage()) {
                rejectThreadListEvent(tr("Ward Core returned a thread update without a thread page."));
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
                rejectThreadListEvent(tr("Ward Core returned a thread-list event without its history scope."));
                break;
            }
            if (event.archived() != showingArchived_)
                return;
            if (!changingThreadLifecycle_)
                finishThreadLoading({});
            break;
        case HistoryEventKind::HISTORY_EVENT_KIND_THREADS_ERROR: {
            if (!event.hasArchived()) {
                rejectThreadListEvent(tr("Ward Core returned a thread-list event without its history scope."));
                break;
            }
            if (event.archived() != showingArchived_)
                return;
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            const QString normalizedMessage =
              message.isEmpty() ? tr("The Codex conversation list could not be observed.") : message;
            if (changingThreadLifecycle_)
                setThreadErrorMessage(normalizedMessage);
            else
                finishThreadLoading(normalizedMessage);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED:
            if (threadId.isEmpty() || !event.hasConversation()) {
                setThreadStartErrorMessage(
                  tr("Ward Core returned a started Codex conversation without its initial state."));
                setStartingThread(false);
                break;
            }
            conversationController_.adoptConversation(threadId, event.conversation());
            setThreadStartErrorMessage({});
            setStartingThread(false);
            break;
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_START_ERROR: {
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            setThreadStartErrorMessage(message.isEmpty() ? tr("The Codex conversation could not be started.")
                                                         : message);
            setStartingThread(false);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_FORKED:
            if (!forkingThread_)
                return;
            if (threadId.isEmpty() || !event.hasConversation()) {
                conversationController_.setErrorMessage(
                  tr("Ward Core returned a forked Codex conversation without its initial state."));
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
                  tr("Ward Core returned a thread-fork error without its source thread."));
                setForkingThread(false);
                break;
            }
            if (threadId != pendingForkSourceThreadId_)
                return;
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            conversationController_.setErrorMessage(
              message.isEmpty() ? tr("The Codex conversation could not be forked.") : message);
            setForkingThread(false);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_LIFECYCLE_ERROR: {
            if (threadId != conversationController_.threadId() || threadId != pendingLifecycleThreadId_)
                return;
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            const QString normalizedMessage =
              message.isEmpty() ? tr("The Codex conversation could not be archived or restored.") : message;
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
            const QString message = tr("Ward Core returned an unsupported Codex history event.");
            if (threadId.isEmpty())
                finishThreadLoading(message);
            else if (threadId == conversationController_.threadId())
                conversationController_.finishLoading(message);
            break;
        }
    }
}
