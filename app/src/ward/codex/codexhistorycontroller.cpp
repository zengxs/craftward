// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexhistorycontroller.h"

#include "ward/coreffi.h"

#include <QByteArray>
#include <QByteArrayView>
#include <QMetaObject>
#include <QThread>
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
}

CodexHistoryController::CodexHistoryController(QObject* parent)
  : QObject(parent)
  , threadModel_(this)
  , messageModel_(this)
{
    ++observerGeneration_;
    callbackContext_ = std::make_unique<CodexHistoryCallbackContext>(CodexHistoryCallbackContext{
      .controller = this,
      .generation = observerGeneration_,
    });

    WardError* rawError = nullptr;
    const QByteArray executable = codexExecutable();
    historyObserver_ = ward_core_codex_history_observer_open(
      executable.constData(), handleHistoryEvent, callbackContext_.get(), &rawError);
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

CodexMessageModel*
CodexHistoryController::messages()
{
    return &messageModel_;
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

void
CodexHistoryController::refresh()
{
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
    messageModel_.clear();
    setConversationErrorMessage({});
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

void
CodexHistoryController::clearError()
{
    threadErrorMessage_.clear();
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
    if (threadErrorMessage_ == message)
        return;
    threadErrorMessage_ = message;
    updateErrorMessage();
}

void
CodexHistoryController::setConversationErrorMessage(const QString& message)
{
    if (conversationErrorMessage_ == message)
        return;
    conversationErrorMessage_ = message;
    updateErrorMessage();
}

void
CodexHistoryController::updateErrorMessage()
{
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
    if (QThread::currentThread() == controller->thread()) {
        apply();
    } else {
        QMetaObject::invokeMethod(controller, std::move(apply), Qt::QueuedConnection);
    }
}

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
            auto messages = conversation.messages();
            messageModel_.reconcileMessages(std::move(messages));
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
