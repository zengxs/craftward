// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexhistorycontroller.h"

#include <utility>

void
CodexHistoryController::applyHistoryEvent(ward::codex::v1::HistoryEvent event, const QString& decodingError)
{
    using HistoryEventKind = ward::codex::v1::HistoryEventKindGadget::HistoryEventKind;

    if (!decodingError.isEmpty()) {
        finishThreadLoading(decodingError);
        if (!selectedThreadId_.isEmpty())
            finishConversationLoading(decodingError);
        return;
    }

    const QString threadId = event.hasThreadId() ? event.threadId() : QString();
    switch (event.kind()) {
        case HistoryEventKind::HISTORY_EVENT_KIND_THREADS_UPDATED: {
            if (!event.hasThreadPage()) {
                finishThreadLoading(tr("Ward Core returned a thread update without a thread page."));
                break;
            }
            auto threads = event.threadPage().threads();
            threadModel_.reconcileThreads(std::move(threads));
            finishThreadLoading({});
            break;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREADS_RECOVERED:
            finishThreadLoading({});
            break;
        case HistoryEventKind::HISTORY_EVENT_KIND_THREADS_ERROR: {
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            finishThreadLoading(message.isEmpty() ? tr("The Codex conversation list could not be observed.") : message);
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
            timelineModel_.reconcileTimeline(std::move(timeline));
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
            finishConversationLoading(message.isEmpty() ? tr("The Codex conversation could not be observed.")
                                                        : message);
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
        case HistoryEventKind::HISTORY_EVENT_KIND_TURN_ERROR: {
            if (threadId != selectedThreadId_)
                return;
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
