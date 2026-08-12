// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexhistorycontroller.h"

#include "ward/coreffi.h"

#include <QByteArray>
#include <QByteArrayView>
#include <QFutureWatcher>
#include <QtConcurrent/QtConcurrentRun>
#include <QtProtobuf/QProtobufSerializer>

#include <memory>

namespace {
struct ThreadLoadResult
{
    QList<CodexThreadSummary> threads;
    QString errorMessage;
};

struct ConversationLoadResult
{
    QString title;
    QList<CodexMessage> messages;
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
deserializeBuffer(const WardBuffer* buffer, Message& message)
{
    const auto* data = reinterpret_cast<const char*>(ward_core_buffer_data(buffer));
    const auto size = qsizetype(ward_core_buffer_size(buffer));
    QProtobufSerializer serializer;
    if (message.deserialize(&serializer, QByteArrayView(data, size)))
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
}

CodexHistoryController::~CodexHistoryController()
{
    ++threadGeneration_;
    ++conversationGeneration_;
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

    const std::uint64_t generation = ++conversationGeneration_;
    const QByteArray executable = codexExecutable();
    const QByteArray encodedThreadId = threadId.toUtf8();
    selectedThreadId_ = threadId;
    selectedThreadTitle_ = title;
    messageModel_.clear();
    setErrorMessage({});
    loadingConversation_ = true;
    emit selectionChanged();
    emit loadingChanged();

    auto* watcher = new QFutureWatcher<ConversationLoadResult>(this);
    connect(watcher, &QFutureWatcher<ConversationLoadResult>::finished, this, [this, watcher, generation, threadId] {
        ConversationLoadResult result = watcher->result();
        watcher->deleteLater();
        applyConversation(generation, threadId, result.title, std::move(result.messages), result.errorMessage);
    });
    watcher->setFuture(QtConcurrent::run([executable, encodedThreadId, title] {
        WardError* rawError = nullptr;
        std::unique_ptr<WardBuffer, BufferDeleter> payload(
          ward_core_codex_read_thread(executable.constData(), encodedThreadId.constData(), &rawError));
        ConversationLoadResult result{ .title = title };
        if (payload == nullptr) {
            result.errorMessage = copyError(rawError);
        } else {
            ward::codex::v1::Conversation conversation;
            result.errorMessage = deserializeBuffer(payload.get(), conversation);
            if (result.errorMessage.isEmpty()) {
                if (!conversation.title().trimmed().isEmpty())
                    result.title = conversation.title();
                result.messages = conversation.messages();
            }
        }
        return result;
    }));
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
CodexHistoryController::applyConversation(std::uint64_t generation,
                                          const QString& threadId,
                                          const QString& title,
                                          QList<CodexMessage> messages,
                                          const QString& errorMessage)
{
    if (generation != conversationGeneration_ || threadId != selectedThreadId_)
        return;
    loadingConversation_ = false;
    if (errorMessage.isEmpty()) {
        selectedThreadTitle_ = title;
        messageModel_.replaceMessages(std::move(messages));
        emit selectionChanged();
    }
    setErrorMessage(errorMessage);
    emit loadingChanged();
}
