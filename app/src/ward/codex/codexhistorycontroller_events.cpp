// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexhistorycontroller.h"

#include <algorithm>
#include <utility>

void
CodexHistoryController::adoptConversation(const QString& threadId, const ward::codex::v1::Conversation& conversation)
{
    const bool wasLoading = std::exchange(loadingConversation_, false);
    selectedThreadId_ = threadId;
    selectedThreadTitle_ = conversation.title();
    auto timeline = conversation.timeline();
    timelineModel_.reconcileTimeline(std::move(timeline), conversation.forkableTurnIds());
    interactionModel_.clear();
    setActivityHistoryPartial(conversation.activityHistoryIsPartial());
    setTurnState(TurnState::Detached);
    setWriteAvailability(WriteAvailability::NotRequested);
    setConversationErrorMessage({});
    setInterruptRequested(false);
    emit selectionChanged();
    if (wasLoading)
        emit loadingChanged();
}

void
CodexHistoryController::applyHistoryEvent(ward::codex::v1::HistoryEvent event, const QString& decodingError)
{
    using HistoryEventKind = ward::codex::v1::HistoryEventKindGadget::HistoryEventKind;

    if (!decodingError.isEmpty()) {
        setSteeringTurn(false);
        if (startingThread_) {
            setThreadStartErrorMessage(decodingError);
            setStartingThread(false);
        }
        if (forkingThread_) {
            setForkingThread(false);
            setConversationErrorMessage(decodingError);
        }
        if (changingThreadLifecycle_)
            setThreadErrorMessage(decodingError);
        else
            finishThreadLoading(decodingError);
        if (!selectedThreadId_.isEmpty())
            finishConversationLoading(decodingError);
        return;
    }

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
            const bool selectedThreadRemains = selectedThreadId_.isEmpty() || containsThread(selectedThreadId_);
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
        case HistoryEventKind::HISTORY_EVENT_KIND_THREADS_RECOVERED: {
            if (!event.hasArchived()) {
                rejectThreadListEvent(tr("Ward Core returned a thread-list event without its history scope."));
                break;
            }
            if (event.archived() != showingArchived_)
                return;
            if (!changingThreadLifecycle_)
                finishThreadLoading({});
            break;
        }
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
            if (changingThreadLifecycle_) {
                setThreadErrorMessage(normalizedMessage);
                break;
            }
            finishThreadLoading(normalizedMessage);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED: {
            if (threadId.isEmpty() || !event.hasConversation()) {
                setThreadStartErrorMessage(
                  tr("Ward Core returned a started Codex conversation without its initial state."));
                setStartingThread(false);
                break;
            }

            adoptConversation(threadId, event.conversation());
            setThreadStartErrorMessage({});
            setStartingThread(false);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_START_ERROR: {
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            setThreadStartErrorMessage(message.isEmpty() ? tr("The Codex conversation could not be started.")
                                                         : message);
            setStartingThread(false);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_FORKED: {
            if (!forkingThread_)
                return;
            if (threadId.isEmpty() || !event.hasConversation()) {
                setConversationErrorMessage(
                  tr("Ward Core returned a forked Codex conversation without its initial state."));
                setForkingThread(false);
                break;
            }

            adoptConversation(threadId, event.conversation());
            setForkingThread(false);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_FORK_ERROR: {
            if (!forkingThread_)
                return;
            if (threadId.isEmpty()) {
                setConversationErrorMessage(tr("Ward Core returned a thread-fork error without its source thread."));
                setForkingThread(false);
                break;
            }
            if (threadId != pendingForkSourceThreadId_)
                return;
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            setConversationErrorMessage(message.isEmpty() ? tr("The Codex conversation could not be forked.")
                                                          : message);
            setForkingThread(false);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_UPDATED: {
            if (threadId != selectedThreadId_)
                return;
            if (!event.hasConversation()) {
                finishConversationLoading(tr("Ward Core returned a conversation update without a conversation."));
                break;
            }
            const auto& conversation = event.conversation();
            if (!conversation.title().trimmed().isEmpty() && conversation.title() != selectedThreadTitle_) {
                selectedThreadTitle_ = conversation.title();
                emit selectionChanged();
            }
            auto timeline = conversation.timeline();
            timelineModel_.reconcileTimeline(std::move(timeline), conversation.forkableTurnIds());
            setActivityHistoryPartial(conversation.activityHistoryIsPartial());
            finishConversationLoading({});
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_RECOVERED:
            if (threadId != selectedThreadId_)
                return;
            finishConversationLoading({});
            break;
        case HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_ERROR: {
            if (threadId != selectedThreadId_)
                return;
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            const QString normalizedMessage =
              message.isEmpty() ? tr("The Codex conversation could not be observed.") : message;
            finishConversationLoading(normalizedMessage);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_LIFECYCLE_ERROR: {
            if (threadId != selectedThreadId_ || threadId != pendingLifecycleThreadId_)
                return;
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            const QString normalizedMessage =
              message.isEmpty() ? tr("The Codex conversation could not be archived or restored.") : message;
            if (changingThreadLifecycle_) {
                setChangingThreadLifecycle(false);
                const bool wasLoadingThreads = std::exchange(loadingThreads_, false);
                setConversationErrorMessage(normalizedMessage);
                if (wasLoadingThreads)
                    emit loadingChanged();
            }
            finishConversationLoading(normalizedMessage);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_TURN_STARTED:
            if (threadId != selectedThreadId_)
                return;
            emit turnStarted();
            break;
        case HistoryEventKind::HISTORY_EVENT_KIND_TURN_COMPLETED:
            if (threadId != selectedThreadId_)
                return;
            break;
        case HistoryEventKind::HISTORY_EVENT_KIND_TURN_STEERED:
            if (threadId != selectedThreadId_)
                return;
            if (!steeringTurn_)
                break;
            setSteeringTurn(false);
            emit turnSteered();
            break;
        case HistoryEventKind::HISTORY_EVENT_KIND_TURN_STEER_ERROR: {
            if (threadId != selectedThreadId_)
                return;
            setSteeringTurn(false);
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            setConversationErrorMessage(message.isEmpty() ? tr("The Codex turn could not be guided.") : message);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_TURN_ERROR: {
            if (threadId != selectedThreadId_)
                return;
            setSteeringTurn(false);
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            setConversationErrorMessage(message.isEmpty() ? tr("The Codex turn failed.") : message);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_TURN_NOTICE: {
            if (threadId != selectedThreadId_)
                return;
            setInterruptRequested(false);
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            if (!message.isEmpty())
                setConversationErrorMessage(message);
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_WRITE_STATE_CHANGED: {
            if (threadId != selectedThreadId_)
                return;
            if (!event.hasThreadWriteState()) {
                setWriteAvailability(WriteAvailability::Unavailable,
                                     tr("Ward Core returned a write-state update without a state."));
                break;
            }
            const auto& state = event.threadWriteState();
            const QString message = state.hasMessage() ? state.message() : QString();
            using ThreadWriteStatus = ward::codex::v1::ThreadWriteStatusGadget::ThreadWriteStatus;
            switch (state.status()) {
                case ThreadWriteStatus::THREAD_WRITE_STATUS_IDLE:
                    setWriteAvailability(WriteAvailability::NotRequested);
                    break;
                case ThreadWriteStatus::THREAD_WRITE_STATUS_CHECKING:
                    setWriteAvailability(WriteAvailability::Checking, message);
                    break;
                case ThreadWriteStatus::THREAD_WRITE_STATUS_WRITABLE:
                    setWriteAvailability(WriteAvailability::Writable, message);
                    break;
                case ThreadWriteStatus::THREAD_WRITE_STATUS_BUSY:
                    setWriteAvailability(WriteAvailability::Busy, message);
                    break;
                case ThreadWriteStatus::THREAD_WRITE_STATUS_UNAVAILABLE:
                    setWriteAvailability(WriteAvailability::Unavailable, message);
                    break;
                case ThreadWriteStatus::THREAD_WRITE_STATUS_UNSPECIFIED:
                default:
                    setWriteAvailability(WriteAvailability::Unavailable,
                                         tr("Ward Core returned an unsupported write state."));
                    break;
            }
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_RUNTIME_STATE_CHANGED: {
            if (threadId != selectedThreadId_)
                return;
            if (!event.hasThreadRuntimeState()) {
                setTurnState(TurnState::Unknown);
                setConversationErrorMessage(tr("Ward Core returned a runtime update without a state."));
                break;
            }
            const auto& state = event.threadRuntimeState();
            bool waitingOnApproval = false;
            bool waitingOnUserInput = false;
            using ThreadActiveFlag = ward::codex::v1::ThreadActiveFlagGadget::ThreadActiveFlag;
            for (const auto flag : state.activeFlags()) {
                switch (flag) {
                    case ThreadActiveFlag::THREAD_ACTIVE_FLAG_WAITING_ON_APPROVAL:
                        waitingOnApproval = true;
                        break;
                    case ThreadActiveFlag::THREAD_ACTIVE_FLAG_WAITING_ON_USER_INPUT:
                        waitingOnUserInput = true;
                        break;
                    case ThreadActiveFlag::THREAD_ACTIVE_FLAG_UNSPECIFIED:
                    case ThreadActiveFlag::THREAD_ACTIVE_FLAG_UNKNOWN:
                    default:
                        break;
                }
            }
            const QString activeTurnId = state.hasTurnId() ? state.turnId() : QString();
            using ThreadRuntimeStatus = ward::codex::v1::ThreadRuntimeStatusGadget::ThreadRuntimeStatus;
            switch (state.status()) {
                case ThreadRuntimeStatus::THREAD_RUNTIME_STATUS_DETACHED:
                    setTurnState(TurnState::Detached);
                    break;
                case ThreadRuntimeStatus::THREAD_RUNTIME_STATUS_STARTING:
                    setTurnState(TurnState::Starting);
                    break;
                case ThreadRuntimeStatus::THREAD_RUNTIME_STATUS_IDLE:
                    setTurnState(TurnState::Idle);
                    break;
                case ThreadRuntimeStatus::THREAD_RUNTIME_STATUS_ACTIVE:
                    setTurnState(TurnState::Running, activeTurnId, waitingOnApproval, waitingOnUserInput);
                    break;
                case ThreadRuntimeStatus::THREAD_RUNTIME_STATUS_SYSTEM_ERROR:
                    setTurnState(TurnState::SystemError);
                    break;
                case ThreadRuntimeStatus::THREAD_RUNTIME_STATUS_UNKNOWN:
                case ThreadRuntimeStatus::THREAD_RUNTIME_STATUS_UNSPECIFIED:
                default:
                    setTurnState(TurnState::Unknown);
                    break;
            }
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_PENDING_INTERACTIONS_UPDATED: {
            if (threadId != selectedThreadId_)
                return;
            if (!event.hasPendingInteractions()) {
                setConversationErrorMessage(tr("Ward Core returned an interaction update without its interactions."));
                break;
            }
            auto interactions = event.pendingInteractions().interactions();
            interactionModel_.reconcileInteractions(std::move(interactions));
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_UNSPECIFIED:
        default: {
            const QString message = tr("Ward Core returned an unsupported Codex history event.");
            if (threadId.isEmpty())
                finishThreadLoading(message);
            else if (threadId == selectedThreadId_)
                finishConversationLoading(message);
            break;
        }
    }
}
