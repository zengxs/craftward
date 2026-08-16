// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "codexinteractionmodel.h"
#include "codexthreadmodel.h"
#include "codextimelinemodel.h"

#include <QObject>
#include <QString>
#include <QUrl>
#include <QVariantMap>
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
    Q_PROPERTY(CodexInteractionModel* interactions READ interactions CONSTANT)
    Q_PROPERTY(QString selectedThreadId READ selectedThreadId NOTIFY selectionChanged)
    Q_PROPERTY(QString selectedThreadTitle READ selectedThreadTitle NOTIFY selectionChanged)
    Q_PROPERTY(QString errorMessage READ errorMessage NOTIFY errorMessageChanged)
    Q_PROPERTY(bool loadingThreads READ loadingThreads NOTIFY loadingChanged)
    Q_PROPERTY(bool loadingConversation READ loadingConversation NOTIFY loadingChanged)
    Q_PROPERTY(bool startingThread READ startingThread NOTIFY startingThreadChanged)
    Q_PROPERTY(bool activityHistoryPartial READ activityHistoryPartial NOTIFY activityHistoryPartialChanged)
    Q_PROPERTY(TurnState turnState READ turnState NOTIFY turnStateChanged)
    Q_PROPERTY(bool turnInFlight READ turnInFlight NOTIFY turnStateChanged)
    Q_PROPERTY(bool turnRunning READ turnRunning NOTIFY turnStateChanged)
    Q_PROPERTY(QString activeTurnId READ activeTurnId NOTIFY turnStateChanged)
    Q_PROPERTY(bool waitingOnApproval READ waitingOnApproval NOTIFY turnStateChanged)
    Q_PROPERTY(bool waitingOnUserInput READ waitingOnUserInput NOTIFY turnStateChanged)
    Q_PROPERTY(bool interruptRequested READ interruptRequested NOTIFY interruptRequestedChanged)
    Q_PROPERTY(TurnMode turnMode READ turnMode WRITE setTurnMode NOTIFY turnOptionsChanged)
    Q_PROPERTY(
      PermissionPreset permissionPreset READ permissionPreset WRITE setPermissionPreset NOTIFY turnOptionsChanged)
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
        NotRequested,
        Checking,
        Writable,
        Busy,
        Unavailable,
    };
    Q_ENUM(WriteAvailability)

    enum class TurnMode
    {
        DefaultMode = 0,
        PlanMode = 1,
    };
    Q_ENUM(TurnMode)

    enum class PermissionPreset
    {
        InheritPermissions = 0,
        RequestApproval = 1,
        ReadOnlyPermissions = 2,
    };
    Q_ENUM(PermissionPreset)

    enum class InteractionKind
    {
        CommandApproval = static_cast<int>(ward::codex::v1::PendingInteractionKindGadget::PendingInteractionKind::
                                             PENDING_INTERACTION_KIND_COMMAND_APPROVAL),
        FileChangeApproval = static_cast<int>(ward::codex::v1::PendingInteractionKindGadget::PendingInteractionKind::
                                                PENDING_INTERACTION_KIND_FILE_CHANGE_APPROVAL),
        UserInput = static_cast<int>(
          ward::codex::v1::PendingInteractionKindGadget::PendingInteractionKind::PENDING_INTERACTION_KIND_USER_INPUT),
    };
    Q_ENUM(InteractionKind)

    enum class InteractionDecision
    {
        Accept = static_cast<int>(ward::codex::v1::PendingInteractionDecisionGadget::PendingInteractionDecision::
                                    PENDING_INTERACTION_DECISION_ACCEPT),
        AcceptForSession =
          static_cast<int>(ward::codex::v1::PendingInteractionDecisionGadget::PendingInteractionDecision::
                             PENDING_INTERACTION_DECISION_ACCEPT_FOR_SESSION),
        Decline = static_cast<int>(ward::codex::v1::PendingInteractionDecisionGadget::PendingInteractionDecision::
                                     PENDING_INTERACTION_DECISION_DECLINE),
        Cancel = static_cast<int>(ward::codex::v1::PendingInteractionDecisionGadget::PendingInteractionDecision::
                                    PENDING_INTERACTION_DECISION_CANCEL),
    };
    Q_ENUM(InteractionDecision)

    explicit CodexHistoryController(const WardRuntime* runtime, QObject* parent = nullptr);
    ~CodexHistoryController() override;

    [[nodiscard]] CodexThreadModel* threads();
    [[nodiscard]] CodexTimelineModel* timeline();
    [[nodiscard]] CodexInteractionModel* interactions();
    [[nodiscard]] QString selectedThreadId() const;
    [[nodiscard]] QString selectedThreadTitle() const;
    [[nodiscard]] QString errorMessage() const;
    [[nodiscard]] bool loadingThreads() const;
    [[nodiscard]] bool loadingConversation() const;
    [[nodiscard]] bool startingThread() const;
    [[nodiscard]] bool activityHistoryPartial() const;
    [[nodiscard]] TurnState turnState() const;
    [[nodiscard]] bool turnInFlight() const;
    [[nodiscard]] bool turnRunning() const;
    [[nodiscard]] QString activeTurnId() const;
    [[nodiscard]] bool waitingOnApproval() const;
    [[nodiscard]] bool waitingOnUserInput() const;
    [[nodiscard]] bool interruptRequested() const;
    [[nodiscard]] TurnMode turnMode() const;
    void setTurnMode(TurnMode mode);
    [[nodiscard]] PermissionPreset permissionPreset() const;
    void setPermissionPreset(PermissionPreset preset);
    [[nodiscard]] WriteAvailability writeAvailability() const;
    [[nodiscard]] QString writeAvailabilityMessage() const;

    Q_INVOKABLE void refresh();
    Q_INVOKABLE void selectThread(const QString& threadId, const QString& title);
    Q_INVOKABLE bool startThread(const QUrl& workingDirectory);
    Q_INVOKABLE void acquireWriteAccess();
    Q_INVOKABLE void releaseWriteAccess();
    Q_INVOKABLE bool startTurn(const QString& prompt);
    Q_INVOKABLE bool interruptTurn();
    Q_INVOKABLE bool respondToApproval(const QString& interactionId, InteractionDecision decision);
    Q_INVOKABLE bool respondToUserInput(const QString& interactionId, const QVariantMap& answers);
    Q_INVOKABLE void clearError();

  signals:
    void selectionChanged();
    void errorMessageChanged();
    void loadingChanged();
    void startingThreadChanged();
    void activityHistoryPartialChanged();
    void turnStateChanged();
    void interruptRequestedChanged();
    void turnOptionsChanged();
    void turnStarted();
    void writeAvailabilityChanged();

  private:
    friend class CodexHistoryControllerTest;

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
    void setThreadStartErrorMessage(const QString& message);
    void setConversationErrorMessage(const QString& message);
    void setStartingThread(bool starting);
    void setActivityHistoryPartial(bool partial);
    void setTurnState(TurnState state,
                      const QString& activeTurnId = {},
                      bool waitingOnApproval = false,
                      bool waitingOnUserInput = false);
    void setWriteAvailability(WriteAvailability availability, const QString& message = {});
    void setInterruptRequested(bool requested);
    bool sendInteractionResponse(const QString& interactionId,
                                 const ward::codex::v1::PendingInteractionResponse& response);
    void updateErrorMessage();
    void finishThreadLoading(const QString& errorMessage);
    void finishConversationLoading(const QString& errorMessage);
    void applyHistoryEvent(ward::codex::v1::HistoryEvent event, const QString& decodingError);

    CodexThreadModel threadModel_;
    CodexTimelineModel timelineModel_;
    CodexInteractionModel interactionModel_;
    WardCodexHistoryObserver* historyObserver_ = nullptr;
    std::unique_ptr<CodexHistoryCallbackContext> callbackContext_;
    QString selectedThreadId_;
    QString selectedThreadTitle_;
    QString errorMessage_;
    QString threadErrorMessage_;
    QString threadStartErrorMessage_;
    QString conversationErrorMessage_;
    std::uint64_t observerGeneration_ = 0;
    bool loadingThreads_ = true;
    bool loadingConversation_ = false;
    bool startingThread_ = false;
    bool activityHistoryPartial_ = false;
    TurnRuntimeState turnRuntimeState_;
    bool interruptRequested_ = false;
    TurnMode turnMode_ = TurnMode::DefaultMode;
    PermissionPreset permissionPreset_ = PermissionPreset::InheritPermissions;
    WriteAvailability writeAvailability_ = WriteAvailability::NotRequested;
    QString writeAvailabilityMessage_;
};
