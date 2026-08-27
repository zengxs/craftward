// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexconversationcontroller.h"

#include <QtTranslation>

#include <utility>

bool
CodexConversationController::applyHistoryEvent(ward::codex::v1::HistoryEvent& event)
{
    using HistoryEventKind = ward::codex::v1::HistoryEventKindGadget::HistoryEventKind;

    const QString threadId = event.hasThreadId() ? event.threadId() : QString();
    switch (event.kind()) {
        case HistoryEventKind::HISTORY_EVENT_KIND_MODEL_CATALOG_UPDATED: {
            if (!event.hasModelCatalog()) {
                finishModelCatalogLoading(/*% "Ward Core returned a model update without a catalog." */ qtTrId(
                  "craftward.codex.error.invalid_event.model_catalog_missing"));
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
            finishModelCatalogLoading(message.isEmpty() ? /*% "The Codex model catalog could not be loaded." */ qtTrId(
                                                            "craftward.codex.error.model_catalog_load")
                                                        : message);
            return true;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_MODEL_CHANGED: {
            if (threadId.isEmpty() || !event.hasThreadModelState() ||
                event.threadModelState().model().trimmed().isEmpty()) {
                if (threadId.isEmpty() || threadId == threadId_)
                    setErrorMessage(/*% "Ward Core returned a thread model update without a model." */ qtTrId(
                      "craftward.codex.error.invalid_event.thread_model_missing"));
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
                finishLoading(/*% "Ward Core returned a conversation update without a conversation." */ qtTrId(
                  "craftward.codex.error.invalid_event.conversation_missing"));
                return true;
            }
            const auto& conversation = event.conversation();
            if (!conversation.title().trimmed().isEmpty())
                updateTitle(conversation.title());
            auto timeline = conversation.timeline();
            timelineModel_.reconcileTimeline(
              std::move(timeline), conversation.forkableTurnIds(), conversation.turnTimings());
            setActivityHistoryPartial(conversation.activityHistoryIsPartial());
            applyPersistedTurnState(conversation);
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
            finishLoading(message.isEmpty() ? /*% "The Codex conversation could not be observed." */ qtTrId(
                                                "craftward.codex.error.conversation_observe")
                                            : message);
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
            setErrorMessage(message.isEmpty()
                              ? /*% "The Codex turn could not be guided." */ qtTrId("craftward.codex.error.turn_steer")
                              : message);
            return true;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_TURN_ERROR: {
            if (threadId != threadId_)
                return true;
            setSteeringTurn(false);
            const QString message = event.hasErrorMessage() ? event.errorMessage() : QString();
            setErrorMessage(message.isEmpty()
                              ? /*% "The Codex turn failed." */ qtTrId("craftward.codex.error.turn_failed")
                              : message);
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
                pendingContinuationThreadId_.clear();
                setWriteAvailability(WriteAvailability::Unavailable,
                                     /*% "Ward Core returned a write-state update without a state." */ qtTrId(
                                       "craftward.codex.error.invalid_event.write_state_missing"));
                return true;
            }
            const auto& state = event.threadWriteState();
            const QString message = state.hasMessage() ? state.message() : QString();
            using ThreadWriteStatus = ward::codex::v1::ThreadWriteStatusGadget::ThreadWriteStatus;
            switch (state.status()) {
                case ThreadWriteStatus::THREAD_WRITE_STATUS_IDLE:
                    pendingContinuationThreadId_.clear();
                    setWriteAvailability(WriteAvailability::NotRequested);
                    break;
                case ThreadWriteStatus::THREAD_WRITE_STATUS_CHECKING:
                    setWriteAvailability(WriteAvailability::Checking, message);
                    break;
                case ThreadWriteStatus::THREAD_WRITE_STATUS_WRITABLE:
                    setWriteAvailability(WriteAvailability::Writable, message);
                    if (pendingContinuationThreadId_ == threadId_) {
                        pendingContinuationThreadId_.clear();
                        dispatchContinuation();
                    }
                    break;
                case ThreadWriteStatus::THREAD_WRITE_STATUS_BUSY:
                    pendingContinuationThreadId_.clear();
                    setWriteAvailability(WriteAvailability::Busy, message);
                    break;
                case ThreadWriteStatus::THREAD_WRITE_STATUS_UNAVAILABLE:
                    pendingContinuationThreadId_.clear();
                    setWriteAvailability(WriteAvailability::Unavailable, message);
                    break;
                case ThreadWriteStatus::THREAD_WRITE_STATUS_UNSPECIFIED:
                default:
                    pendingContinuationThreadId_.clear();
                    setWriteAvailability(WriteAvailability::Unavailable,
                                         /*% "Ward Core returned an unsupported write state." */ qtTrId(
                                           "craftward.codex.error.invalid_event.write_state_unsupported"));
                    break;
            }
            return true;
        }
        case HistoryEventKind::HISTORY_EVENT_KIND_THREAD_RUNTIME_STATE_CHANGED: {
            if (threadId != threadId_)
                return true;
            if (!event.hasThreadRuntimeState()) {
                setTurnState(TurnState::Unknown);
                setErrorMessage(/*% "Ward Core returned a runtime update without a state." */ qtTrId(
                  "craftward.codex.error.invalid_event.runtime_state_missing"));
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
                setErrorMessage(/*% "Ward Core returned an interaction update without its interactions." */ qtTrId(
                  "craftward.codex.error.invalid_event.interactions_missing"));
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
