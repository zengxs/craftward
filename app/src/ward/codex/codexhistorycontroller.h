// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "codexthreadmodel.h"
#include "codextimelinemodel.h"

#include <QObject>
#include <QString>
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
    Q_PROPERTY(CodexTimelineModel* timeline READ timeline CONSTANT)
    Q_PROPERTY(QString selectedThreadId READ selectedThreadId NOTIFY selectionChanged)
    Q_PROPERTY(QString selectedThreadTitle READ selectedThreadTitle NOTIFY selectionChanged)
    Q_PROPERTY(QString errorMessage READ errorMessage NOTIFY errorMessageChanged)
    Q_PROPERTY(bool loadingThreads READ loadingThreads NOTIFY loadingChanged)
    Q_PROPERTY(bool loadingConversation READ loadingConversation NOTIFY loadingChanged)
    Q_PROPERTY(bool activityHistoryPartial READ activityHistoryPartial NOTIFY activityHistoryPartialChanged)
    Q_PROPERTY(TurnState turnState READ turnState NOTIFY turnStateChanged)
    Q_PROPERTY(bool turnInFlight READ turnInFlight NOTIFY turnStateChanged)
    Q_PROPERTY(bool turnRunning READ turnRunning NOTIFY turnStateChanged)
    Q_PROPERTY(QString activeTurnId READ activeTurnId NOTIFY turnStateChanged)
    Q_PROPERTY(bool waitingOnApproval READ waitingOnApproval NOTIFY turnStateChanged)
    Q_PROPERTY(bool waitingOnUserInput READ waitingOnUserInput NOTIFY turnStateChanged)
    Q_PROPERTY(WriteAvailability writeAvailability READ writeAvailability NOTIFY writeAvailabilityChanged)
    Q_PROPERTY(QString writeAvailabilityMessage READ writeAvailabilityMessage NOTIFY writeAvailabilityChanged)

  public:
    enum class TurnState
    {
        Detached,
        Starting,
        Idle,
        Running,
        SystemError,
        Unknown,
    };
    Q_ENUM(TurnState)

    enum class WriteAvailability
    {
        Idle,
        Checking,
        Writable,
        Busy,
        Unavailable,
    };
    Q_ENUM(WriteAvailability)

    explicit CodexHistoryController(const WardRuntime* runtime, QObject* parent = nullptr);
    ~CodexHistoryController() override;

    [[nodiscard]] CodexThreadModel* threads();
    [[nodiscard]] CodexTimelineModel* timeline();
    [[nodiscard]] QString selectedThreadId() const;
    [[nodiscard]] QString selectedThreadTitle() const;
    [[nodiscard]] QString errorMessage() const;
    [[nodiscard]] bool loadingThreads() const;
    [[nodiscard]] bool loadingConversation() const;
    [[nodiscard]] bool activityHistoryPartial() const;
    [[nodiscard]] TurnState turnState() const;
    [[nodiscard]] bool turnInFlight() const;
    [[nodiscard]] bool turnRunning() const;
    [[nodiscard]] QString activeTurnId() const;
    [[nodiscard]] bool waitingOnApproval() const;
    [[nodiscard]] bool waitingOnUserInput() const;
    [[nodiscard]] WriteAvailability writeAvailability() const;
    [[nodiscard]] QString writeAvailabilityMessage() const;

    Q_INVOKABLE void refresh();
    Q_INVOKABLE void selectThread(const QString& threadId, const QString& title);
    Q_INVOKABLE void acquireWriteAccess();
    Q_INVOKABLE void releaseWriteAccess();
    Q_INVOKABLE bool startTurn(const QString& prompt);
    Q_INVOKABLE void clearError();

  signals:
    void selectionChanged();
    void errorMessageChanged();
    void loadingChanged();
    void activityHistoryPartialChanged();
    void turnStateChanged();
    void turnStarted();
    void writeAvailabilityChanged();

  private:
    struct TurnRuntimeState
    {
        TurnState status = TurnState::Detached;
        QString activeTurnId;
        bool waitingOnApproval = false;
        bool waitingOnUserInput = false;
    };

    static void handleHistoryEvent(void* context, const WardBuffer* event);

    void setErrorMessage(const QString& message);
    void setThreadErrorMessage(const QString& message);
    void setConversationErrorMessage(const QString& message);
    void setActivityHistoryPartial(bool partial);
    void setTurnState(TurnState state,
                      const QString& activeTurnId = {},
                      bool waitingOnApproval = false,
                      bool waitingOnUserInput = false);
    void setWriteAvailability(WriteAvailability availability, const QString& message = {});
    void updateErrorMessage();
    void finishThreadLoading(const QString& errorMessage);
    void finishConversationLoading(const QString& errorMessage);
    void applyHistoryEvent(ward::codex::v1::HistoryEvent event, const QString& decodingError);

    CodexThreadModel threadModel_;
    CodexTimelineModel timelineModel_;
    WardCodexHistoryObserver* historyObserver_ = nullptr;
    std::unique_ptr<CodexHistoryCallbackContext> callbackContext_;
    QString selectedThreadId_;
    QString selectedThreadTitle_;
    QString errorMessage_;
    QString threadErrorMessage_;
    QString conversationErrorMessage_;
    std::uint64_t observerGeneration_ = 0;
    bool loadingThreads_ = true;
    bool loadingConversation_ = false;
    bool activityHistoryPartial_ = false;
    TurnRuntimeState turnRuntimeState_;
    WriteAvailability writeAvailability_ = WriteAvailability::Idle;
    QString writeAvailabilityMessage_;
};
