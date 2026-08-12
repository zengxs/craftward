// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexhistorycontroller.h"

#include "ward/coreffi.h"

#include <QByteArray>
#include <QByteArrayView>
#include <QFutureWatcher>
#include <QMetaObject>
#include <QThread>
#include <QtConcurrent/QtConcurrentRun>
#include <QtProtobuf/QProtobufSerializer>

#include <memory>
#include <utility>

struct CodexHistoryCallbackContext
{
    CodexHistoryController* controller;
    std::uint64_t generation;
};

namespace {
struct ThreadLoadResult
{
    QList<CodexThreadSummary> threads;
    QString errorMessage;
};

struct ErrorDeleter
{
    void operator()(WardError* error) const { ward_core_error_destroy(error); }
};

struct BufferDeleter
{
    void operator()(WardBuffer* buffer) const { ward_core_buffer_destroy(buffer); }
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

template<typename Message>
QString
deserializeBuffer(const WardBuffer* buffer, Message& message)
{
    const auto* data = reinterpret_cast<const char*>(ward_core_buffer_data(buffer));
    const auto size = qsizetype(ward_core_buffer_size(buffer));
    return deserializeBytes(QByteArrayView(data, size), message);
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
        QString message = copyError(rawError);
        if (message.isEmpty())
            message = tr("The Codex history observer could not be started.");
        setErrorMessage(message);
    }
}

CodexHistoryController::~CodexHistoryController()
{
    ++threadGeneration_;
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
    const std::uint64_t generation = ++threadGeneration_;
    const QByteArray executable = codexExecutable();
    setErrorMessage({});
    loadingThreads_ = true;
    emit loadingChanged();

    auto* watcher = new QFutureWatcher<ThreadLoadResult>(this);
    connect(watcher, &QFutureWatcher<ThreadLoadResult>::finished, this, [this, watcher, generation] {
        ThreadLoadResult result = watcher->result();
        watcher->deleteLater();
        applyThreads(generation, std::move(result.threads), result.errorMessage);
    });
    watcher->setFuture(QtConcurrent::run([executable] {
        WardError* rawError = nullptr;
        std::unique_ptr<WardBuffer, BufferDeleter> payload(
          ward_core_codex_list_threads(executable.constData(), 100, &rawError));
        ThreadLoadResult result;
        if (payload == nullptr) {
            result.errorMessage = copyError(rawError);
        } else {
            ward::codex::v1::ThreadPage threadPage;
            result.errorMessage = deserializeBuffer(payload.get(), threadPage);
            if (result.errorMessage.isEmpty())
                result.threads = threadPage.threads();
        }
        return result;
    }));
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
    setErrorMessage({});
    loadingConversation_ = true;
    emit selectionChanged();
    emit loadingChanged();

    if (historyObserver_ == nullptr) {
        loadingConversation_ = false;
        if (errorMessage_.isEmpty())
            setErrorMessage(tr("The Codex history observer is unavailable."));
        emit loadingChanged();
        return;
    }

    WardError* rawError = nullptr;
    if (!ward_core_codex_history_observer_watch(historyObserver_, encodedThreadId.constData(), &rawError)) {
        loadingConversation_ = false;
        QString message = copyError(rawError);
        if (message.isEmpty())
            message = tr("The Codex conversation could not be observed.");
        setErrorMessage(message);
        emit loadingChanged();
    }
}

void
CodexHistoryController::clearError()
{
    setErrorMessage({});
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
CodexHistoryController::handleHistoryEvent(void* context, const WardCodexHistoryEvent* event)
{
    if (context == nullptr || event == nullptr || event->thread_id == nullptr)
        return;

    auto* callbackContext = static_cast<CodexHistoryCallbackContext*>(context);
    CodexHistoryController* controller = callbackContext->controller;
    const std::uint64_t generation = callbackContext->generation;
    const CodexHistoryEventKind kind = [event] {
        switch (event->kind) {
            case WardCodexHistoryEventUpdated:
                return CodexHistoryEventKind::Updated;
            case WardCodexHistoryEventRecovered:
                return CodexHistoryEventKind::Recovered;
            case WardCodexHistoryEventError:
                return CodexHistoryEventKind::Error;
        }
        return CodexHistoryEventKind::Unsupported;
    }();
    const QString threadId = copyString(event->thread_id);
    QString errorMessage = copyString(event->error_message);
    QString title;
    QList<CodexMessage> messages;
    if (event->conversation != nullptr) {
        const auto* data = reinterpret_cast<const char*>(ward_core_buffer_data(event->conversation));
        const auto size = qsizetype(ward_core_buffer_size(event->conversation));
        ward::codex::v1::Conversation conversation;
        errorMessage = deserializeBytes(QByteArrayView(data, size), conversation);
        if (errorMessage.isEmpty()) {
            title = conversation.title();
            messages = conversation.messages();
        }
    }

    auto apply =
      [controller, generation, kind, threadId, title, messages = std::move(messages), errorMessage]() mutable {
          if (controller->observerGeneration_ == generation)
              controller->applyHistoryEvent(kind, threadId, title, std::move(messages), errorMessage);
      };
    if (QThread::currentThread() == controller->thread()) {
        apply();
    } else {
        QMetaObject::invokeMethod(controller, std::move(apply), Qt::QueuedConnection);
    }
}

void
CodexHistoryController::applyThreads(std::uint64_t generation,
                                     QList<CodexThreadSummary> threads,
                                     const QString& errorMessage)
{
    if (generation != threadGeneration_)
        return;
    loadingThreads_ = false;
    if (errorMessage.isEmpty())
        threadModel_.replaceThreads(std::move(threads));
    setErrorMessage(errorMessage);
    emit loadingChanged();
}

void
CodexHistoryController::applyHistoryEvent(CodexHistoryEventKind kind,
                                          const QString& threadId,
                                          const QString& title,
                                          QList<CodexMessage> messages,
                                          const QString& errorMessage)
{
    if (threadId != selectedThreadId_)
        return;

    const bool wasLoading = std::exchange(loadingConversation_, false);
    switch (kind) {
        case CodexHistoryEventKind::Updated: {
            if (!errorMessage.isEmpty()) {
                setErrorMessage(errorMessage);
                break;
            }
            if (!title.trimmed().isEmpty() && title != selectedThreadTitle_) {
                selectedThreadTitle_ = title;
                emit selectionChanged();
            }
            messageModel_.reconcileMessages(std::move(messages));
            setErrorMessage({});
            break;
        }
        case CodexHistoryEventKind::Recovered:
            setErrorMessage({});
            break;
        case CodexHistoryEventKind::Error:
            setErrorMessage(errorMessage.isEmpty() ? tr("The Codex conversation could not be observed.")
                                                   : errorMessage);
            break;
        case CodexHistoryEventKind::Unsupported:
            setErrorMessage(tr("Ward Core returned an unsupported Codex history event."));
            break;
    }
    if (wasLoading)
        emit loadingChanged();
}
