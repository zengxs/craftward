// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexconversationcontroller.h"

#include "ward/codex/codexattachmentinput.h"
#include "ward/codex/codexhistorycontroller.h"
#include "ward/coreffierror.h"

#include <ward_core.h>

#include <QByteArray>
#include <QtProtobuf/QProtobufSerializer>
#include <QtTranslation>

#include <utility>

namespace {
QVariantMap
attachmentDescriptorMap(const CodexAttachmentDescriptor& attachment)
{
    return {
        { QStringLiteral("url"), attachment.url },
        { QStringLiteral("name"), attachment.name },
        { QStringLiteral("mimeType"), attachment.mimeType },
        { QStringLiteral("kind"), CodexAttachmentInput::kindName(attachment.kind) },
        { QStringLiteral("managed"), attachment.managed },
        { QStringLiteral("nameKind"), CodexAttachmentInput::nameKindName(attachment.nameKind) },
    };
}

QVariantList
attachmentDescriptorList(const QList<CodexAttachmentDescriptor>& attachments)
{
    QVariantList described;
    described.reserve(attachments.size());
    for (const CodexAttachmentDescriptor& attachment : attachments)
        described.append(attachmentDescriptorMap(attachment));
    return described;
}

static_assert(static_cast<int>(CodexConversationController::TurnMode::DefaultMode) == WardCodexTurnModeDefault);
static_assert(static_cast<int>(CodexConversationController::TurnMode::PlanMode) == WardCodexTurnModePlan);
static_assert(static_cast<int>(CodexConversationController::PermissionPreset::InheritPermissions) ==
              WardCodexPermissionPresetInherit);
static_assert(static_cast<int>(CodexConversationController::PermissionPreset::RequestApproval) ==
              WardCodexPermissionPresetRequestApproval);
static_assert(static_cast<int>(CodexConversationController::PermissionPreset::ReadOnlyPermissions) ==
              WardCodexPermissionPresetReadOnly);
static_assert(static_cast<int>(CodexAttachmentKind::LocalImage) == WardCodexTurnAttachmentKindLocalImage);
static_assert(static_cast<int>(CodexAttachmentKind::LocalAudio) == WardCodexTurnAttachmentKindLocalAudio);
static_assert(static_cast<int>(CodexAttachmentKind::Mention) == WardCodexTurnAttachmentKindMention);
}

CodexConversationController::CodexConversationController(CodexHistoryController* history, QObject* parent)
  : QObject(parent)
  , history_(history)
  , modelCatalogModel_(this)
  , timelineModel_(this)
  , interactionModel_(this)
{
}

void
CodexConversationController::setObserver(WardCodexHistoryObserver* observer)
{
    observer_ = observer;
}

void
CodexConversationController::setObserverUnavailable(const QString& message)
{
    observer_ = nullptr;
    finishModelCatalogLoading(message);
}

void
CodexConversationController::beginRefresh()
{
    if (!threadId_.isEmpty() && !std::exchange(loading_, true))
        emit loadingChanged();
    const bool catalogStateChanged = !std::exchange(loadingModelCatalog_, true) || !modelCatalogErrorMessage_.isEmpty();
    modelCatalogErrorMessage_.clear();
    if (catalogStateChanged)
        emit modelCatalogStateChanged();
}

void
CodexConversationController::failRefresh(const QString& message)
{
    finishModelCatalogLoading(message);
    if (!threadId_.isEmpty())
        finishLoading(message);
}

void
CodexConversationController::beginLoadingThread(const QString& threadId, const QString& title)
{
    threadId_ = threadId;
    title_ = title;
    restoreInferenceSelection();
    timelineModel_.clear();
    interactionModel_.clear();
    setActivityHistoryPartial(false);
    setTurnState(TurnState::Detached);
    setWriteAvailability(WriteAvailability::NotRequested);
    setErrorMessage({});
    setInterruptRequested(false);
    loading_ = true;
    emit selectionChanged();
    emit loadingChanged();
}

void
CodexConversationController::updateTitle(const QString& title)
{
    if (title_ == title)
        return;
    title_ = title;
    emit selectionChanged();
}

bool
CodexConversationController::forkReady() const
{
    const bool runtimeReady =
      turnRuntimeState_.status == TurnState::Detached || turnRuntimeState_.status == TurnState::Idle;
    const bool writeReady =
      writeAvailability_ == WriteAvailability::NotRequested || writeAvailability_ == WriteAvailability::Writable;
    return runtimeReady && writeReady;
}

void
CodexConversationController::applyDecodingError(const QString& message)
{
    if (loadingModelCatalog_)
        finishModelCatalogLoading(message);
    setSteeringTurn(false);
    if (!threadId_.isEmpty())
        finishLoading(message);
}

QString
CodexConversationController::errorMessage() const
{
    return errorMessage_;
}

CodexModelCatalogModel*
CodexConversationController::modelCatalog()
{
    return &modelCatalogModel_;
}

CodexTimelineModel*
CodexConversationController::timeline()
{
    return &timelineModel_;
}

CodexInteractionModel*
CodexConversationController::interactions()
{
    return &interactionModel_;
}

QString
CodexConversationController::threadId() const
{
    return threadId_;
}

QString
CodexConversationController::title() const
{
    return title_;
}

QString
CodexConversationController::model() const
{
    return model_;
}

QString
CodexConversationController::reasoningEffort() const
{
    return reasoningEffort_;
}

QVariantList
CodexConversationController::reasoningEfforts() const
{
    return modelCatalogModel_.reasoningEffortsForModel(model_);
}

bool
CodexConversationController::loadingModelCatalog() const
{
    return loadingModelCatalog_;
}

QString
CodexConversationController::modelCatalogErrorMessage() const
{
    return modelCatalogErrorMessage_;
}

bool
CodexConversationController::loading() const
{
    return loading_;
}

bool
CodexConversationController::activityHistoryPartial() const
{
    return activityHistoryPartial_;
}

CodexConversationController::TurnState
CodexConversationController::turnState() const
{
    return turnRuntimeState_.status;
}

bool
CodexConversationController::turnInFlight() const
{
    return turnRuntimeState_.status == TurnState::Starting || turnRuntimeState_.status == TurnState::Running;
}

bool
CodexConversationController::mutationInFlight() const
{
    return history_->startingThread() || history_->forkingThread() || turnInFlight() ||
           history_->changingThreadLifecycle();
}

bool
CodexConversationController::turnRunning() const
{
    return turnRuntimeState_.status == TurnState::Running;
}

QString
CodexConversationController::activeTurnId() const
{
    return turnRuntimeState_.activeTurnId;
}

bool
CodexConversationController::waitingOnApproval() const
{
    return turnRuntimeState_.waitingOnApproval;
}

bool
CodexConversationController::waitingOnUserInput() const
{
    return turnRuntimeState_.waitingOnUserInput;
}

bool
CodexConversationController::steeringTurn() const
{
    return steeringTurn_;
}

bool
CodexConversationController::interruptRequested() const
{
    return interruptRequested_;
}

CodexConversationController::TurnMode
CodexConversationController::turnMode() const
{
    return turnMode_;
}

void
CodexConversationController::setTurnMode(TurnMode mode)
{
    if (turnMode_ == mode)
        return;
    turnMode_ = mode;
    emit turnOptionsChanged();
}

CodexConversationController::PermissionPreset
CodexConversationController::permissionPreset() const
{
    return permissionPreset_;
}

void
CodexConversationController::setPermissionPreset(PermissionPreset preset)
{
    if (permissionPreset_ == preset)
        return;
    permissionPreset_ = preset;
    emit turnOptionsChanged();
}

CodexConversationController::WriteAvailability
CodexConversationController::writeAvailability() const
{
    return writeAvailability_;
}

QString
CodexConversationController::writeAvailabilityMessage() const
{
    return writeAvailabilityMessage_;
}

void
CodexConversationController::acquireWriteAccess()
{
    if (history_->showingArchived() || threadId_.isEmpty() || mutationInFlight() ||
        writeAvailability_ == WriteAvailability::Checking || writeAvailability_ == WriteAvailability::Writable)
        return;
    if (observer_ == nullptr) {
        setWriteAvailability(WriteAvailability::Unavailable,
                             /*% "The Codex history observer is unavailable." */ qtTrId(
                               "craftward.codex.error.history_observer_unavailable"));
        return;
    }

    const QByteArray threadId = threadId_.toUtf8();
    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_acquire_write_async(observer_, threadId.constData(), &rawError)) {
        QString message = ward::coreffi::takeErrorMessage(rawError);
        if (message.isEmpty())
            message = /*% "Writing access could not be checked for this conversation." */ qtTrId(
              "craftward.codex.error.write_access_check");
        setWriteAvailability(WriteAvailability::Unavailable, message);
        return;
    }

    setWriteAvailability(WriteAvailability::Checking);
}

void
CodexConversationController::releaseWriteAccess()
{
    if (threadId_.isEmpty() || history_->startingThread() || history_->forkingThread() || turnInFlight() ||
        writeAvailability_ == WriteAvailability::NotRequested)
        return;
    if (observer_ == nullptr) {
        setWriteAvailability(WriteAvailability::NotRequested);
        return;
    }

    const QByteArray threadId = threadId_.toUtf8();
    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_release_write_async(observer_, threadId.constData(), &rawError)) {
        QString message = ward::coreffi::takeErrorMessage(rawError);
        if (message.isEmpty())
            message = /*% "Writing access could not be released for this conversation." */ qtTrId(
              "craftward.codex.error.write_access_release");
        setWriteAvailability(WriteAvailability::Unavailable, message);
        return;
    }

    setWriteAvailability(WriteAvailability::NotRequested);
}

bool
CodexConversationController::startTurn(const QString& prompt, const QList<QUrl>& attachments)
{
    if (history_->showingArchived() || (prompt.trimmed().isEmpty() && attachments.isEmpty()) || threadId_.isEmpty() ||
        mutationInFlight() || writeAvailability_ != WriteAvailability::Writable)
        return false;
    if (observer_ == nullptr) {
        setErrorMessage(/*% "The Codex history observer is unavailable." */ qtTrId(
          "craftward.codex.error.history_observer_unavailable"));
        return false;
    }

    QString attachmentError;
    const std::optional<QList<CodexTurnAttachment>> preparedAttachments =
      CodexAttachmentInput::prepare(attachments, &attachmentError);
    if (!preparedAttachments.has_value()) {
        setErrorMessage(attachmentError);
        return false;
    }
    QList<WardCodexTurnAttachment> encodedAttachments;
    encodedAttachments.reserve(preparedAttachments->size());
    for (const CodexTurnAttachment& attachment : *preparedAttachments) {
        encodedAttachments.append({ static_cast<WardCodexTurnAttachmentKind>(static_cast<int>(attachment.kind)),
                                    attachment.name.constData(),
                                    attachment.path.constData() });
    }

    const QByteArray threadId = threadId_.toUtf8();
    const QByteArray encodedPrompt = prompt.toUtf8();
    const QByteArray encodedModel = modelOverride().toUtf8();
    const QByteArray encodedReasoningEffort = reasoningEffortOverride().toUtf8();
    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_start_turn_async(
          observer_,
          threadId.constData(),
          encodedPrompt.constData(),
          encodedAttachments.isEmpty() ? nullptr : encodedAttachments.constData(),
          static_cast<size_t>(encodedAttachments.size()),
          encodedModel.isEmpty() ? nullptr : encodedModel.constData(),
          encodedReasoningEffort.isEmpty() ? nullptr : encodedReasoningEffort.constData(),
          static_cast<WardCodexTurnMode>(static_cast<int>(turnMode_)),
          static_cast<WardCodexPermissionPreset>(static_cast<int>(permissionPreset_)),
          &rawError)) {
        QString message = ward::coreffi::takeErrorMessage(rawError);
        if (message.isEmpty())
            message = /*% "The Codex turn could not be started." */ qtTrId("craftward.codex.error.turn_start");
        setErrorMessage(message);
        return false;
    }

    setErrorMessage({});
    setInterruptRequested(false);
    setTurnState(TurnState::Starting);
    return true;
}

QVariantList
CodexConversationController::describeAttachments(const QList<QUrl>& attachments)
{
    QString attachmentError;
    const std::optional<QList<CodexAttachmentDescriptor>> described =
      CodexAttachmentInput::describe(attachments, false, &attachmentError);
    if (!described.has_value()) {
        setErrorMessage(attachmentError);
        return {};
    }
    if (!described->isEmpty())
        setErrorMessage({});
    return attachmentDescriptorList(*described);
}

QVariantList
CodexConversationController::attachmentsFromClipboard()
{
    QString attachmentError;
    const QList<CodexAttachmentDescriptor> attachments = CodexAttachmentInput::fromClipboard(&attachmentError);
    if (!attachmentError.isEmpty())
        setErrorMessage(attachmentError);
    else if (!attachments.isEmpty())
        setErrorMessage({});
    return attachmentDescriptorList(attachments);
}

bool
CodexConversationController::selectModel(const QString& model)
{
    if (threadId_.isEmpty() || history_->showingArchived() || turnInFlight() ||
        !modelCatalogModel_.containsModel(model))
        return false;

    ThreadInferenceSelection& selection = inferenceSelections_[threadId_];
    selection.selected.model = model;
    selection.selected.reasoningEffort =
      modelCatalogModel_.resolveReasoningEffort(model, selection.selected.reasoningEffort);
    setDisplayedModel(model);
    setDisplayedReasoningEffort(selection.selected.reasoningEffort);
    return true;
}

bool
CodexConversationController::selectReasoningEffort(const QString& effort)
{
    if (threadId_.isEmpty() || history_->showingArchived() || turnInFlight() ||
        !modelCatalogModel_.supportsReasoningEffort(model_, effort))
        return false;

    ThreadInferenceSelection& selection = inferenceSelections_[threadId_];
    selection.selected.reasoningEffort = effort;
    setDisplayedReasoningEffort(effort);
    return true;
}

void
CodexConversationController::applyThreadInferenceOptions(const QString& threadId,
                                                         const QString& model,
                                                         const QString& reasoningEffort)
{
    ThreadInferenceSelection& selection = inferenceSelections_[threadId];
    const bool hasPendingModel =
      !selection.selected.model.isEmpty() && selection.selected.model != selection.active.model;
    const bool hasPendingReasoningEffort = !selection.selected.reasoningEffort.isEmpty() &&
                                           selection.selected.reasoningEffort != selection.active.reasoningEffort;
    const bool hasPendingOverride = hasPendingModel || hasPendingReasoningEffort;
    const QString normalizedReasoningEffort =
      reasoningEffort.isEmpty() ? modelCatalogModel_.resolveReasoningEffort(model, {}) : reasoningEffort;
    selection.active = { model, normalizedReasoningEffort };
    const bool pendingOverrideWasAccepted =
      selection.selected.model == model && selection.selected.reasoningEffort == normalizedReasoningEffort;
    if (!hasPendingOverride || pendingOverrideWasAccepted) {
        selection.selected = selection.active;
    }
    if (threadId == threadId_) {
        setDisplayedModel(selection.selected.model);
        setDisplayedReasoningEffort(selection.selected.reasoningEffort);
    }
}

void
CodexConversationController::restoreInferenceSelection()
{
    const auto selection = inferenceSelections_.constFind(threadId_);
    const bool missing = selection == inferenceSelections_.cend();
    setDisplayedModel(missing ? QString() : selection->selected.model);
    setDisplayedReasoningEffort(missing ? QString() : selection->selected.reasoningEffort);
}

void
CodexConversationController::reconcileInferenceSelections()
{
    for (ThreadInferenceSelection& selection : inferenceSelections_) {
        if (selection.active.reasoningEffort.isEmpty())
            selection.active.reasoningEffort = modelCatalogModel_.resolveReasoningEffort(selection.active.model, {});
        if (selection.selected.model.isEmpty())
            selection.selected.model = selection.active.model;
        if (selection.selected.reasoningEffort.isEmpty()) {
            selection.selected.reasoningEffort =
              selection.selected.model == selection.active.model
                ? selection.active.reasoningEffort
                : modelCatalogModel_.resolveReasoningEffort(selection.selected.model, {});
        }
    }
    restoreInferenceSelection();
}

void
CodexConversationController::setDisplayedModel(const QString& model)
{
    if (model_ == model)
        return;
    model_ = model;
    emit modelChanged();
    emit reasoningEffortsChanged();
}

void
CodexConversationController::setDisplayedReasoningEffort(const QString& effort)
{
    if (reasoningEffort_ == effort)
        return;
    reasoningEffort_ = effort;
    emit reasoningEffortChanged();
}

QString
CodexConversationController::modelOverride() const
{
    const auto selection = inferenceSelections_.constFind(threadId_);
    if (selection == inferenceSelections_.cend() || selection->selected.model.isEmpty() ||
        selection->selected.model == selection->active.model)
        return {};
    return selection->selected.model;
}

QString
CodexConversationController::reasoningEffortOverride() const
{
    const auto selection = inferenceSelections_.constFind(threadId_);
    if (selection == inferenceSelections_.cend() || selection->selected.reasoningEffort.isEmpty())
        return {};
    const bool modelChanges =
      !selection->selected.model.isEmpty() && selection->selected.model != selection->active.model;
    if (!modelChanges && selection->selected.reasoningEffort == selection->active.reasoningEffort)
        return {};
    return selection->selected.reasoningEffort;
}

bool
CodexConversationController::steerTurn(const QString& prompt)
{
    if (history_->showingArchived() || history_->changingThreadLifecycle() || prompt.trimmed().isEmpty() ||
        threadId_.isEmpty() || !turnRunning() || activeTurnId().isEmpty() || steeringTurn_ || interruptRequested_ ||
        writeAvailability_ != WriteAvailability::Writable)
        return false;
    if (observer_ == nullptr) {
        setErrorMessage(/*% "The Codex history observer is unavailable." */ qtTrId(
          "craftward.codex.error.history_observer_unavailable"));
        return false;
    }

    const QByteArray threadId = threadId_.toUtf8();
    const QByteArray turnId = activeTurnId().toUtf8();
    const QByteArray encodedPrompt = prompt.toUtf8();
    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_steer_turn_async(
          observer_, threadId.constData(), turnId.constData(), encodedPrompt.constData(), &rawError)) {
        QString message = ward::coreffi::takeErrorMessage(rawError);
        if (message.isEmpty())
            message = /*% "The Codex turn could not be guided." */ qtTrId("craftward.codex.error.turn_steer");
        setErrorMessage(message);
        return false;
    }

    setErrorMessage({});
    setSteeringTurn(true);
    return true;
}

bool
CodexConversationController::interruptTurn()
{
    if (history_->showingArchived() || history_->changingThreadLifecycle() || history_->startingThread() ||
        !turnInFlight() || threadId_.isEmpty() || interruptRequested_ || observer_ == nullptr)
        return false;

    const QByteArray threadId = threadId_.toUtf8();
    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_interrupt_turn_async(observer_, threadId.constData(), &rawError)) {
        QString message = ward::coreffi::takeErrorMessage(rawError);
        if (message.isEmpty())
            message = /*% "The Codex turn could not be stopped." */ qtTrId("craftward.codex.error.turn_stop");
        setErrorMessage(message);
        return false;
    }

    setErrorMessage({});
    setInterruptRequested(true);
    return true;
}

bool
CodexConversationController::respondToApproval(const QString& interactionId, InteractionDecision decision)
{
    if (history_->showingArchived() || history_->changingThreadLifecycle() || history_->startingThread())
        return false;
    bool validId = false;
    const qulonglong id = interactionId.toULongLong(&validId);
    if (!validId || id == 0 || observer_ == nullptr)
        return false;

    using WireDecision = ward::codex::v1::PendingInteractionDecisionGadget::PendingInteractionDecision;
    WireDecision wireDecision;
    switch (decision) {
        case InteractionDecision::Accept:
            wireDecision = WireDecision::PENDING_INTERACTION_DECISION_ACCEPT;
            break;
        case InteractionDecision::AcceptForSession:
            wireDecision = WireDecision::PENDING_INTERACTION_DECISION_ACCEPT_FOR_SESSION;
            break;
        case InteractionDecision::Decline:
            wireDecision = WireDecision::PENDING_INTERACTION_DECISION_DECLINE;
            break;
        case InteractionDecision::Cancel:
            wireDecision = WireDecision::PENDING_INTERACTION_DECISION_CANCEL;
            break;
        default:
            return false;
    }

    ward::codex::v1::PendingInteractionResponse response;
    response.setInteractionId(id);
    response.setDecision(wireDecision);
    return sendInteractionResponse(interactionId, response);
}

bool
CodexConversationController::respondToUserInput(const QString& interactionId, const QVariantMap& answers)
{
    if (history_->showingArchived() || history_->changingThreadLifecycle() || history_->startingThread())
        return false;
    bool validId = false;
    const qulonglong id = interactionId.toULongLong(&validId);
    if (!validId || id == 0 || observer_ == nullptr)
        return false;

    QList<ward::codex::v1::PendingInteractionAnswer> encodedAnswers;
    encodedAnswers.reserve(answers.size());
    for (auto answer = answers.cbegin(); answer != answers.cend(); ++answer) {
        QStringList values;
        if (answer.value().metaType().id() == QMetaType::QStringList) {
            values = answer.value().toStringList();
        } else if (answer.value().metaType().id() == QMetaType::QVariantList) {
            for (const QVariant& value : answer.value().toList())
                values.append(value.toString());
        } else {
            values.append(answer.value().toString());
        }
        ward::codex::v1::PendingInteractionAnswer encodedAnswer;
        encodedAnswer.setQuestionId(answer.key());
        encodedAnswer.setAnswers(std::move(values));
        encodedAnswers.append(std::move(encodedAnswer));
    }

    ward::codex::v1::PendingUserInputResponse userInput;
    userInput.setAnswers(std::move(encodedAnswers));
    ward::codex::v1::PendingInteractionResponse response;
    response.setInteractionId(id);
    response.setUserInput(std::move(userInput));
    return sendInteractionResponse(interactionId, response);
}

void
CodexConversationController::setErrorMessage(const QString& message)
{
    if (errorMessage_ == message)
        return;
    errorMessage_ = message;
    emit errorMessageChanged();
}

void
CodexConversationController::clearSelection()
{
    const bool hadSelection = !threadId_.isEmpty() || !title_.isEmpty();
    const bool wasLoading = std::exchange(loading_, false);
    threadId_.clear();
    title_.clear();
    setDisplayedModel({});
    setDisplayedReasoningEffort({});
    timelineModel_.clear();
    interactionModel_.clear();
    setActivityHistoryPartial(false);
    setTurnState(TurnState::Detached);
    setWriteAvailability(WriteAvailability::NotRequested);
    setErrorMessage({});
    setInterruptRequested(false);
    if (hadSelection)
        emit selectionChanged();
    if (wasLoading)
        emit loadingChanged();
}

void
CodexConversationController::setActivityHistoryPartial(bool partial)
{
    if (activityHistoryPartial_ == partial)
        return;
    activityHistoryPartial_ = partial;
    emit activityHistoryPartialChanged();
}

void
CodexConversationController::setTurnState(TurnState state,
                                          const QString& activeTurnId,
                                          bool waitingOnApproval,
                                          bool waitingOnUserInput)
{
    if (state != TurnState::Running)
        setSteeringTurn(false);
    if (state != TurnState::Starting && state != TurnState::Running)
        setInterruptRequested(false);
    if (turnRuntimeState_.status == state && turnRuntimeState_.activeTurnId == activeTurnId &&
        turnRuntimeState_.waitingOnApproval == waitingOnApproval &&
        turnRuntimeState_.waitingOnUserInput == waitingOnUserInput)
        return;
    turnRuntimeState_ = {
        .status = state,
        .activeTurnId = activeTurnId,
        .waitingOnApproval = waitingOnApproval,
        .waitingOnUserInput = waitingOnUserInput,
    };
    emit turnStateChanged();
}

void
CodexConversationController::setSteeringTurn(bool steering)
{
    if (steeringTurn_ == steering)
        return;
    steeringTurn_ = steering;
    emit steeringTurnChanged();
}

void
CodexConversationController::setWriteAvailability(WriteAvailability availability, const QString& message)
{
    if (writeAvailability_ == availability && writeAvailabilityMessage_ == message)
        return;
    writeAvailability_ = availability;
    writeAvailabilityMessage_ = message;
    emit writeAvailabilityChanged();
}

void
CodexConversationController::setInterruptRequested(bool requested)
{
    if (interruptRequested_ == requested)
        return;
    interruptRequested_ = requested;
    emit interruptRequestedChanged();
}

bool
CodexConversationController::sendInteractionResponse(const QString& interactionId,
                                                     const ward::codex::v1::PendingInteractionResponse& response)
{
    QProtobufSerializer serializer;
    const QByteArray encoded = response.serialize(&serializer);
    if (serializer.lastError() != QAbstractProtobufSerializer::Error::None) {
        setErrorMessage(
          /*% "The Codex response could not be encoded: %1" */ qtTrId("craftward.codex.error.response_encode")
            .arg(serializer.lastErrorString()));
        return false;
    }

    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_resolve_interaction_async(
          observer_,
          reinterpret_cast<const std::uint8_t*>(encoded.constData()),
          static_cast<std::size_t>(encoded.size()),
          &rawError)) {
        QString message = ward::coreffi::takeErrorMessage(rawError);
        if (message.isEmpty())
            message = /*% "The Codex response could not be sent." */ qtTrId("craftward.codex.error.response_send");
        setErrorMessage(message);
        return false;
    }

    interactionModel_.setResolving(interactionId, true);
    setErrorMessage({});
    return true;
}

void
CodexConversationController::finishModelCatalogLoading(const QString& errorMessage)
{
    const bool changed = std::exchange(loadingModelCatalog_, false) || modelCatalogErrorMessage_ != errorMessage;
    modelCatalogErrorMessage_ = errorMessage;
    if (changed)
        emit modelCatalogStateChanged();
}

void
CodexConversationController::finishLoading(const QString& errorMessage)
{
    const bool wasLoading = std::exchange(loading_, false);
    setErrorMessage(errorMessage);
    if (wasLoading)
        emit loadingChanged();
}

void
CodexConversationController::adoptConversation(const QString& threadId,
                                               const ward::codex::v1::Conversation& conversation)
{
    const bool wasLoading = std::exchange(loading_, false);
    threadId_ = threadId;
    title_ = conversation.title();
    restoreInferenceSelection();
    auto timeline = conversation.timeline();
    timelineModel_.reconcileTimeline(std::move(timeline), conversation.forkableTurnIds());
    interactionModel_.clear();
    setActivityHistoryPartial(conversation.activityHistoryIsPartial());
    setTurnState(TurnState::Detached);
    setWriteAvailability(WriteAvailability::NotRequested);
    setErrorMessage({});
    setInterruptRequested(false);
    emit selectionChanged();
    if (wasLoading)
        emit loadingChanged();
}
