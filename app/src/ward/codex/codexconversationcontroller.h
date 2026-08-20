// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "codexinteractionmodel.h"
#include "codexmodelcatalogmodel.h"
#include "codextimelinemodel.h"

#include <QHash>
#include <QObject>
#include <QString>
#include <QUrl>
#include <QVariantList>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>

namespace ward::codex::v1 {
class Conversation;
class HistoryEvent;
class PendingInteractionResponse;
}

class CodexHistoryController;
struct WardCodexHistoryObserver;

class CodexConversationController : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("CodexConversationController is provided by CodexHistoryController.")
    Q_PROPERTY(CodexModelCatalogModel* modelCatalog READ modelCatalog CONSTANT)
    Q_PROPERTY(CodexTimelineModel* timeline READ timeline CONSTANT)
    Q_PROPERTY(CodexInteractionModel* interactions READ interactions CONSTANT)
    Q_PROPERTY(QString threadId READ threadId NOTIFY selectionChanged)
    Q_PROPERTY(QString title READ title NOTIFY selectionChanged)
    Q_PROPERTY(QString model READ model NOTIFY modelChanged)
    Q_PROPERTY(QString reasoningEffort READ reasoningEffort NOTIFY reasoningEffortChanged)
    Q_PROPERTY(QVariantList reasoningEfforts READ reasoningEfforts NOTIFY reasoningEffortsChanged)
    Q_PROPERTY(bool loadingModelCatalog READ loadingModelCatalog NOTIFY modelCatalogStateChanged)
    Q_PROPERTY(QString modelCatalogErrorMessage READ modelCatalogErrorMessage NOTIFY modelCatalogStateChanged)
    Q_PROPERTY(bool loading READ loading NOTIFY loadingChanged)
    Q_PROPERTY(bool activityHistoryPartial READ activityHistoryPartial NOTIFY activityHistoryPartialChanged)
    Q_PROPERTY(TurnState turnState READ turnState NOTIFY turnStateChanged)
    Q_PROPERTY(bool turnInFlight READ turnInFlight NOTIFY turnStateChanged)
    Q_PROPERTY(bool turnRunning READ turnRunning NOTIFY turnStateChanged)
    Q_PROPERTY(QString activeTurnId READ activeTurnId NOTIFY turnStateChanged)
    Q_PROPERTY(bool waitingOnApproval READ waitingOnApproval NOTIFY turnStateChanged)
    Q_PROPERTY(bool waitingOnUserInput READ waitingOnUserInput NOTIFY turnStateChanged)
    Q_PROPERTY(bool steeringTurn READ steeringTurn NOTIFY steeringTurnChanged)
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

    explicit CodexConversationController(CodexHistoryController* history, QObject* parent = nullptr);

    [[nodiscard]] CodexModelCatalogModel* modelCatalog();
    [[nodiscard]] CodexTimelineModel* timeline();
    [[nodiscard]] CodexInteractionModel* interactions();
    [[nodiscard]] QString threadId() const;
    [[nodiscard]] QString title() const;
    [[nodiscard]] QString model() const;
    [[nodiscard]] QString reasoningEffort() const;
    [[nodiscard]] QVariantList reasoningEfforts() const;
    [[nodiscard]] QString errorMessage() const;
    [[nodiscard]] bool loadingModelCatalog() const;
    [[nodiscard]] QString modelCatalogErrorMessage() const;
    [[nodiscard]] bool loading() const;
    [[nodiscard]] bool activityHistoryPartial() const;
    [[nodiscard]] TurnState turnState() const;
    [[nodiscard]] bool turnInFlight() const;
    [[nodiscard]] bool turnRunning() const;
    [[nodiscard]] QString activeTurnId() const;
    [[nodiscard]] bool waitingOnApproval() const;
    [[nodiscard]] bool waitingOnUserInput() const;
    [[nodiscard]] bool steeringTurn() const;
    [[nodiscard]] bool interruptRequested() const;
    [[nodiscard]] TurnMode turnMode() const;
    void setTurnMode(TurnMode mode);
    [[nodiscard]] PermissionPreset permissionPreset() const;
    void setPermissionPreset(PermissionPreset preset);
    [[nodiscard]] WriteAvailability writeAvailability() const;
    [[nodiscard]] QString writeAvailabilityMessage() const;

    Q_INVOKABLE void acquireWriteAccess();
    Q_INVOKABLE void releaseWriteAccess();
    Q_INVOKABLE QVariantList describeAttachments(const QList<QUrl>& attachments);
    Q_INVOKABLE QVariantList attachmentsFromClipboard();
    Q_INVOKABLE bool startTurn(const QString& prompt, const QList<QUrl>& attachments);
    Q_INVOKABLE bool selectModel(const QString& model);
    Q_INVOKABLE bool selectReasoningEffort(const QString& effort);
    Q_INVOKABLE bool steerTurn(const QString& prompt);
    Q_INVOKABLE bool interruptTurn();
    Q_INVOKABLE bool respondToApproval(const QString& interactionId, InteractionDecision decision);
    Q_INVOKABLE bool respondToUserInput(const QString& interactionId, const QVariantMap& answers);

  signals:
    void selectionChanged();
    void modelChanged();
    void reasoningEffortChanged();
    void reasoningEffortsChanged();
    void loadingChanged();
    void modelCatalogStateChanged();
    void activityHistoryPartialChanged();
    void turnStateChanged();
    void steeringTurnChanged();
    void interruptRequestedChanged();
    void turnOptionsChanged();
    void turnStarted();
    void turnSteered();
    void writeAvailabilityChanged();
    void errorMessageChanged();

  private:
    friend class CodexHistoryController;
    friend class CodexHistoryControllerTest;

    struct TurnRuntimeState
    {
        TurnState status = TurnState::Detached;
        QString activeTurnId;
        bool waitingOnApproval = false;
        bool waitingOnUserInput = false;
    };

    struct InferenceState
    {
        QString model;
        QString reasoningEffort;
    };

    struct ThreadInferenceSelection
    {
        InferenceState active;
        InferenceState selected;
    };

    void setObserver(WardCodexHistoryObserver* observer);
    void setObserverUnavailable(const QString& message);
    void beginRefresh();
    void failRefresh(const QString& message);
    void beginLoadingThread(const QString& threadId, const QString& title);
    void updateTitle(const QString& title);
    [[nodiscard]] bool forkReady() const;
    void applyDecodingError(const QString& message);
    [[nodiscard]] bool applyHistoryEvent(ward::codex::v1::HistoryEvent& event);
    void adoptConversation(const QString& threadId, const ward::codex::v1::Conversation& conversation);
    void clearSelection();
    void setErrorMessage(const QString& message);
    void setActivityHistoryPartial(bool partial);
    void setTurnState(TurnState state,
                      const QString& activeTurnId = {},
                      bool waitingOnApproval = false,
                      bool waitingOnUserInput = false);
    void setSteeringTurn(bool steering);
    void setWriteAvailability(WriteAvailability availability, const QString& message = {});
    void setInterruptRequested(bool requested);
    void applyThreadInferenceOptions(const QString& threadId, const QString& model, const QString& reasoningEffort);
    void restoreInferenceSelection();
    void reconcileInferenceSelections();
    void setDisplayedModel(const QString& model);
    void setDisplayedReasoningEffort(const QString& effort);
    [[nodiscard]] QString modelOverride() const;
    [[nodiscard]] QString reasoningEffortOverride() const;
    [[nodiscard]] bool mutationInFlight() const;
    bool sendInteractionResponse(const QString& interactionId,
                                 const ward::codex::v1::PendingInteractionResponse& response);
    void finishModelCatalogLoading(const QString& errorMessage);
    void finishLoading(const QString& errorMessage);

    CodexHistoryController* history_;
    WardCodexHistoryObserver* observer_ = nullptr;
    CodexModelCatalogModel modelCatalogModel_;
    CodexTimelineModel timelineModel_;
    CodexInteractionModel interactionModel_;
    QString threadId_;
    QString title_;
    QHash<QString, ThreadInferenceSelection> inferenceSelections_;
    QString model_;
    QString reasoningEffort_;
    QString errorMessage_;
    bool loadingModelCatalog_ = true;
    QString modelCatalogErrorMessage_;
    bool loading_ = false;
    bool activityHistoryPartial_ = false;
    TurnRuntimeState turnRuntimeState_;
    bool steeringTurn_ = false;
    bool interruptRequested_ = false;
    TurnMode turnMode_ = TurnMode::DefaultMode;
    PermissionPreset permissionPreset_ = PermissionPreset::InheritPermissions;
    WriteAvailability writeAvailability_ = WriteAvailability::NotRequested;
    QString writeAvailabilityMessage_;
};
