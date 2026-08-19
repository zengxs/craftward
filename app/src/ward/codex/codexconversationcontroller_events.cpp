// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexconversationcontroller.h"

#include <utility>

bool
CodexConversationController::applyHistoryEvent(ward::codex::v1::HistoryEvent& event)
{
    using HistoryEventKind = ward::codex::v1::HistoryEventKindGadget::HistoryEventKind;

    const QString threadId = event.hasThreadId() ? event.threadId() : QString();
    switch (event.kind()) {
        case HistoryEventKind::HISTORY_EVENT_KIND_MODEL_CATALOG_UPDATED: {
            if (!event.hasModelCatalog()) {
                finishModelCatalogLoading(tr("Ward Core returned a model update without a catalog."));
                return true;
            }
            auto models = event.modelCatalog().models();
            modelCatalogModel_.replaceModels(std::move(models));
            reconcileInferenceSelections();
            emit reasoningEffortsChanged();
            finishModelCatalogLoading({});
            return true;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_MODEL_CATALOG_ERROR: {
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            finishModelCatalogLoading(message.isEmpty() ? tr("The Codex model catalog could not be loaded.") : message);
            return true;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_MODEL_CHANGED: {
            if (threadId.isEmpty() || !event.hasThreadModelState() ||
                event.threadModelState().model().trimmed().isEmpty()) {
                if (threadId.isEmpty() || threadId == threadId_)
                    setErrorMessage(tr("Ward Core returned a thread model update without a model."));
                return true;
            }
            const auto& state = event.threadModelState();
            applyThreadInferenceOptions(
              threadId, state.model(), state.hasReasoningEffort() ? state.reasoningEffort() : QString());
            return true;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_UPDATED: {
            if (threadId != threadId_)
                return true;
            if (!event.hasConversation()) {
                finishLoading(tr("Ward Core returned a conversation update without a conversation."));
                return true;
            }
            const auto& conversation = event.conversation();
            if (!conversation.title().trimmed().isEmpty())
                updateTitle(conversation.title());
            auto timeline = conversation.timeline();
            timelineModel_.reconcileTimeline(std::move(timeline), conversation.forkableTurnIds());
            setActivityHistoryPartial(conversation.activityHistoryIsPartial());
            finishLoading({});
            return true;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_RECOVERED:
            if (threadId == threadId_)
                finishLoading({});
            return true;
        case HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_ERROR: {
            if (threadId != threadId_)
                return true;
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            finishLoading(message.isEmpty() ? tr("The Codex conversation could not be observed.") : message);
            return true;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_TURN_STARTED:
            if (threadId == threadId_)
                emit turnStarted();
            return true;
        case HistoryEventKind::HISTORY_EVENT_KIND_TURN_COMPLETED:
            return true;
        case HistoryEventKind::HISTORY_EVENT_KIND_TURN_STEERED:
            if (threadId == threadId_ && steeringTurn_) {
                setSteeringTurn(false);
                emit turnSteered();
            }
            return true;
        case HistoryEventKind::HISTORY_EVENT_KIND_TURN_STEER_ERROR: {
            if (threadId != threadId_)
                return true;
            setSteeringTurn(false);
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            setErrorMessage(message.isEmpty() ? tr("The Codex turn could not be guided.") : message);
            return true;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_TURN_ERROR: {
            if (threadId != threadId_)
                return true;
            setSteeringTurn(false);
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            setErrorMessage(message.isEmpty() ? tr("The Codex turn failed.") : message);
            return true;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_TURN_NOTICE: {
            if (threadId != threadId_)
                return true;
            setInterruptRequested(false);
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            if (!message.isEmpty())
                setErrorMessage(message);
            return true;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_WRITE_STATE_CHANGED: {
            if (threadId != threadId_)
                return true;
            if (!event.hasThreadWriteState()) {
                setWriteAvailability(WriteAvailability::Unavailable,
                                     tr("Ward Core returned a write-state update without a state."));
                return true;
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
            return true;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_RUNTIME_STATE_CHANGED: {
            if (threadId != threadId_)
                return true;
            if (!event.hasThreadRuntimeState()) {
                setTurnState(TurnState::Unknown);
                setErrorMessage(tr("Ward Core returned a runtime update without a state."));
                return true;
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
            return true;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_PENDING_INTERACTIONS_UPDATED: {
            if (threadId != threadId_)
                return true;
            if (!event.hasPendingInteractions()) {
                setErrorMessage(tr("Ward Core returned an interaction update without its interactions."));
                return true;
            }
            auto interactions = event.pendingInteractions().interactions();
            interactionModel_.reconcileInteractions(std::move(interactions));
            return true;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREADS_UPDATED:
        case HistoryEventKind::HISTORY_EVENT_KIND_THREADS_RECOVERED:
        case HistoryEventKind::HISTORY_EVENT_KIND_THREADS_ERROR:
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED:
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_START_ERROR:
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_FORKED:
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_FORK_ERROR:
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_LIFECYCLE_ERROR:
        case HistoryEventKind::HISTORY_EVENT_KIND_UNSPECIFIED:
        default:
            return false;
    }
}
