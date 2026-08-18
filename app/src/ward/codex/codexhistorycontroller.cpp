// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexhistorycontroller.h"

#include "ward/coreffi.h"

#include <QByteArray>
#include <QByteArrayView>
#include <QDir>
#include <QFileInfo>
#include <QMetaObject>
#include <QtProtobuf/QProtobufSerializer>

#include <memory>
#include <utility>

struct CodexHistoryCallbackContext
{
    CodexHistoryController* controller;
    std::uint64_t generation;
};

namespace {
struct ErrorDeleter
{
    void operator()(WardError* error) const { ward_core_error_destroy(error); }
};

QString
copyString(const char* value)
{
    return value != nullptr ? QString::fromUtf8(value) : QString();
}

QString
copyError(WardError* error)
{
    std::unique_ptr<WardError, ErrorDeleter> ownedError(error);
    return error != nullptr ? copyString(ward_core_error_message(error)) : QString();
}

template<typename Message>
QString
deserializeBytes(QByteArrayView bytes, Message& message)
{
    QProtobufSerializer serializer;
    if (message.deserialize(&serializer, bytes))
        return {};
    return QStringLiteral("Failed to decode Ward Core response: %1").arg(serializer.lastErrorString());
}

QByteArray
codexExecutable()
{
    const QByteArray configured = qgetenv("CRAFTWARD_CODEX_PATH");
    return configured.isEmpty() ? QByteArrayLiteral("codex") : configured;
}

static_assert(static_cast<int>(CodexHistoryController::TurnMode::DefaultMode) == WardCodexTurnModeDefault);
static_assert(static_cast<int>(CodexHistoryController::TurnMode::PlanMode) == WardCodexTurnModePlan);
static_assert(static_cast<int>(CodexHistoryController::PermissionPreset::InheritPermissions) ==
              WardCodexPermissionPresetInherit);
static_assert(static_cast<int>(CodexHistoryController::PermissionPreset::RequestApproval) ==
              WardCodexPermissionPresetRequestApproval);
static_assert(static_cast<int>(CodexHistoryController::PermissionPreset::ReadOnlyPermissions) ==
              WardCodexPermissionPresetReadOnly);
}

CodexHistoryController::CodexHistoryController(const WardRuntime* runtime, QObject* parent)
  : QObject(parent)
  , threadModel_(this)
  , timelineModel_(this)
  , interactionModel_(this)
{
    ++observerGeneration_;
    callbackContext_ = std::make_unique<CodexHistoryCallbackContext>(CodexHistoryCallbackContext{
      .controller = this,
      .generation = observerGeneration_,
    });

    WardError* rawError = nullptr;
    const QByteArray executable = codexExecutable();
    historyObserver_ = ward_core_codex_history_observer_open(
      runtime, executable.constData(), handleHistoryEvent, callbackContext_.get(), &rawError);
    if (historyObserver_ == nullptr) {
        callbackContext_.reset();
        loadingThreads_ = false;
        QString message = copyError(rawError);
        if (message.isEmpty())
            message = tr("The Codex history observer could not be started.");
        setThreadErrorMessage(message);
    }
}

CodexHistoryController::~CodexHistoryController()
{
    ++observerGeneration_;
    if (historyObserver_ != nullptr)
        ward_core_codex_history_observer_destroy(std::exchange(historyObserver_, nullptr));
    callbackContext_.reset();
}

CodexThreadModel*
CodexHistoryController::threads()
{
    return &threadModel_;
}

CodexTimelineModel*
CodexHistoryController::timeline()
{
    return &timelineModel_;
}

CodexInteractionModel*
CodexHistoryController::interactions()
{
    return &interactionModel_;
}

QString
CodexHistoryController::selectedThreadId() const
{
    return selectedThreadId_;
}

QString
CodexHistoryController::selectedThreadTitle() const
{
    return selectedThreadTitle_;
}

QString
CodexHistoryController::errorMessage() const
{
    return errorMessage_;
}

bool
CodexHistoryController::loadingThreads() const
{
    return loadingThreads_;
}

bool
CodexHistoryController::loadingConversation() const
{
    return loadingConversation_;
}

bool
CodexHistoryController::startingThread() const
{
    return startingThread_;
}

bool
CodexHistoryController::activityHistoryPartial() const
{
    return activityHistoryPartial_;
}

CodexHistoryController::TurnState
CodexHistoryController::turnState() const
{
    return turnRuntimeState_.status;
}

bool
CodexHistoryController::turnInFlight() const
{
    return turnRuntimeState_.status == TurnState::Starting || turnRuntimeState_.status == TurnState::Running;
}

bool
CodexHistoryController::turnRunning() const
{
    return turnRuntimeState_.status == TurnState::Running;
}

QString
CodexHistoryController::activeTurnId() const
{
    return turnRuntimeState_.activeTurnId;
}

bool
CodexHistoryController::waitingOnApproval() const
{
    return turnRuntimeState_.waitingOnApproval;
}

bool
CodexHistoryController::waitingOnUserInput() const
{
    return turnRuntimeState_.waitingOnUserInput;
}

bool
CodexHistoryController::steeringTurn() const
{
    return steeringTurn_;
}

bool
CodexHistoryController::interruptRequested() const
{
    return interruptRequested_;
}

CodexHistoryController::TurnMode
CodexHistoryController::turnMode() const
{
    return turnMode_;
}

void
CodexHistoryController::setTurnMode(TurnMode mode)
{
    if (turnMode_ == mode)
        return;
    turnMode_ = mode;
    emit turnOptionsChanged();
}

CodexHistoryController::PermissionPreset
CodexHistoryController::permissionPreset() const
{
    return permissionPreset_;
}

void
CodexHistoryController::setPermissionPreset(PermissionPreset preset)
{
    if (permissionPreset_ == preset)
        return;
    permissionPreset_ = preset;
    emit turnOptionsChanged();
}

CodexHistoryController::WriteAvailability
CodexHistoryController::writeAvailability() const
{
    return writeAvailability_;
}

QString
CodexHistoryController::writeAvailabilityMessage() const
{
    return writeAvailabilityMessage_;
}

void
CodexHistoryController::refresh()
{
    if (startingThread_ || turnInFlight())
        return;
    if (historyObserver_ == nullptr) {
        setThreadErrorMessage(tr("The Codex history observer is unavailable."));
        if (!selectedThreadId_.isEmpty())
            setConversationErrorMessage(tr("The Codex history observer is unavailable."));
        return;
    }

    bool loadingChanged = false;
    if (!std::exchange(loadingThreads_, true))
        loadingChanged = true;
    if (!selectedThreadId_.isEmpty() && !std::exchange(loadingConversation_, true))
        loadingChanged = true;
    if (loadingChanged)
        emit this->loadingChanged();

    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_refresh(historyObserver_, &rawError)) {
        QString message = copyError(rawError);
        if (message.isEmpty())
            message = tr("The Codex history could not be refreshed.");
        finishThreadLoading(message);
        if (!selectedThreadId_.isEmpty())
            finishConversationLoading(message);
    }
}

void
CodexHistoryController::selectThread(const QString& threadId, const QString& title)
{
    if (threadId.isEmpty())
        return;
    if (startingThread_)
        return;
    if (turnInFlight() && threadId != selectedThreadId_)
        return;
    if (threadId == selectedThreadId_) {
        if (title != selectedThreadTitle_) {
            selectedThreadTitle_ = title;
            emit selectionChanged();
        }
        return;
    }

    const QByteArray encodedThreadId = threadId.toUtf8();
    selectedThreadId_ = threadId;
    selectedThreadTitle_ = title;
    timelineModel_.clear();
    interactionModel_.clear();
    setActivityHistoryPartial(false);
    setTurnState(TurnState::Detached);
    setWriteAvailability(WriteAvailability::NotRequested);
    setConversationErrorMessage({});
    setInterruptRequested(false);
    loadingConversation_ = true;
    emit selectionChanged();
    emit loadingChanged();

    if (historyObserver_ == nullptr) {
        finishConversationLoading(tr("The Codex history observer is unavailable."));
        return;
    }

    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_watch(historyObserver_, encodedThreadId.constData(), &rawError)) {
        QString message = copyError(rawError);
        if (message.isEmpty())
            message = tr("The Codex conversation could not be observed.");
        finishConversationLoading(message);
    }
}

bool
CodexHistoryController::renameSelectedThread(const QString& name)
{
    const QString normalizedName = name.trimmed();
    if (selectedThreadId_.isEmpty() || normalizedName.isEmpty() || normalizedName == selectedThreadTitle_.trimmed() ||
        startingThread_ || turnInFlight() || loadingConversation_)
        return false;
    if (historyObserver_ == nullptr) {
        setConversationErrorMessage(tr("The Codex history observer is unavailable."));
        return false;
    }

    const QByteArray threadId = selectedThreadId_.toUtf8();
    const QByteArray encodedName = normalizedName.toUtf8();
    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_rename_thread(
          historyObserver_, threadId.constData(), encodedName.constData(), &rawError)) {
        QString message = copyError(rawError);
        if (message.isEmpty())
            message = tr("The Codex conversation could not be renamed.");
        setConversationErrorMessage(message);
        return false;
    }

    setConversationErrorMessage({});
    return true;
}

bool
CodexHistoryController::startThread(const QUrl& workingDirectory)
{
    if (startingThread_ || turnInFlight() || loadingConversation_)
        return false;
    if (!workingDirectory.isLocalFile()) {
        setThreadStartErrorMessage(tr("Choose a local working directory for the new Codex conversation."));
        return false;
    }

    const QString path = QDir::cleanPath(workingDirectory.toLocalFile());
    const QFileInfo directory(path);
    if (!directory.isAbsolute() || !directory.isDir()) {
        setThreadStartErrorMessage(tr("The selected Codex working directory is unavailable."));
        return false;
    }
    if (historyObserver_ == nullptr) {
        setThreadStartErrorMessage(tr("The Codex history observer is unavailable."));
        return false;
    }

    const QByteArray encodedPath = path.toUtf8();
    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_start_thread(historyObserver_, encodedPath.constData(), &rawError)) {
        QString message = copyError(rawError);
        if (message.isEmpty())
            message = tr("The Codex conversation could not be started.");
        setThreadStartErrorMessage(message);
        return false;
    }

    setThreadStartErrorMessage({});
    setStartingThread(true);
    return true;
}

void
CodexHistoryController::acquireWriteAccess()
{
    if (selectedThreadId_.isEmpty() || startingThread_ || turnInFlight() ||
        writeAvailability_ == WriteAvailability::Checking || writeAvailability_ == WriteAvailability::Writable)
        return;
    if (historyObserver_ == nullptr) {
        setWriteAvailability(WriteAvailability::Unavailable, tr("The Codex history observer is unavailable."));
        return;
    }

    const QByteArray threadId = selectedThreadId_.toUtf8();
    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_acquire_write(historyObserver_, threadId.constData(), &rawError)) {
        QString message = copyError(rawError);
        if (message.isEmpty())
            message = tr("Writing access could not be checked for this conversation.");
        setWriteAvailability(WriteAvailability::Unavailable, message);
        return;
    }

    setWriteAvailability(WriteAvailability::Checking);
}

void
CodexHistoryController::releaseWriteAccess()
{
    if (selectedThreadId_.isEmpty() || startingThread_ || turnInFlight() ||
        writeAvailability_ == WriteAvailability::NotRequested)
        return;
    if (historyObserver_ == nullptr) {
        setWriteAvailability(WriteAvailability::NotRequested);
        return;
    }

    const QByteArray threadId = selectedThreadId_.toUtf8();
    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_release_write(historyObserver_, threadId.constData(), &rawError)) {
        QString message = copyError(rawError);
        if (message.isEmpty())
            message = tr("Writing access could not be released for this conversation.");
        setWriteAvailability(WriteAvailability::Unavailable, message);
        return;
    }

    setWriteAvailability(WriteAvailability::NotRequested);
}

bool
CodexHistoryController::startTurn(const QString& prompt)
{
    if (prompt.trimmed().isEmpty() || selectedThreadId_.isEmpty() || startingThread_ || turnInFlight() ||
        writeAvailability_ != WriteAvailability::Writable)
        return false;
    if (historyObserver_ == nullptr) {
        setConversationErrorMessage(tr("The Codex history observer is unavailable."));
        return false;
    }

    const QByteArray threadId = selectedThreadId_.toUtf8();
    const QByteArray encodedPrompt = prompt.toUtf8();
    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_start_turn(
          historyObserver_,
          threadId.constData(),
          encodedPrompt.constData(),
          static_cast<WardCodexTurnMode>(static_cast<int>(turnMode_)),
          static_cast<WardCodexPermissionPreset>(static_cast<int>(permissionPreset_)),
          &rawError)) {
        QString message = copyError(rawError);
        if (message.isEmpty())
            message = tr("The Codex turn could not be started.");
        setConversationErrorMessage(message);
        return false;
    }

    setConversationErrorMessage({});
    setInterruptRequested(false);
    setTurnState(TurnState::Starting);
    return true;
}

bool
CodexHistoryController::steerTurn(const QString& prompt)
{
    if (prompt.trimmed().isEmpty() || selectedThreadId_.isEmpty() || !turnRunning() || activeTurnId().isEmpty() ||
        steeringTurn_ || interruptRequested_ || writeAvailability_ != WriteAvailability::Writable)
        return false;
    if (historyObserver_ == nullptr) {
        setConversationErrorMessage(tr("The Codex history observer is unavailable."));
        return false;
    }

    const QByteArray threadId = selectedThreadId_.toUtf8();
    const QByteArray turnId = activeTurnId().toUtf8();
    const QByteArray encodedPrompt = prompt.toUtf8();
    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_steer_turn(
          historyObserver_, threadId.constData(), turnId.constData(), encodedPrompt.constData(), &rawError)) {
        QString message = copyError(rawError);
        if (message.isEmpty())
            message = tr("The Codex turn could not be guided.");
        setConversationErrorMessage(message);
        return false;
    }

    setConversationErrorMessage({});
    setSteeringTurn(true);
    return true;
}

bool
CodexHistoryController::interruptTurn()
{
    if (startingThread_ || !turnInFlight() || selectedThreadId_.isEmpty() || interruptRequested_ ||
        historyObserver_ == nullptr)
        return false;

    const QByteArray threadId = selectedThreadId_.toUtf8();
    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_interrupt_turn(historyObserver_, threadId.constData(), &rawError)) {
        QString message = copyError(rawError);
        if (message.isEmpty())
            message = tr("The Codex turn could not be stopped.");
        setConversationErrorMessage(message);
        return false;
    }

    setConversationErrorMessage({});
    setInterruptRequested(true);
    return true;
}

bool
CodexHistoryController::respondToApproval(const QString& interactionId, InteractionDecision decision)
{
    if (startingThread_)
        return false;
    bool validId = false;
    const qulonglong id = interactionId.toULongLong(&validId);
    if (!validId || id == 0 || historyObserver_ == nullptr)
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
CodexHistoryController::respondToUserInput(const QString& interactionId, const QVariantMap& answers)
{
    if (startingThread_)
        return false;
    bool validId = false;
    const qulonglong id = interactionId.toULongLong(&validId);
    if (!validId || id == 0 || historyObserver_ == nullptr)
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
CodexHistoryController::clearError()
{
    threadErrorMessage_.clear();
    threadStartErrorMessage_.clear();
    conversationErrorMessage_.clear();
    updateErrorMessage();
}

void
CodexHistoryController::setErrorMessage(const QString& message)
{
    if (errorMessage_ == message)
        return;
    errorMessage_ = message;
    emit errorMessageChanged();
}

void
CodexHistoryController::setThreadErrorMessage(const QString& message)
{
    const bool clearedOlderStartError = !message.isEmpty() && !threadStartErrorMessage_.isEmpty();
    if (clearedOlderStartError)
        threadStartErrorMessage_.clear();
    if (threadErrorMessage_ == message) {
        if (clearedOlderStartError)
            updateErrorMessage();
        return;
    }
    threadErrorMessage_ = message;
    updateErrorMessage();
}

void
CodexHistoryController::setThreadStartErrorMessage(const QString& message)
{
    if (threadStartErrorMessage_ == message)
        return;
    threadStartErrorMessage_ = message;
    updateErrorMessage();
}

void
CodexHistoryController::setConversationErrorMessage(const QString& message)
{
    const bool clearedOlderStartError = !message.isEmpty() && !threadStartErrorMessage_.isEmpty();
    if (clearedOlderStartError)
        threadStartErrorMessage_.clear();
    if (conversationErrorMessage_ == message) {
        if (clearedOlderStartError)
            updateErrorMessage();
        return;
    }
    conversationErrorMessage_ = message;
    updateErrorMessage();
}

void
CodexHistoryController::setStartingThread(bool starting)
{
    if (startingThread_ == starting)
        return;
    startingThread_ = starting;
    emit startingThreadChanged();
}

void
CodexHistoryController::setActivityHistoryPartial(bool partial)
{
    if (activityHistoryPartial_ == partial)
        return;
    activityHistoryPartial_ = partial;
    emit activityHistoryPartialChanged();
}

void
CodexHistoryController::setTurnState(TurnState state,
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
CodexHistoryController::setSteeringTurn(bool steering)
{
    if (steeringTurn_ == steering)
        return;
    steeringTurn_ = steering;
    emit steeringTurnChanged();
}

void
CodexHistoryController::setWriteAvailability(WriteAvailability availability, const QString& message)
{
    if (writeAvailability_ == availability && writeAvailabilityMessage_ == message)
        return;
    writeAvailability_ = availability;
    writeAvailabilityMessage_ = message;
    emit writeAvailabilityChanged();
}

void
CodexHistoryController::setInterruptRequested(bool requested)
{
    if (interruptRequested_ == requested)
        return;
    interruptRequested_ = requested;
    emit interruptRequestedChanged();
}

bool
CodexHistoryController::sendInteractionResponse(const QString& interactionId,
                                                const ward::codex::v1::PendingInteractionResponse& response)
{
    QProtobufSerializer serializer;
    const QByteArray encoded = response.serialize(&serializer);
    if (serializer.lastError() != QAbstractProtobufSerializer::Error::None) {
        setConversationErrorMessage(
          tr("The Codex response could not be encoded: %1").arg(serializer.lastErrorString()));
        return false;
    }

    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_resolve_interaction(
          historyObserver_,
          reinterpret_cast<const std::uint8_t*>(encoded.constData()),
          static_cast<std::size_t>(encoded.size()),
          &rawError)) {
        QString message = copyError(rawError);
        if (message.isEmpty())
            message = tr("The Codex response could not be sent.");
        setConversationErrorMessage(message);
        return false;
    }

    interactionModel_.setResolving(interactionId, true);
    setConversationErrorMessage({});
    return true;
}

void
CodexHistoryController::updateErrorMessage()
{
    if (!threadStartErrorMessage_.isEmpty()) {
        setErrorMessage(threadStartErrorMessage_);
        return;
    }
    setErrorMessage(conversationErrorMessage_.isEmpty() ? threadErrorMessage_ : conversationErrorMessage_);
}

void
CodexHistoryController::finishThreadLoading(const QString& errorMessage)
{
    const bool wasLoading = std::exchange(loadingThreads_, false);
    setThreadErrorMessage(errorMessage);
    if (wasLoading)
        emit loadingChanged();
}

void
CodexHistoryController::finishConversationLoading(const QString& errorMessage)
{
    const bool wasLoading = std::exchange(loadingConversation_, false);
    setConversationErrorMessage(errorMessage);
    if (wasLoading)
        emit loadingChanged();
}

void
CodexHistoryController::handleHistoryEvent(void* context, const WardBuffer* event)
{
    if (context == nullptr || event == nullptr)
        return;

    auto* callbackContext = static_cast<CodexHistoryCallbackContext*>(context);
    CodexHistoryController* controller = callbackContext->controller;
    const std::uint64_t generation = callbackContext->generation;
    const auto* data = reinterpret_cast<const char*>(ward_core_buffer_data(event));
    const auto size = qsizetype(ward_core_buffer_size(event));
    ward::codex::v1::HistoryEvent historyEvent;
    QString decodingError = deserializeBytes(QByteArrayView(data, size), historyEvent);

    auto apply = [controller,
                  generation,
                  historyEvent = std::move(historyEvent),
                  decodingError = std::move(decodingError)]() mutable {
        if (controller->observerGeneration_ == generation)
            controller->applyHistoryEvent(std::move(historyEvent), decodingError);
    };
    QMetaObject::invokeMethod(controller, std::move(apply), Qt::QueuedConnection);
}
