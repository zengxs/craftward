// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "codexconversationcontroller.h"
#include "codexthreadmodel.h"

#include <QObject>
#include <QString>
#include <QUrl>
#include <QtQml/qqmlregistration.h>

#include <cstdint>
#include <memory>

struct CodexHistoryCallbackContext;
struct WardBuffer;
struct WardCodexHistoryObserver;
struct WardRuntime;

namespace ward::codex::v1 {
class HistoryEvent;
}

class CodexHistoryController : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("CodexHistoryController is provided by the application.")
    Q_PROPERTY(CodexThreadModel* threads READ threads CONSTANT)
    Q_PROPERTY(CodexConversationController* conversation READ conversation CONSTANT)
    Q_PROPERTY(bool showingArchived READ showingArchived NOTIFY historyScopeChanged)
    Q_PROPERTY(QString errorMessage READ errorMessage NOTIFY errorMessageChanged)
    Q_PROPERTY(bool loadingThreads READ loadingThreads NOTIFY loadingChanged)
    Q_PROPERTY(bool startingThread READ startingThread NOTIFY startingThreadChanged)
    Q_PROPERTY(bool forkingThread READ forkingThread NOTIFY forkingThreadChanged)
    Q_PROPERTY(bool changingThreadLifecycle READ changingThreadLifecycle NOTIFY changingThreadLifecycleChanged)

  public:
    explicit CodexHistoryController(const WardRuntime* runtime, QObject* parent = nullptr);
    ~CodexHistoryController() override;

    [[nodiscard]] CodexThreadModel* threads();
    [[nodiscard]] CodexConversationController* conversation();
    [[nodiscard]] bool showingArchived() const;
    [[nodiscard]] QString errorMessage() const;
    [[nodiscard]] bool loadingThreads() const;
    [[nodiscard]] bool startingThread() const;
    [[nodiscard]] bool forkingThread() const;
    [[nodiscard]] bool changingThreadLifecycle() const;

    Q_INVOKABLE void refresh();
    Q_INVOKABLE bool showArchivedThreads(bool archived);
    Q_INVOKABLE void selectThread(const QString& threadId, const QString& title);
    Q_INVOKABLE bool renameSelectedThread(const QString& name);
    Q_INVOKABLE bool forkSelectedThread(const QString& lastTurnId);
    Q_INVOKABLE bool archiveSelectedThread();
    Q_INVOKABLE bool restoreSelectedThread();
    Q_INVOKABLE bool startThread(const QUrl& workingDirectory);
    Q_INVOKABLE void clearError();

  signals:
    void historyScopeChanged();
    void errorMessageChanged();
    void loadingChanged();
    void startingThreadChanged();
    void forkingThreadChanged();
    void changingThreadLifecycleChanged();

  private:
    friend class CodexHistoryControllerTest;

    enum class ThreadLifecycleAction
    {
        Archive,
        Restore,
    };

    static void handleHistoryEvent(void* context, const WardBuffer* event);

    void setErrorMessage(const QString& message);
    void setThreadErrorMessage(const QString& message);
    void setThreadStartErrorMessage(const QString& message);
    void handleConversationErrorChanged();
    void clearSelection();
    bool changeSelectedThreadLifecycle(ThreadLifecycleAction action);
    void setChangingThreadLifecycle(bool changing, const QString& threadId = {});
    void setStartingThread(bool starting);
    void setForkingThread(bool forking, const QString& sourceThreadId = {});
    [[nodiscard]] bool threadCreationInFlight() const;
    void updateErrorMessage();
    void finishThreadLoading(const QString& errorMessage);
    void applyHistoryEvent(ward::codex::v1::HistoryEvent event, const QString& decodingError);

    CodexThreadModel threadModel_;
    CodexConversationController conversationController_;
    WardCodexHistoryObserver* historyObserver_ = nullptr;
    std::unique_ptr<CodexHistoryCallbackContext> callbackContext_;
    bool showingArchived_ = false;
    QString errorMessage_;
    QString threadErrorMessage_;
    QString threadStartErrorMessage_;
    std::uint64_t observerGeneration_ = 0;
    bool loadingThreads_ = true;
    bool startingThread_ = false;
    bool forkingThread_ = false;
    QString pendingForkSourceThreadId_;
    bool changingThreadLifecycle_ = false;
    QString pendingLifecycleThreadId_;
};
