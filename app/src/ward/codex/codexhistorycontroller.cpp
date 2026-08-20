// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexhistorycontroller.h"

#include "ward/coreffierror.h"

#include <ward_core.h>

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
template<typename Message>
QString
deserializeBytes(QByteArrayView bytes, Message& message)
{
    QProtobufSerializer serializer;
    if (message.deserialize(&serializer, bytes))
        return {};
    return QStringLiteral("Failed to decode Ward Core response: %1").arg(serializer.lastErrorString());
}

}

CodexHistoryController::CodexHistoryController(const WardRuntime* runtime,
                                               const WardCodexExecutionTarget* executionTarget,
                                               QObject* parent)
  : QObject(parent)
  , threadModel_(this)
  , conversationController_(this, this)
{
    connect(&conversationController_,
            &CodexConversationController::errorMessageChanged,
            this,
            &CodexHistoryController::handleConversationErrorChanged);
    ++observerGeneration_;
    callbackContext_ = std::make_unique<CodexHistoryCallbackContext>(CodexHistoryCallbackContext{
      .controller = this,
      .generation = observerGeneration_,
    });

    WardError* rawError = nullptr;
    historyObserver_ = ward_core_codex_history_observer_open(
      runtime, executionTarget, handleHistoryEvent, callbackContext_.get(), &rawError);
    if (historyObserver_ == nullptr) {
        callbackContext_.reset();
        loadingThreads_ = false;
        QString message = ward::coreffi::takeErrorMessage(rawError);
        if (message.isEmpty())
            message = tr("The Codex history observer could not be started.");
        setThreadErrorMessage(message);
        conversationController_.setObserverUnavailable(message);
    } else {
        conversationController_.setObserver(historyObserver_);
    }
}

CodexHistoryController::~CodexHistoryController()
{
    ++observerGeneration_;
    conversationController_.setObserver(nullptr);
    if (historyObserver_ != nullptr)
        ward_core_codex_history_observer_destroy(std::exchange(historyObserver_, nullptr));
    callbackContext_.reset();
}

CodexThreadModel*
CodexHistoryController::threads()
{
    return &threadModel_;
}

CodexConversationController*
CodexHistoryController::conversation()
{
    return &conversationController_;
}

bool
CodexHistoryController::showingArchived() const
{
    return showingArchived_;
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
CodexHistoryController::startingThread() const
{
    return startingThread_;
}

bool
CodexHistoryController::forkingThread() const
{
    return forkingThread_;
}

bool
CodexHistoryController::changingThreadLifecycle() const
{
    return changingThreadLifecycle_;
}

bool
CodexHistoryController::threadCreationInFlight() const
{
    return startingThread_ || forkingThread_;
}

void
CodexHistoryController::refresh()
{
    if (threadCreationInFlight() || conversationController_.turnInFlight())
        return;
    if (historyObserver_ == nullptr) {
        const QString message = tr("The Codex history observer is unavailable.");
        setThreadErrorMessage(message);
        conversationController_.failRefresh(message);
        return;
    }

    if (!std::exchange(loadingThreads_, true))
        emit loadingChanged();
    conversationController_.beginRefresh();

    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_refresh_async(historyObserver_, &rawError)) {
        QString message = ward::coreffi::takeErrorMessage(rawError);
        if (message.isEmpty())
            message = tr("The Codex history could not be refreshed.");
        finishThreadLoading(message);
        conversationController_.failRefresh(message);
    }
}

bool
CodexHistoryController::showArchivedThreads(bool archived)
{
    if (showingArchived_ == archived)
        return true;
    if (loadingThreads_ || conversationController_.loading() || conversationController_.mutationInFlight())
        return false;
    if (historyObserver_ == nullptr) {
        setThreadErrorMessage(tr("The Codex history observer is unavailable."));
        return false;
    }

    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_show_archived_async(historyObserver_, archived, &rawError)) {
        QString message = ward::coreffi::takeErrorMessage(rawError);
        if (message.isEmpty())
            message = tr("The displayed Codex history could not be changed.");
        setThreadErrorMessage(message);
        return false;
    }

    showingArchived_ = archived;
    emit historyScopeChanged();
    threadModel_.reconcileThreads({});
    clearSelection();
    setThreadErrorMessage({});
    loadingThreads_ = true;
    emit loadingChanged();
    return true;
}

void
CodexHistoryController::selectThread(const QString& threadId, const QString& title)
{
    if (threadId.isEmpty() || threadCreationInFlight())
        return;
    if (conversationController_.turnInFlight() && threadId != conversationController_.threadId())
        return;
    if (threadId == conversationController_.threadId()) {
        conversationController_.updateTitle(title);
        return;
    }

    const QByteArray encodedThreadId = threadId.toUtf8();
    conversationController_.beginLoadingThread(threadId, title);

    if (historyObserver_ == nullptr) {
        conversationController_.finishLoading(tr("The Codex history observer is unavailable."));
        return;
    }

    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_watch_async(historyObserver_, encodedThreadId.constData(), &rawError)) {
        QString message = ward::coreffi::takeErrorMessage(rawError);
        if (message.isEmpty())
            message = tr("The Codex conversation could not be observed.");
        conversationController_.finishLoading(message);
    }
}

bool
CodexHistoryController::renameSelectedThread(const QString& name)
{
    const QString normalizedName = name.trimmed();
    if (conversationController_.threadId().isEmpty() || normalizedName.isEmpty() ||
        normalizedName == conversationController_.title().trimmed() || showingArchived_ ||
        conversationController_.mutationInFlight() || conversationController_.loading())
        return false;
    if (historyObserver_ == nullptr) {
        conversationController_.setErrorMessage(tr("The Codex history observer is unavailable."));
        return false;
    }

    const QByteArray threadId = conversationController_.threadId().toUtf8();
    const QByteArray encodedName = normalizedName.toUtf8();
    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_rename_thread_async(
          historyObserver_, threadId.constData(), encodedName.constData(), &rawError)) {
        QString message = ward::coreffi::takeErrorMessage(rawError);
        if (message.isEmpty())
            message = tr("The Codex conversation could not be renamed.");
        conversationController_.setErrorMessage(message);
        return false;
    }

    conversationController_.setErrorMessage({});
    return true;
}

bool
CodexHistoryController::forkSelectedThread(const QString& lastTurnId)
{
    if (conversationController_.threadId().isEmpty() || lastTurnId.trimmed().isEmpty() || showingArchived_ ||
        loadingThreads_ || conversationController_.loading() || conversationController_.mutationInFlight() ||
        !conversationController_.forkReady())
        return false;
    if (historyObserver_ == nullptr) {
        conversationController_.setErrorMessage(tr("The Codex history observer is unavailable."));
        return false;
    }

    const QByteArray threadId = conversationController_.threadId().toUtf8();
    const QByteArray encodedLastTurnId = lastTurnId.toUtf8();
    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_fork_thread_async(
          historyObserver_, threadId.constData(), encodedLastTurnId.constData(), &rawError)) {
        QString message = ward::coreffi::takeErrorMessage(rawError);
        if (message.isEmpty())
            message = tr("The Codex conversation could not be forked.");
        conversationController_.setErrorMessage(message);
        return false;
    }

    conversationController_.setErrorMessage({});
    setForkingThread(true, conversationController_.threadId());
    if (!std::exchange(loadingThreads_, true))
        emit loadingChanged();
    return true;
}

bool
CodexHistoryController::archiveSelectedThread()
{
    return changeSelectedThreadLifecycle(ThreadLifecycleAction::Archive);
}

bool
CodexHistoryController::restoreSelectedThread()
{
    return changeSelectedThreadLifecycle(ThreadLifecycleAction::Restore);
}

bool
CodexHistoryController::changeSelectedThreadLifecycle(ThreadLifecycleAction action)
{
    const bool actionMatchesScope = (action == ThreadLifecycleAction::Archive && !showingArchived_) ||
                                    (action == ThreadLifecycleAction::Restore && showingArchived_);
    if (conversationController_.threadId().isEmpty() || !actionMatchesScope || loadingThreads_ ||
        conversationController_.loading() || conversationController_.mutationInFlight())
        return false;
    if (historyObserver_ == nullptr) {
        conversationController_.setErrorMessage(tr("The Codex history observer is unavailable."));
        return false;
    }

    const QByteArray threadId = conversationController_.threadId().toUtf8();
    WardError* rawError = nullptr;
    bool queued = false;
    QString fallbackError;
    switch (action) {
        case ThreadLifecycleAction::Archive:
            queued =
              ward_core_codex_history_observer_archive_thread_async(historyObserver_, threadId.constData(), &rawError);
            fallbackError = tr("The Codex conversation could not be archived.");
            break;
        case ThreadLifecycleAction::Restore:
            queued =
              ward_core_codex_history_observer_restore_thread_async(historyObserver_, threadId.constData(), &rawError);
            fallbackError = tr("The Codex conversation could not be restored.");
            break;
    }
    if (!queued) {
        QString message = ward::coreffi::takeErrorMessage(rawError);
        if (message.isEmpty())
            message = fallbackError;
        conversationController_.setErrorMessage(message);
        return false;
    }

    conversationController_.setErrorMessage({});
    setChangingThreadLifecycle(true, conversationController_.threadId());
    loadingThreads_ = true;
    emit loadingChanged();
    return true;
}

bool
CodexHistoryController::startThread(const QUrl& workingDirectory)
{
    if (showingArchived_ || conversationController_.mutationInFlight() || loadingThreads_ ||
        conversationController_.loading())
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
    if (!ward_core_codex_history_observer_start_thread_async(historyObserver_, encodedPath.constData(), &rawError)) {
        QString message = ward::coreffi::takeErrorMessage(rawError);
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
CodexHistoryController::clearError()
{
    threadErrorMessage_.clear();
    threadStartErrorMessage_.clear();
    conversationController_.setErrorMessage({});
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
CodexHistoryController::handleConversationErrorChanged()
{
    if (!conversationController_.errorMessage().isEmpty())
        threadStartErrorMessage_.clear();
    updateErrorMessage();
}

void
CodexHistoryController::clearSelection()
{
    setForkingThread(false);
    conversationController_.clearSelection();
}

void
CodexHistoryController::setChangingThreadLifecycle(bool changing, const QString& threadId)
{
    const QString pendingThreadId = changing ? threadId : QString();
    if (changingThreadLifecycle_ == changing && pendingLifecycleThreadId_ == pendingThreadId)
        return;
    changingThreadLifecycle_ = changing;
    pendingLifecycleThreadId_ = pendingThreadId;
    emit changingThreadLifecycleChanged();
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
CodexHistoryController::setForkingThread(bool forking, const QString& sourceThreadId)
{
    const QString pendingSource = forking ? sourceThreadId : QString();
    if (forkingThread_ == forking && pendingForkSourceThreadId_ == pendingSource)
        return;
    forkingThread_ = forking;
    pendingForkSourceThreadId_ = pendingSource;
    emit forkingThreadChanged();
}

void
CodexHistoryController::updateErrorMessage()
{
    if (!threadStartErrorMessage_.isEmpty()) {
        setErrorMessage(threadStartErrorMessage_);
        return;
    }
    const QString conversationError = conversationController_.errorMessage();
    setErrorMessage(conversationError.isEmpty() ? threadErrorMessage_ : conversationError);
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
