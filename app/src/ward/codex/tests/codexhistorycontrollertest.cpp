// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexhistorycontroller.h"
#include "ward/codex/codexattachmentinput.h"

#include <QCoreApplication>
#include <QDirIterator>
#include <QFile>
#include <QImage>
#include <QMimeData>
#include <QRegularExpression>
#include <QTemporaryDir>
#include <QTranslator>
#include <QtTest/QSignalSpy>
#include <QtTest/QTest>

namespace {
using Conversation = ward::codex::v1::Conversation;
using Activity = ward::codex::v1::Activity;
using ActivityKind = ward::codex::v1::ActivityKindGadget::ActivityKind;
using ActivityStatus = ward::codex::v1::ActivityStatusGadget::ActivityStatus;
using CommandAction = ward::codex::v1::CommandAction;
using CommandActionKind = ward::codex::v1::CommandActionKindGadget::CommandActionKind;
using HistoryEvent = ward::codex::v1::HistoryEvent;
using HistoryEventKind = ward::codex::v1::HistoryEventKindGadget::HistoryEventKind;
using Message = ward::codex::v1::Message;
using MessagePhase = ward::codex::v1::MessagePhaseGadget::MessagePhase;
using MessageRole = ward::codex::v1::MessageRoleGadget::MessageRole;
using PersistedTurnStatus = ward::codex::v1::PersistedTurnStatusGadget::PersistedTurnStatus;
using ThreadPage = ward::codex::v1::ThreadPage;
using ThreadRuntimeStatus = ward::codex::v1::ThreadRuntimeStatusGadget::ThreadRuntimeStatus;
using ThreadSummary = ward::codex::v1::ThreadSummary;
using TimelineItem = ward::codex::v1::TimelineItem;

HistoryEvent
modelCatalogEvent()
{
    ward::codex::v1::ReasoningEffortOption low;
    low.setReasoningEffort(QStringLiteral("low"));
    low.setDescription(QStringLiteral("Faster responses"));
    ward::codex::v1::ReasoningEffortOption medium;
    medium.setReasoningEffort(QStringLiteral("medium"));
    medium.setDescription(QStringLiteral("Balanced reasoning"));
    ward::codex::v1::ReasoningEffortOption high;
    high.setReasoningEffort(QStringLiteral("high"));
    high.setDescription(QStringLiteral("Deeper reasoning"));

    ward::codex::v1::ModelInfo model;
    model.setModelId(QStringLiteral("balanced"));
    model.setModel(QStringLiteral("gpt-balanced"));
    model.setDisplayName(QStringLiteral("Balanced"));
    model.setDescription(QStringLiteral("Balances capability and speed."));
    model.setIsDefault(true);
    model.setDefaultReasoningEffort(QStringLiteral("medium"));
    model.setSupportedReasoningEfforts({ std::move(low), std::move(medium), std::move(high) });

    ward::codex::v1::ReasoningEffortOption fastLow;
    fastLow.setReasoningEffort(QStringLiteral("low"));
    fastLow.setDescription(QStringLiteral("Faster responses"));
    ward::codex::v1::ReasoningEffortOption fastMedium;
    fastMedium.setReasoningEffort(QStringLiteral("medium"));
    fastMedium.setDescription(QStringLiteral("Balanced reasoning"));
    ward::codex::v1::ModelInfo fast;
    fast.setModelId(QStringLiteral("fast"));
    fast.setModel(QStringLiteral("gpt-fast"));
    fast.setDisplayName(QStringLiteral("Fast"));
    fast.setDescription(QStringLiteral("Optimized for quick iteration."));
    fast.setDefaultReasoningEffort(QStringLiteral("low"));
    fast.setSupportedReasoningEfforts({ std::move(fastLow), std::move(fastMedium) });

    ward::codex::v1::ModelCatalog catalog;
    catalog.setModels({ std::move(model), std::move(fast) });
    HistoryEvent event;
    event.setKind(HistoryEventKind::HISTORY_EVENT_KIND_MODEL_CATALOG_UPDATED);
    event.setModelCatalog(std::move(catalog));
    return event;
}

HistoryEvent
threadModelEvent(const QString& model,
                 const QString& threadId = QStringLiteral("thread-new"),
                 const QString& reasoningEffort = {})
{
    ward::codex::v1::ThreadModelState state;
    state.setModel(model);
    if (!reasoningEffort.isEmpty())
        state.setReasoningEffort(reasoningEffort);
    HistoryEvent event;
    event.setKind(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_MODEL_CHANGED);
    event.setThreadId(threadId);
    event.setThreadModelState(std::move(state));
    return event;
}

TimelineItem
messageItem(const QString& turnId, const QString& messageId, MessageRole role, MessagePhase phase, const QString& text)
{
    Message message;
    message.setMessageId(messageId);
    message.setRole(role);
    message.setPhase(phase);
    message.setText(text);

    TimelineItem item;
    item.setTurnId(turnId);
    item.setMessage(std::move(message));
    return item;
}

TimelineItem
activityItem(const QString& turnId,
             const QString& activityId,
             ActivityKind kind,
             ActivityStatus status,
             QList<CommandAction> commandActions = {})
{
    Activity activity;
    activity.setActivityId(activityId);
    activity.setKind(kind);
    activity.setStatus(status);
    activity.setCommandActions(std::move(commandActions));

    TimelineItem item;
    item.setTurnId(turnId);
    item.setActivity(std::move(activity));
    return item;
}

CommandAction
commandAction(CommandActionKind kind)
{
    CommandAction action;
    action.setKind(kind);
    return action;
}

HistoryEvent
conversationEvent(HistoryEventKind kind,
                  QList<TimelineItem> timeline,
                  const QString& title = QStringLiteral("New conversation"),
                  const QStringList& forkableTurnIds = {},
                  PersistedTurnStatus persistedTurnStatus = PersistedTurnStatus::PERSISTED_TURN_STATUS_UNSPECIFIED)
{
    Conversation conversation;
    conversation.setTitle(title);
    conversation.setTimeline(std::move(timeline));
    conversation.setForkableTurnIds(forkableTurnIds);
    if (persistedTurnStatus != PersistedTurnStatus::PERSISTED_TURN_STATUS_UNSPECIFIED) {
        ward::codex::v1::PersistedTurnState turnState;
        turnState.setStatus(persistedTurnStatus);
        conversation.setPersistedTurnState(std::move(turnState));
    }

    HistoryEvent event;
    event.setKind(kind);
    event.setThreadId(QStringLiteral("thread-new"));
    event.setConversation(std::move(conversation));
    return event;
}

HistoryEvent
threadRuntimeEvent(ThreadRuntimeStatus status, const QString& turnId = {})
{
    ward::codex::v1::ThreadRuntimeState state;
    state.setStatus(status);
    if (!turnId.isEmpty())
        state.setTurnId(turnId);

    HistoryEvent event;
    event.setKind(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_RUNTIME_STATE_CHANGED);
    event.setThreadId(QStringLiteral("thread-new"));
    event.setThreadRuntimeState(std::move(state));
    return event;
}

HistoryEvent
threadPageEvent(const QString& name, bool archived = false, bool includeThread = true)
{
    ThreadPage page;
    if (includeThread) {
        ThreadSummary thread;
        thread.setThreadId(QStringLiteral("thread-new"));
        thread.setName(name);
        thread.setPreview(QStringLiteral("New conversation"));
        page.setThreads({ std::move(thread) });
    }

    HistoryEvent event;
    event.setKind(HistoryEventKind::HISTORY_EVENT_KIND_THREADS_UPDATED);
    event.setArchived(archived);
    event.setThreadPage(std::move(page));
    return event;
}

HistoryEvent
turnOutcomeEvent(HistoryEventKind kind, const QString& errorMessage = {})
{
    HistoryEvent event;
    event.setKind(kind);
    event.setThreadId(QStringLiteral("thread-new"));
    if (!errorMessage.isEmpty())
        event.setErrorMessage(errorMessage);
    return event;
}

HistoryEvent
threadListErrorEvent(bool archived, const QString& errorMessage)
{
    HistoryEvent event;
    event.setKind(HistoryEventKind::HISTORY_EVENT_KIND_THREADS_ERROR);
    event.setArchived(archived);
    event.setErrorMessage(errorMessage);
    return event;
}
}

class CodexHistoryControllerTest : public QObject
{
    Q_OBJECT

  private slots:
    void initTestCase();
    void cleanupTestCase();
    void updatesPollingVisibilityState();
    void retranslatesTimelinePresentationWhenRequested();
    void classifiesContextCompactionAsStandaloneActivity();
    void exposesStableActivityPresentationKinds();
    void exposesTurnTimingMetadata();
    void trimsBoundaryLineBreaksFromDisplayedUserMessages();
    void adaptsMessageFormatsAndPreservesMarkupModelsAcrossStreamingUpdates();
    void reconcilesPersistedAndOwnedThreadRunState();
    void exposesInterruptedLatestTurnForContinuation();
    void replacesLiveFirstTurnWithItsPersistedSnapshot();
    void marksOnlyAuthoritativeTurnEndsAsForkBoundaries();
    void confirmsAcceptedTurnGuidance();
    void reportsRejectedTurnGuidanceWithoutConfirmation();
    void validatesConversationRenamesBeforeDispatch();
    void appliesForkedConversationAndSelectsIt();
    void reportsDedicatedThreadForkErrors();
    void validatesConversationForksBeforeDispatch();
    void appliesRenamedConversationTitles();
    void ignoresStaleThreadPagesAndWaitsForLifecycleConfirmation();
    void keepsLifecyclePendingAfterHistoryDecodingFailure();
    void keepsLifecyclePendingAfterMalformedThreadUpdates();
    void rejectsUnscopedThreadRecoveryAndErrorEvents();
    void reportsDedicatedThreadLifecycleErrors();
    void keepsArchivedConversationsReadOnly();
    void appliesModelCatalogUpdatesAndErrors();
    void keepsInferenceSelectionScopedToTheConversationUntilAccepted();
    void describesAndClassifiesLocalAttachmentsByContent();
    void describesClipboardImagesWithSemanticNames();
    void storesClipboardImagesByBlake3Content();
    void rejectsInvalidLocalAttachments();

  private:
    QTranslator englishTranslator_;
};

void
CodexHistoryControllerTest::initTestCase()
{
    QVERIFY(englishTranslator_.load(QStringLiteral(":/i18n/craftward_en.qm")));
    QVERIFY(QCoreApplication::installTranslator(&englishTranslator_));
}

void
CodexHistoryControllerTest::cleanupTestCase()
{
    QVERIFY(QCoreApplication::removeTranslator(&englishTranslator_));
}

void
CodexHistoryControllerTest::updatesPollingVisibilityState()
{
    CodexHistoryController controller(nullptr, nullptr);
    QSignalSpy changedSpy(&controller, &CodexHistoryController::pollingEnabledChanged);

    QVERIFY(controller.pollingEnabled());
    controller.setPollingEnabled(false);
    QVERIFY(!controller.pollingEnabled());
    QCOMPARE(changedSpy.count(), 1);

    controller.setPollingEnabled(false);
    QCOMPARE(changedSpy.count(), 1);

    controller.setPollingEnabled(true);
    QVERIFY(controller.pollingEnabled());
    QCOMPARE(changedSpy.count(), 2);
}

void
CodexHistoryControllerTest::retranslatesTimelinePresentationWhenRequested()
{
    CodexTimelineModel model;
    model.reconcileTimeline({ activityItem(QStringLiteral("turn-1"),
                                           QStringLiteral("activity-1"),
                                           ActivityKind::ACTIVITY_KIND_REASONING,
                                           ActivityStatus::ACTIVITY_STATUS_IN_PROGRESS) },
                            {});
    const QModelIndex row = model.index(0);
    QCOMPARE(model.data(row, CodexTimelineModel::ActivityLabelRole).toString(), QStringLiteral("Reasoning"));
    QCOMPARE(model.data(row, CodexTimelineModel::ActivityItemsRole)
               .toList()
               .constFirst()
               .toMap()
               .value(QStringLiteral("statusLabel"))
               .toString(),
             QStringLiteral("In progress"));
    QSignalSpy dataSpy(&model, &QAbstractItemModel::dataChanged);
    QTranslator simplifiedChinese;
    QVERIFY(simplifiedChinese.load(QStringLiteral(":/i18n/craftward_zh_CN.qm")));

    QVERIFY(QCoreApplication::installTranslator(&simplifiedChinese));
    model.retranslate();

    QVERIFY(dataSpy.count() > 0);
    QCOMPARE(model.data(row, CodexTimelineModel::ActivityLabelRole).toString(), QStringLiteral("推理"));
    QCOMPARE(model.data(row, CodexTimelineModel::ActivityItemsRole)
               .toList()
               .constFirst()
               .toMap()
               .value(QStringLiteral("statusLabel"))
               .toString(),
             QStringLiteral("进行中"));

    QVERIFY(QCoreApplication::removeTranslator(&simplifiedChinese));
}

void
CodexHistoryControllerTest::classifiesContextCompactionAsStandaloneActivity()
{
    CodexTimelineModel model;
    model.reconcileTimeline(
      {
        activityItem(QStringLiteral("turn-1"),
                     QStringLiteral("reasoning"),
                     ActivityKind::ACTIVITY_KIND_REASONING,
                     ActivityStatus::ACTIVITY_STATUS_COMPLETED),
        activityItem(QStringLiteral("turn-1"),
                     QStringLiteral("compaction-1"),
                     ActivityKind::ACTIVITY_KIND_CONTEXT_COMPACTION,
                     ActivityStatus::ACTIVITY_STATUS_COMPLETED),
        activityItem(QStringLiteral("turn-1"),
                     QStringLiteral("compaction-2"),
                     ActivityKind::ACTIVITY_KIND_CONTEXT_COMPACTION,
                     ActivityStatus::ACTIVITY_STATUS_COMPLETED),
      },
      {});

    QCOMPARE(model.rowCount(), 3);
    QVERIFY(!model.data(model.index(0), CodexTimelineModel::StandaloneActivityRole).toBool());
    QVERIFY(model.data(model.index(1), CodexTimelineModel::StandaloneActivityRole).toBool());
    QVERIFY(model.data(model.index(2), CodexTimelineModel::StandaloneActivityRole).toBool());
    QCOMPARE(model.data(model.index(1), CodexTimelineModel::ActivityCountRole).toInt(), 1);
    QCOMPARE(model.data(model.index(2), CodexTimelineModel::ActivityCountRole).toInt(), 1);
}

void
CodexHistoryControllerTest::exposesStableActivityPresentationKinds()
{
    CodexTimelineModel model;
    model.reconcileTimeline(
      {
        activityItem(QStringLiteral("turn-1"),
                     QStringLiteral("read"),
                     ActivityKind::ACTIVITY_KIND_COMMAND_EXECUTION,
                     ActivityStatus::ACTIVITY_STATUS_COMPLETED,
                     { commandAction(CommandActionKind::COMMAND_ACTION_KIND_READ) }),
        activityItem(QStringLiteral("turn-1"),
                     QStringLiteral("list"),
                     ActivityKind::ACTIVITY_KIND_COMMAND_EXECUTION,
                     ActivityStatus::ACTIVITY_STATUS_COMPLETED,
                     { commandAction(CommandActionKind::COMMAND_ACTION_KIND_LIST_FILES) }),
        activityItem(QStringLiteral("turn-1"),
                     QStringLiteral("search"),
                     ActivityKind::ACTIVITY_KIND_COMMAND_EXECUTION,
                     ActivityStatus::ACTIVITY_STATUS_COMPLETED,
                     { commandAction(CommandActionKind::COMMAND_ACTION_KIND_SEARCH) }),
        activityItem(QStringLiteral("turn-1"),
                     QStringLiteral("command"),
                     ActivityKind::ACTIVITY_KIND_COMMAND_EXECUTION,
                     ActivityStatus::ACTIVITY_STATUS_COMPLETED,
                     { commandAction(CommandActionKind::COMMAND_ACTION_KIND_OTHER) }),
        activityItem(QStringLiteral("turn-1"),
                     QStringLiteral("edit"),
                     ActivityKind::ACTIVITY_KIND_FILE_CHANGE,
                     ActivityStatus::ACTIVITY_STATUS_COMPLETED),
        activityItem(QStringLiteral("turn-1"),
                     QStringLiteral("web"),
                     ActivityKind::ACTIVITY_KIND_WEB_SEARCH,
                     ActivityStatus::ACTIVITY_STATUS_COMPLETED),
        activityItem(QStringLiteral("turn-1"),
                     QStringLiteral("compaction"),
                     ActivityKind::ACTIVITY_KIND_CONTEXT_COMPACTION,
                     ActivityStatus::ACTIVITY_STATUS_COMPLETED),
      },
      {});

    const QStringList expectedKinds{
        QStringLiteral("readFiles"),         QStringLiteral("listFiles"),  QStringLiteral("searchFiles"),
        QStringLiteral("runCommands"),       QStringLiteral("fileChange"), QStringLiteral("webSearch"),
        QStringLiteral("contextCompaction"),
    };
    QCOMPARE(model.roleNames().value(CodexTimelineModel::ActivityPresentationKindRole),
             QByteArray("activityPresentationKind"));
    QCOMPARE(model.rowCount(), expectedKinds.size());
    for (int row = 0; row < model.rowCount(); ++row) {
        QCOMPARE(model.data(model.index(row), CodexTimelineModel::ActivityPresentationKindRole).toString(),
                 expectedKinds.at(row));
    }
}

void
CodexHistoryControllerTest::exposesTurnTimingMetadata()
{
    CodexTimelineModel model;
    CodexTurnTiming timing;
    timing.setTurnId(QStringLiteral("turn-1"));
    timing.setStartedAtUnixSeconds(100);
    timing.setCompletedAtUnixSeconds(112);
    timing.setDurationMilliseconds(12'345);
    model.reconcileTimeline(
      {
        messageItem(QStringLiteral("turn-1"),
                    QStringLiteral("agent-1"),
                    MessageRole::MESSAGE_ROLE_AGENT,
                    MessagePhase::MESSAGE_PHASE_FINAL_ANSWER,
                    QStringLiteral("Answer")),
      },
      {},
      { timing });

    QCOMPARE(model.data(model.index(0), CodexTimelineModel::TurnStartedAtUnixSecondsRole).toLongLong(), 100);
    QCOMPARE(model.data(model.index(0), CodexTimelineModel::TurnCompletedAtUnixSecondsRole).toLongLong(), 112);
    QCOMPARE(model.data(model.index(0), CodexTimelineModel::TurnDurationMillisecondsRole).toLongLong(), 12'345);
}

void
CodexHistoryControllerTest::trimsBoundaryLineBreaksFromDisplayedUserMessages()
{
    CodexTimelineModel model;
    model.reconcileTimeline(
      {
        messageItem(QStringLiteral("turn-1"),
                    QStringLiteral("user-1"),
                    MessageRole::MESSAGE_ROLE_USER,
                    MessagePhase::MESSAGE_PHASE_UNSPECIFIED,
                    QStringLiteral("\r\n\nPrompt\r\n\n")),
        messageItem(QStringLiteral("turn-1"),
                    QStringLiteral("agent-1"),
                    MessageRole::MESSAGE_ROLE_AGENT,
                    MessagePhase::MESSAGE_PHASE_FINAL_ANSWER,
                    QStringLiteral("Answer\n")),
      },
      {});

    QCOMPARE(model.data(model.index(0), CodexTimelineModel::TextRole).toString(), QStringLiteral("Prompt"));
    QCOMPARE(model.data(model.index(1), CodexTimelineModel::TextRole).toString(), QStringLiteral("Answer\n"));
    auto* userDocument = qobject_cast<MarkupDocumentModel*>(
      model.data(model.index(0), CodexTimelineModel::MarkupDocumentRole).value<QObject*>());
    QVERIFY(userDocument != nullptr);
    QTRY_COMPARE(userDocument->rowCount(), 1);
    QVERIFY(userDocument->data(userDocument->index(0), MarkupDocumentModel::MarkdownRole).toBool());
    QCOMPARE(userDocument->data(userDocument->index(0), MarkupDocumentModel::PlainTextRole).toString(),
             QStringLiteral("Prompt"));
}

void
CodexHistoryControllerTest::adaptsMessageFormatsAndPreservesMarkupModelsAcrossStreamingUpdates()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    const QString markdownPrefix = QStringLiteral("Before\n\n```sh\necho ready\n```\n\n");
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_UPDATED,
                                                   {
                                                     messageItem(QStringLiteral("turn-1"),
                                                                 QStringLiteral("user-1"),
                                                                 MessageRole::MESSAGE_ROLE_USER,
                                                                 MessagePhase::MESSAGE_PHASE_UNSPECIFIED,
                                                                 QStringLiteral("# Plain user input")),
                                                     messageItem(QStringLiteral("turn-1"),
                                                                 QStringLiteral("agent-1"),
                                                                 MessageRole::MESSAGE_ROLE_AGENT,
                                                                 MessagePhase::MESSAGE_PHASE_FINAL_ANSWER,
                                                                 markdownPrefix + QStringLiteral("After")),
                                                   }),
                                 {});

    CodexTimelineModel* timeline = conversation->timeline();
    QCOMPARE(timeline->entryIdAt(0), QStringLiteral("message:turn-1:user-1"));
    QCOMPARE(timeline->entryIdAt(1), QStringLiteral("message:turn-1:agent-1"));
    QVERIFY(timeline->entryIdAt(-1).isEmpty());
    QVERIFY(timeline->entryIdAt(timeline->rowCount()).isEmpty());
    QCOMPARE(timeline->findChildren<MarkupDocumentModel*>().size(), 0);
    auto* userDocument = qobject_cast<MarkupDocumentModel*>(
      timeline->data(timeline->index(0), CodexTimelineModel::MarkupDocumentRole).value<QObject*>());
    QCOMPARE(timeline->findChildren<MarkupDocumentModel*>().size(), 1);
    auto* agentDocument = qobject_cast<MarkupDocumentModel*>(
      timeline->data(timeline->index(1), CodexTimelineModel::MarkupDocumentRole).value<QObject*>());
    QCOMPARE(timeline->findChildren<MarkupDocumentModel*>().size(), 2);
    QVERIFY(userDocument != nullptr);
    QVERIFY(agentDocument != nullptr);
    QTRY_COMPARE(userDocument->rowCount(), 1);
    QVERIFY(userDocument->data(userDocument->index(0), MarkupDocumentModel::MarkdownRole).toBool());
    QTRY_COMPARE(agentDocument->rowCount(), 3);
    QVERIFY(agentDocument->data(agentDocument->index(1), MarkupDocumentModel::CodeBlockRole).toBool());
    QSignalSpy timelineDataSpy(timeline, &QAbstractItemModel::dataChanged);

    const QString updatedMarkdown = markdownPrefix + QStringLiteral("After more");
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_UPDATED,
                                                   {
                                                     messageItem(QStringLiteral("turn-1"),
                                                                 QStringLiteral("user-1"),
                                                                 MessageRole::MESSAGE_ROLE_USER,
                                                                 MessagePhase::MESSAGE_PHASE_UNSPECIFIED,
                                                                 QStringLiteral("# Plain user input")),
                                                     messageItem(QStringLiteral("turn-1"),
                                                                 QStringLiteral("agent-1"),
                                                                 MessageRole::MESSAGE_ROLE_AGENT,
                                                                 MessagePhase::MESSAGE_PHASE_FINAL_ANSWER,
                                                                 updatedMarkdown),
                                                   }),
                                 {});

    QVERIFY(!timelineDataSpy.isEmpty());
    const QList<int> changedRoles = qvariant_cast<QList<int>>(timelineDataSpy.constLast().at(2));
    QVERIFY(!changedRoles.isEmpty());
    QVERIFY(!changedRoles.contains(CodexTimelineModel::EntryIdRole));
    auto* updatedAgentDocument = qobject_cast<MarkupDocumentModel*>(
      timeline->data(timeline->index(1), CodexTimelineModel::MarkupDocumentRole).value<QObject*>());
    QCOMPARE(updatedAgentDocument, agentDocument);
    QTRY_COMPARE(
      updatedAgentDocument->data(updatedAgentDocument->index(2), MarkupDocumentModel::PlainTextRole).toString(),
      QStringLiteral("After more"));

    QSignalSpy finalizedSpy(agentDocument, &MarkupDocumentModel::documentReconciled);
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_UPDATED,
                                                   {
                                                     messageItem(QStringLiteral("turn-1"),
                                                                 QStringLiteral("user-1"),
                                                                 MessageRole::MESSAGE_ROLE_USER,
                                                                 MessagePhase::MESSAGE_PHASE_UNSPECIFIED,
                                                                 QStringLiteral("# Plain user input")),
                                                     messageItem(QStringLiteral("turn-1"),
                                                                 QStringLiteral("agent-1"),
                                                                 MessageRole::MESSAGE_ROLE_AGENT,
                                                                 MessagePhase::MESSAGE_PHASE_FINAL_ANSWER,
                                                                 updatedMarkdown),
                                                   },
                                                   QStringLiteral("New conversation"),
                                                   { QStringLiteral("turn-1") }),
                                 {});
    QTRY_COMPARE(finalizedSpy.count(), 1);
}

void
CodexHistoryControllerTest::reconcilesPersistedAndOwnedThreadRunState()
{
    using ThreadRunState = CodexConversationController::ThreadRunState;

    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    QSignalSpy runningEvidenceSpy(conversation, &CodexConversationController::runningEvidenceChanged);
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED,
                                                   {},
                                                   QStringLiteral("Running elsewhere"),
                                                   {},
                                                   PersistedTurnStatus::PERSISTED_TURN_STATUS_IN_PROGRESS),
                                 {});

    QCOMPARE(conversation->threadRunState(), ThreadRunState::RunStatePersistedInProgress);
    QVERIFY(conversation->hasRunningEvidence());
    QVERIFY(!conversation->turnRunning());
    QVERIFY(conversation->activeTurnId().isEmpty());
    QCOMPARE(runningEvidenceSpy.count(), 1);

    controller.applyHistoryEvent(threadRuntimeEvent(ThreadRuntimeStatus::THREAD_RUNTIME_STATUS_STARTING), {});

    QCOMPARE(conversation->threadRunState(), ThreadRunState::RunStateUnknown);
    QVERIFY(conversation->turnInFlight());
    QVERIFY(!conversation->turnRunning());
    QVERIFY(conversation->hasRunningEvidence());
    QCOMPARE(runningEvidenceSpy.count(), 1);

    controller.applyHistoryEvent(
      threadRuntimeEvent(ThreadRuntimeStatus::THREAD_RUNTIME_STATUS_ACTIVE, QStringLiteral("turn-owned")), {});

    QCOMPARE(conversation->threadRunState(), ThreadRunState::RunStateObserverOwnedRunning);
    QVERIFY(conversation->hasRunningEvidence());
    QVERIFY(conversation->turnRunning());
    QCOMPARE(conversation->activeTurnId(), QStringLiteral("turn-owned"));
    QCOMPARE(runningEvidenceSpy.count(), 1);

    controller.applyHistoryEvent(threadRuntimeEvent(ThreadRuntimeStatus::THREAD_RUNTIME_STATUS_IDLE), {});

    QCOMPARE(conversation->threadRunState(), ThreadRunState::RunStateNotRunning);
    QVERIFY(!conversation->hasRunningEvidence());
    QVERIFY(!conversation->turnRunning());
    QCOMPARE(runningEvidenceSpy.count(), 2);

    controller.applyHistoryEvent(threadRuntimeEvent(ThreadRuntimeStatus::THREAD_RUNTIME_STATUS_DETACHED), {});

    QCOMPARE(conversation->threadRunState(), ThreadRunState::RunStatePersistedInProgress);
    QVERIFY(conversation->hasRunningEvidence());
    QVERIFY(!conversation->turnRunning());
    QCOMPARE(runningEvidenceSpy.count(), 3);

    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_UPDATED,
                                                   {},
                                                   QStringLiteral("Completed"),
                                                   {},
                                                   PersistedTurnStatus::PERSISTED_TURN_STATUS_COMPLETED),
                                 {});

    QCOMPARE(conversation->threadRunState(), ThreadRunState::RunStateNotRunning);
    QVERIFY(!conversation->hasRunningEvidence());
    QCOMPARE(runningEvidenceSpy.count(), 4);

    conversation->beginLoadingThread(QStringLiteral("thread-next"), QStringLiteral("Next"));

    QCOMPARE(conversation->threadRunState(), ThreadRunState::RunStateUnknown);
    QVERIFY(!conversation->hasRunningEvidence());
    QCOMPARE(runningEvidenceSpy.count(), 4);
}

void
CodexHistoryControllerTest::exposesInterruptedLatestTurnForContinuation()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    QSignalSpy statusSpy(conversation, &CodexConversationController::latestTurnStatusChanged);

    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED,
                                                   {},
                                                   QStringLiteral("Interrupted"),
                                                   {},
                                                   PersistedTurnStatus::PERSISTED_TURN_STATUS_INTERRUPTED),
                                 {});

    QVERIFY(conversation->hasInterruptedLatestTurn());
    QCOMPARE(statusSpy.count(), 1);

    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_UPDATED,
                                                   {},
                                                   QStringLiteral("Continued"),
                                                   {},
                                                   PersistedTurnStatus::PERSISTED_TURN_STATUS_COMPLETED),
                                 {});

    QVERIFY(!conversation->hasInterruptedLatestTurn());
    QCOMPARE(statusSpy.count(), 2);

    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_UPDATED,
                                                   {},
                                                   QStringLiteral("Interrupted again"),
                                                   {},
                                                   PersistedTurnStatus::PERSISTED_TURN_STATUS_INTERRUPTED),
                                 {});
    QVERIFY(conversation->hasInterruptedLatestTurn());
    conversation->beginLoadingThread(QStringLiteral("thread-next"), QStringLiteral("Next"));
    QVERIFY(!conversation->hasInterruptedLatestTurn());
    QCOMPARE(statusSpy.count(), 4);
}

void
CodexHistoryControllerTest::describesAndClassifiesLocalAttachmentsByContent()
{
    QTemporaryDir directory;
    QVERIFY(directory.isValid());
    const auto writeFile = [&directory](const QString& name, const QByteArray& content) {
        QFile file(directory.filePath(name));
        return file.open(QIODevice::WriteOnly) && file.write(content) == content.size();
    };
    const QString imagePath = directory.filePath(QStringLiteral("visual.dat"));
    const QString audioPath = directory.filePath(QStringLiteral("recording.dat"));
    const QString documentPath = directory.filePath(QStringLiteral("requirements.dat"));
    QVERIFY(writeFile(QStringLiteral("visual.dat"),
                      QByteArray::fromHex("89504e470d0a1a0a0000000d4948445200000001000000010806000000")));
    QVERIFY(writeFile(
      QStringLiteral("recording.dat"),
      QByteArray::fromHex("524946462400000057415645666d74201000000001000100401f0000803e0000020010006461746100000000")));
    QVERIFY(writeFile(QStringLiteral("requirements.dat"), QByteArrayLiteral("%PDF-1.7\n")));

    QString errorMessage;
    const auto described = CodexAttachmentInput::describe({ QUrl::fromLocalFile(imagePath),
                                                            QUrl::fromLocalFile(audioPath),
                                                            QUrl::fromLocalFile(documentPath),
                                                            QUrl::fromLocalFile(imagePath) },
                                                          false,
                                                          &errorMessage);
    QVERIFY(described.has_value());
    QVERIFY(errorMessage.isEmpty());
    QCOMPARE(described->size(), 3);
    QCOMPARE((*described)[0].kind, CodexAttachmentKind::LocalImage);
    QCOMPARE((*described)[0].mimeType, QStringLiteral("image/png"));
    QCOMPARE((*described)[0].name, QStringLiteral("visual.dat"));
    QCOMPARE((*described)[0].url, QUrl::fromLocalFile(QFileInfo(imagePath).canonicalFilePath()));
    QVERIFY(!(*described)[0].managed);
    QCOMPARE((*described)[0].nameKind, CodexAttachmentNameKind::FileName);
    QCOMPARE(CodexAttachmentInput::nameKindName(CodexAttachmentNameKind::FileName), QStringLiteral("fileName"));
    QCOMPARE(CodexAttachmentInput::nameKindName(CodexAttachmentNameKind::PastedImage), QStringLiteral("pastedImage"));
    QCOMPARE((*described)[1].kind, CodexAttachmentKind::LocalAudio);
    QVERIFY((*described)[1].mimeType.startsWith(QStringLiteral("audio/")));
    QCOMPARE((*described)[2].kind, CodexAttachmentKind::Mention);
    QCOMPARE((*described)[2].mimeType, QStringLiteral("application/pdf"));

    const auto prepared = CodexAttachmentInput::prepare({ QUrl::fromLocalFile(imagePath),
                                                          QUrl::fromLocalFile(audioPath),
                                                          QUrl::fromLocalFile(documentPath),
                                                          QUrl::fromLocalFile(imagePath) },
                                                        &errorMessage);

    QVERIFY(prepared.has_value());
    QVERIFY(errorMessage.isEmpty());
    QCOMPARE(prepared->size(), 3);
    QCOMPARE((*prepared)[0].kind, CodexAttachmentKind::LocalImage);
    QCOMPARE((*prepared)[0].name, QByteArrayLiteral("visual.dat"));
    QCOMPARE((*prepared)[1].kind, CodexAttachmentKind::LocalAudio);
    QCOMPARE((*prepared)[1].name, QByteArrayLiteral("recording.dat"));
    QCOMPARE((*prepared)[2].kind, CodexAttachmentKind::Mention);
    QCOMPARE((*prepared)[2].name, QByteArrayLiteral("requirements.dat"));
    QCOMPARE((*prepared)[2].path, QFileInfo(documentPath).canonicalFilePath().toUtf8());
}

void
CodexHistoryControllerTest::describesClipboardImagesWithSemanticNames()
{
    QTemporaryDir dataDirectory;
    QVERIFY(dataDirectory.isValid());
    QImage image(2, 2, QImage::Format_RGBA8888);
    image.fill(QColorConstants::Red);
    QMimeData mimeData;
    mimeData.setImageData(image);
    QString errorMessage;

    const QList<CodexAttachmentDescriptor> described =
      CodexAttachmentInput::fromMimeData(mimeData, dataDirectory.path(), &errorMessage);

    QVERIFY2(errorMessage.isEmpty(), qPrintable(errorMessage));
    QCOMPARE(described.size(), 1);
    QVERIFY(described.constFirst().managed);
    QCOMPARE(described.constFirst().kind, CodexAttachmentKind::LocalImage);
    QCOMPARE(described.constFirst().nameKind, CodexAttachmentNameKind::PastedImage);
}

void
CodexHistoryControllerTest::storesClipboardImagesByBlake3Content()
{
    QTemporaryDir dataDirectory;
    QVERIFY(dataDirectory.isValid());
    QImage firstImage(2, 2, QImage::Format_RGBA8888);
    firstImage.fill(QColorConstants::Red);
    QImage secondImage(2, 2, QImage::Format_RGBA8888);
    secondImage.fill(QColorConstants::Blue);

    QString errorMessage;
    const QUrl firstUrl = CodexAttachmentInput::storeClipboardImage(firstImage, dataDirectory.path(), &errorMessage);
    QVERIFY2(!firstUrl.isEmpty(), qPrintable(errorMessage));
    QVERIFY(errorMessage.isEmpty());
    const QUrl duplicateUrl =
      CodexAttachmentInput::storeClipboardImage(firstImage, dataDirectory.path(), &errorMessage);
    QCOMPARE(duplicateUrl, firstUrl);
    QVERIFY(errorMessage.isEmpty());
    const QUrl secondUrl = CodexAttachmentInput::storeClipboardImage(secondImage, dataDirectory.path(), &errorMessage);
    QVERIFY2(!secondUrl.isEmpty(), qPrintable(errorMessage));
    QVERIFY(secondUrl != firstUrl);

    const QFileInfo firstFile(firstUrl.toLocalFile());
    QVERIFY(firstFile.isFile());
    QVERIFY(QRegularExpression(QStringLiteral("^[0-9a-f]{64}\\.png$")).match(firstFile.fileName()).hasMatch());
    QDir contentDirectory = firstFile.dir();
    QCOMPARE(contentDirectory.dirName(), firstFile.completeBaseName().left(2));
    QVERIFY(contentDirectory.cdUp());
    QCOMPARE(contentDirectory.dirName(), QStringLiteral("attachments"));
    QVERIFY(contentDirectory.cdUp());
    QCOMPARE(contentDirectory.absolutePath(), QDir(dataDirectory.path()).absolutePath());

    qsizetype storedFileCount = 0;
    QDirIterator files(dataDirectory.path(), { QStringLiteral("*.png") }, QDir::Files, QDirIterator::Subdirectories);
    while (files.hasNext()) {
        files.next();
        ++storedFileCount;
    }
    QCOMPARE(storedFileCount, 2);
}

void
CodexHistoryControllerTest::rejectsInvalidLocalAttachments()
{
    QString errorMessage;
    auto prepared =
      CodexAttachmentInput::prepare({ QUrl(QStringLiteral("https://example.com/file.pdf")) }, &errorMessage);
    QVERIFY(!prepared.has_value());
    QVERIFY(!errorMessage.isEmpty());

    QTemporaryDir directory;
    QVERIFY(directory.isValid());
    prepared = CodexAttachmentInput::prepare({ QUrl::fromLocalFile(directory.path()) }, &errorMessage);
    QVERIFY(!prepared.has_value());
    QVERIFY(!errorMessage.isEmpty());

    prepared = CodexAttachmentInput::prepare({ QUrl::fromLocalFile(directory.filePath(QStringLiteral("missing.txt"))) },
                                             &errorMessage);
    QVERIFY(!prepared.has_value());
    QVERIFY(!errorMessage.isEmpty());
}

void
CodexHistoryControllerTest::appliesModelCatalogUpdatesAndErrors()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();

    HistoryEvent error;
    error.setKind(HistoryEventKind::HISTORY_EVENT_KIND_MODEL_CATALOG_ERROR);
    error.setErrorMessage(QStringLiteral("Catalog unavailable"));
    controller.applyHistoryEvent(std::move(error), {});

    QVERIFY(!conversation->loadingModelCatalog());
    QCOMPARE(conversation->modelCatalogErrorMessage(), QStringLiteral("Catalog unavailable"));

    controller.applyHistoryEvent(modelCatalogEvent(), {});

    QVERIFY(!conversation->loadingModelCatalog());
    QVERIFY(conversation->modelCatalogErrorMessage().isEmpty());
    CodexModelCatalogModel* catalog = conversation->modelCatalog();
    QCOMPARE(catalog->rowCount(), 2);
    const QModelIndex index = catalog->index(0);
    QCOMPARE(catalog->data(index, CodexModelCatalogModel::ModelIdRole).toString(), QStringLiteral("balanced"));
    QCOMPARE(catalog->data(index, CodexModelCatalogModel::ModelRole).toString(), QStringLiteral("gpt-balanced"));
    QCOMPARE(catalog->data(index, CodexModelCatalogModel::DisplayNameRole).toString(), QStringLiteral("Balanced"));
    QCOMPARE(catalog->data(index, CodexModelCatalogModel::DescriptionRole).toString(),
             QStringLiteral("Balances capability and speed."));
    QVERIFY(catalog->data(index, CodexModelCatalogModel::DefaultRole).toBool());
    QCOMPARE(catalog->data(index, CodexModelCatalogModel::DefaultReasoningEffortRole).toString(),
             QStringLiteral("medium"));
    const QVariantList efforts = catalog->data(index, CodexModelCatalogModel::SupportedReasoningEffortsRole).toList();
    QCOMPARE(efforts.size(), 3);
    QCOMPARE(efforts[0].toMap().value(QStringLiteral("reasoningEffort")).toString(), QStringLiteral("low"));
    QCOMPARE(efforts[0].toMap().value(QStringLiteral("description")).toString(), QStringLiteral("Faster responses"));
}

void
CodexHistoryControllerTest::keepsInferenceSelectionScopedToTheConversationUntilAccepted()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    controller.applyHistoryEvent(modelCatalogEvent(), {});
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    QSignalSpy modelSpy(conversation, &CodexConversationController::modelChanged);
    QSignalSpy effortSpy(conversation, &CodexConversationController::reasoningEffortChanged);

    controller.applyHistoryEvent(threadModelEvent(QStringLiteral("gpt-balanced")), {});
    QCOMPARE(conversation->model(), QStringLiteral("gpt-balanced"));
    QCOMPARE(conversation->reasoningEffort(), QStringLiteral("medium"));
    QCOMPARE(conversation->reasoningEfforts().size(), 3);
    QCOMPARE(modelSpy.count(), 1);
    QCOMPARE(effortSpy.count(), 1);

    QVERIFY(conversation->selectReasoningEffort(QStringLiteral("high")));
    QCOMPARE(conversation->reasoningEffort(), QStringLiteral("high"));
    QVERIFY(conversation->selectModel(QStringLiteral("gpt-fast")));
    QCOMPARE(conversation->model(), QStringLiteral("gpt-fast"));
    QCOMPARE(conversation->reasoningEffort(), QStringLiteral("low"));
    QCOMPARE(conversation->reasoningEfforts().size(), 2);
    QCOMPARE(conversation->modelOverride(), QStringLiteral("gpt-fast"));
    QCOMPARE(conversation->reasoningEffortOverride(), QStringLiteral("low"));
    QCOMPARE(modelSpy.count(), 2);
    QVERIFY(!conversation->selectModel(QStringLiteral("gpt-missing")));
    QVERIFY(!conversation->selectReasoningEffort(QStringLiteral("high")));

    controller.applyHistoryEvent(
      threadModelEvent(QStringLiteral("gpt-balanced"), QStringLiteral("thread-new"), QStringLiteral("medium")), {});
    QCOMPARE(conversation->model(), QStringLiteral("gpt-fast"));
    QCOMPARE(conversation->reasoningEffort(), QStringLiteral("low"));
    QCOMPARE(modelSpy.count(), 2);

    controller.applyHistoryEvent(
      threadModelEvent(QStringLiteral("gpt-fast"), QStringLiteral("thread-new"), QStringLiteral("low")), {});
    QCOMPARE(conversation->model(), QStringLiteral("gpt-fast"));
    QCOMPARE(conversation->reasoningEffort(), QStringLiteral("low"));
    QCOMPARE(conversation->modelOverride(), QString());
    QCOMPARE(conversation->reasoningEffortOverride(), QString());
    QCOMPARE(modelSpy.count(), 2);

    controller.applyHistoryEvent(
      threadModelEvent(QStringLiteral("gpt-balanced"), QStringLiteral("thread-other"), QStringLiteral("low")), {});
    QCOMPARE(conversation->model(), QStringLiteral("gpt-fast"));
    controller.selectThread(QStringLiteral("thread-other"), QStringLiteral("Other conversation"));
    QCOMPARE(conversation->model(), QStringLiteral("gpt-balanced"));
    QCOMPARE(conversation->reasoningEffort(), QStringLiteral("low"));
    QVERIFY(conversation->selectReasoningEffort(QStringLiteral("medium")));
    QCOMPARE(conversation->reasoningEffort(), QStringLiteral("medium"));
    QCOMPARE(conversation->reasoningEffortOverride(), QStringLiteral("medium"));
    controller.applyHistoryEvent(
      threadModelEvent(QStringLiteral("gpt-balanced"), QStringLiteral("thread-other"), QStringLiteral("low")), {});
    QCOMPARE(conversation->reasoningEffort(), QStringLiteral("medium"));
    controller.applyHistoryEvent(
      threadModelEvent(QStringLiteral("gpt-balanced"), QStringLiteral("thread-other"), QStringLiteral("medium")), {});
    QCOMPARE(conversation->reasoningEffortOverride(), QString());

    controller.selectThread(QStringLiteral("thread-new"), QStringLiteral("New conversation"));
    QCOMPARE(conversation->model(), QStringLiteral("gpt-fast"));
    QCOMPARE(conversation->reasoningEffort(), QStringLiteral("low"));

    controller.clearSelection();
    QVERIFY(conversation->model().isEmpty());
    QVERIFY(conversation->reasoningEffort().isEmpty());
    QVERIFY(modelSpy.count() >= 4);
    QVERIFY(effortSpy.count() >= 4);
}

void
CodexHistoryControllerTest::replacesLiveFirstTurnWithItsPersistedSnapshot()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_UPDATED,
                                                   {
                                                     messageItem(QStringLiteral("live-turn-1"),
                                                                 QStringLiteral("live-user-1"),
                                                                 MessageRole::MESSAGE_ROLE_USER,
                                                                 MessagePhase::MESSAGE_PHASE_UNSPECIFIED,
                                                                 QStringLiteral("Hello")),
                                                     messageItem(QStringLiteral("live-turn-1"),
                                                                 QStringLiteral("live-agent-1"),
                                                                 MessageRole::MESSAGE_ROLE_AGENT,
                                                                 MessagePhase::MESSAGE_PHASE_FINAL_ANSWER,
                                                                 QStringLiteral("Done.")),
                                                   }),
                                 {});

    CodexTimelineModel* timeline = conversation->timeline();
    QCOMPARE(timeline->rowCount(), 2);
    QCOMPARE(timeline->data(timeline->index(0), CodexTimelineModel::EntryIdRole).toString(),
             QStringLiteral("message:live-turn-1:live-user-1"));
    QCOMPARE(timeline->data(timeline->index(1), CodexTimelineModel::EntryIdRole).toString(),
             QStringLiteral("message:live-turn-1:live-agent-1"));
    QVERIFY(!timeline->data(timeline->index(1), CodexTimelineModel::ForkBoundaryRole).toBool());

    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_UPDATED,
                                                   {
                                                     messageItem(QStringLiteral("persisted-turn-1"),
                                                                 QStringLiteral("persisted-user-1"),
                                                                 MessageRole::MESSAGE_ROLE_USER,
                                                                 MessagePhase::MESSAGE_PHASE_UNSPECIFIED,
                                                                 QStringLiteral("Hello")),
                                                     messageItem(QStringLiteral("persisted-turn-1"),
                                                                 QStringLiteral("persisted-agent-1"),
                                                                 MessageRole::MESSAGE_ROLE_AGENT,
                                                                 MessagePhase::MESSAGE_PHASE_FINAL_ANSWER,
                                                                 QStringLiteral("Done.")),
                                                   },
                                                   QStringLiteral("New conversation"),
                                                   { QStringLiteral("persisted-turn-1") }),
                                 {});

    QCOMPARE(timeline->rowCount(), 2);
    QCOMPARE(timeline->data(timeline->index(0), CodexTimelineModel::EntryIdRole).toString(),
             QStringLiteral("message:persisted-turn-1:persisted-user-1"));
    QCOMPARE(timeline->data(timeline->index(0), CodexTimelineModel::TextRole).toString(), QStringLiteral("Hello"));
    QCOMPARE(timeline->data(timeline->index(1), CodexTimelineModel::EntryIdRole).toString(),
             QStringLiteral("message:persisted-turn-1:persisted-agent-1"));
    QCOMPARE(timeline->data(timeline->index(1), CodexTimelineModel::TextRole).toString(), QStringLiteral("Done."));
    QVERIFY(timeline->data(timeline->index(1), CodexTimelineModel::ForkBoundaryRole).toBool());
}

void
CodexHistoryControllerTest::marksOnlyAuthoritativeTurnEndsAsForkBoundaries()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED,
                                                   {
                                                     messageItem(QStringLiteral("turn-1"),
                                                                 QStringLiteral("user-1"),
                                                                 MessageRole::MESSAGE_ROLE_USER,
                                                                 MessagePhase::MESSAGE_PHASE_UNSPECIFIED,
                                                                 QStringLiteral("First prompt")),
                                                     messageItem(QStringLiteral("turn-1"),
                                                                 QStringLiteral("agent-1"),
                                                                 MessageRole::MESSAGE_ROLE_AGENT,
                                                                 MessagePhase::MESSAGE_PHASE_FINAL_ANSWER,
                                                                 QStringLiteral("First answer")),
                                                     messageItem(QStringLiteral("turn-2"),
                                                                 QStringLiteral("user-2"),
                                                                 MessageRole::MESSAGE_ROLE_USER,
                                                                 MessagePhase::MESSAGE_PHASE_UNSPECIFIED,
                                                                 QStringLiteral("Second prompt")),
                                                     messageItem(QStringLiteral("turn-2"),
                                                                 QStringLiteral("agent-2"),
                                                                 MessageRole::MESSAGE_ROLE_AGENT,
                                                                 MessagePhase::MESSAGE_PHASE_FINAL_ANSWER,
                                                                 QStringLiteral("Second answer")),
                                                   },
                                                   QStringLiteral("New conversation"),
                                                   { QStringLiteral("turn-1") }),
                                 {});

    CodexTimelineModel* timeline = conversation->timeline();
    QCOMPARE(timeline->rowCount(), 4);
    QVERIFY(!timeline->data(timeline->index(0), CodexTimelineModel::ForkBoundaryRole).toBool());
    QVERIFY(timeline->data(timeline->index(1), CodexTimelineModel::ForkBoundaryRole).toBool());
    QVERIFY(!timeline->data(timeline->index(2), CodexTimelineModel::ForkBoundaryRole).toBool());
    QVERIFY(!timeline->data(timeline->index(3), CodexTimelineModel::ForkBoundaryRole).toBool());
    QVERIFY(timeline->data(timeline->index(0), CodexTimelineModel::TurnForkableRole).toBool());
    QVERIFY(timeline->data(timeline->index(1), CodexTimelineModel::TurnForkableRole).toBool());
    QVERIFY(!timeline->data(timeline->index(2), CodexTimelineModel::TurnForkableRole).toBool());
    QVERIFY(!timeline->data(timeline->index(3), CodexTimelineModel::TurnForkableRole).toBool());
    QVERIFY(!timeline->data(timeline->index(0), CodexTimelineModel::LatestTurnRole).toBool());
    QVERIFY(!timeline->data(timeline->index(1), CodexTimelineModel::LatestTurnRole).toBool());
    QVERIFY(timeline->data(timeline->index(2), CodexTimelineModel::LatestTurnRole).toBool());
    QVERIFY(timeline->data(timeline->index(3), CodexTimelineModel::LatestTurnRole).toBool());
    QCOMPARE(timeline->roleNames().value(CodexTimelineModel::TurnForkableRole), QByteArray("turnForkable"));
    QCOMPARE(timeline->roleNames().value(CodexTimelineModel::LatestTurnRole), QByteArray("latestTurn"));
}

void
CodexHistoryControllerTest::confirmsAcceptedTurnGuidance()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    QSignalSpy steeredSpy(conversation, &CodexConversationController::turnSteered);
    conversation->setSteeringTurn(true);

    controller.applyHistoryEvent(turnOutcomeEvent(HistoryEventKind::HISTORY_EVENT_KIND_TURN_STEERED), {});

    QVERIFY(!conversation->steeringTurn());
    QCOMPARE(steeredSpy.count(), 1);
}

void
CodexHistoryControllerTest::reportsRejectedTurnGuidanceWithoutConfirmation()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    QSignalSpy steeredSpy(conversation, &CodexConversationController::turnSteered);
    conversation->setSteeringTurn(true);

    controller.applyHistoryEvent(
      turnOutcomeEvent(HistoryEventKind::HISTORY_EVENT_KIND_TURN_STEER_ERROR, QStringLiteral("The turn finished.")),
      {});

    QVERIFY(!conversation->steeringTurn());
    QCOMPARE(steeredSpy.count(), 0);
    QCOMPARE(controller.errorMessage(), QStringLiteral("The turn finished."));
}

void
CodexHistoryControllerTest::validatesConversationRenamesBeforeDispatch()
{
    CodexHistoryController controller(nullptr, nullptr);
    controller.clearError();

    QVERIFY(!controller.renameSelectedThread(QStringLiteral("Focused work")));
    QVERIFY(controller.errorMessage().isEmpty());

    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    QVERIFY(!controller.renameSelectedThread(QStringLiteral("   ")));
    QVERIFY(!controller.renameSelectedThread(QStringLiteral("  New conversation  ")));
    QVERIFY(controller.errorMessage().isEmpty());

    QVERIFY(!controller.renameSelectedThread(QStringLiteral("Focused work")));
    QCOMPARE(controller.errorMessage(), QStringLiteral("The Codex history observer is unavailable."));
}

void
CodexHistoryControllerTest::appliesForkedConversationAndSelectsIt()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    controller.setForkingThread(true, QStringLiteral("thread-new"));
    controller.loadingThreads_ = true;
    QSignalSpy selectionSpy(conversation, &CodexConversationController::selectionChanged);

    HistoryEvent forked = conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_FORKED,
                                            { messageItem(QStringLiteral("turn-1"),
                                                          QStringLiteral("agent-1"),
                                                          MessageRole::MESSAGE_ROLE_AGENT,
                                                          MessagePhase::MESSAGE_PHASE_FINAL_ANSWER,
                                                          QStringLiteral("Copied answer")) },
                                            QStringLiteral("Forked conversation"));
    forked.setThreadId(QStringLiteral("thread-fork-1"));
    controller.applyHistoryEvent(std::move(forked), {});

    QCOMPARE(conversation->threadId(), QStringLiteral("thread-fork-1"));
    QCOMPARE(conversation->title(), QStringLiteral("Forked conversation"));
    CodexTimelineModel* timeline = conversation->timeline();
    QCOMPARE(timeline->rowCount(), 1);
    QCOMPARE(timeline->data(timeline->index(0), CodexTimelineModel::TextRole).toString(),
             QStringLiteral("Copied answer"));
    QVERIFY(!controller.forkingThread());
    QVERIFY(controller.loadingThreads());
    QCOMPARE(conversation->turnState(), CodexConversationController::TurnState::Detached);
    QCOMPARE(conversation->writeAvailability(), CodexConversationController::WriteAvailability::NotRequested);
    QCOMPARE(selectionSpy.count(), 1);

    ward::codex::v1::ThreadWriteState writeState;
    writeState.setStatus(ward::codex::v1::ThreadWriteStatusGadget::ThreadWriteStatus::THREAD_WRITE_STATUS_WRITABLE);
    HistoryEvent writable;
    writable.setKind(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_WRITE_STATE_CHANGED);
    writable.setThreadId(QStringLiteral("thread-fork-1"));
    writable.setThreadWriteState(std::move(writeState));
    controller.applyHistoryEvent(std::move(writable), {});

    QCOMPARE(conversation->writeAvailability(), CodexConversationController::WriteAvailability::Writable);
}

void
CodexHistoryControllerTest::reportsDedicatedThreadForkErrors()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    controller.setForkingThread(true, QStringLiteral("thread-new"));
    controller.loadingThreads_ = true;
    controller.clearError();

    HistoryEvent staleError =
      turnOutcomeEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_FORK_ERROR, QStringLiteral("Stale failure"));
    staleError.setThreadId(QStringLiteral("thread-other"));
    controller.applyHistoryEvent(std::move(staleError), {});
    QVERIFY(controller.forkingThread());
    QVERIFY(controller.errorMessage().isEmpty());

    controller.applyHistoryEvent(
      turnOutcomeEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_FORK_ERROR, QStringLiteral("Fork failed")), {});

    QCOMPARE(conversation->threadId(), QStringLiteral("thread-new"));
    QVERIFY(!controller.forkingThread());
    QVERIFY(controller.loadingThreads());
    QCOMPARE(controller.errorMessage(), QStringLiteral("Fork failed"));

    controller.applyHistoryEvent(threadPageEvent(QStringLiteral("New conversation")), {});

    QVERIFY(!controller.loadingThreads());
    QCOMPARE(conversation->threadId(), QStringLiteral("thread-new"));
    QCOMPARE(controller.errorMessage(), QStringLiteral("Fork failed"));
}

void
CodexHistoryControllerTest::validatesConversationForksBeforeDispatch()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    controller.clearError();

    QVERIFY(!controller.forkSelectedThread(QStringLiteral("turn-1")));
    QVERIFY(controller.errorMessage().isEmpty());

    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    conversation->loading_ = true;
    QVERIFY(!controller.forkSelectedThread(QStringLiteral("turn-1")));
    conversation->loading_ = false;
    conversation->setWriteAvailability(CodexConversationController::WriteAvailability::Checking);
    QVERIFY(!controller.forkSelectedThread(QStringLiteral("turn-1")));
    QVERIFY(controller.errorMessage().isEmpty());
    conversation->setWriteAvailability(CodexConversationController::WriteAvailability::NotRequested);
    conversation->setTurnState(CodexConversationController::TurnState::SystemError);
    QVERIFY(!controller.forkSelectedThread(QStringLiteral("turn-1")));
    QVERIFY(controller.errorMessage().isEmpty());
    conversation->setTurnState(CodexConversationController::TurnState::Detached);
    QVERIFY(!controller.forkSelectedThread({}));
    QVERIFY(controller.errorMessage().isEmpty());
    QVERIFY(!controller.forkSelectedThread(QStringLiteral("turn-1")));
    QCOMPARE(controller.errorMessage(), QStringLiteral("The Codex history observer is unavailable."));

    controller.clearError();
    controller.showingArchived_ = true;
    QVERIFY(!controller.forkSelectedThread(QStringLiteral("turn-1")));
    QVERIFY(controller.errorMessage().isEmpty());
}

void
CodexHistoryControllerTest::appliesRenamedConversationTitles()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    controller.applyHistoryEvent(threadPageEvent(QStringLiteral("New conversation")), {});
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    QSignalSpy selectionSpy(conversation, &CodexConversationController::selectionChanged);

    controller.applyHistoryEvent(threadPageEvent(QStringLiteral("Focused work")), {});
    controller.applyHistoryEvent(
      conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_UPDATED, {}, QStringLiteral("Focused work")),
      {});

    QCOMPARE(controller.threads()->rowCount(), 1);
    QCOMPARE(controller.threads()->data(controller.threads()->index(0), CodexThreadModel::TitleRole).toString(),
             QStringLiteral("Focused work"));
    QCOMPARE(conversation->title(), QStringLiteral("Focused work"));
    QCOMPARE(selectionSpy.count(), 1);
}

void
CodexHistoryControllerTest::ignoresStaleThreadPagesAndWaitsForLifecycleConfirmation()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    controller.applyHistoryEvent(threadPageEvent(QStringLiteral("New conversation")), {});
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    controller.pendingLifecycleThreadId_ = QStringLiteral("thread-new");
    controller.changingThreadLifecycle_ = true;
    controller.loadingThreads_ = true;

    controller.applyHistoryEvent(threadPageEvent(QStringLiteral("New conversation"), true, false), {});
    QCOMPARE(controller.threads()->rowCount(), 1);
    QCOMPARE(conversation->threadId(), QStringLiteral("thread-new"));
    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());

    controller.applyHistoryEvent(threadListErrorEvent(true, QStringLiteral("Stale archived error")), {});
    QCOMPARE(conversation->threadId(), QStringLiteral("thread-new"));
    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());
    QVERIFY(controller.errorMessage().isEmpty());

    controller.applyHistoryEvent(turnOutcomeEvent(HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_ERROR,
                                                  QStringLiteral("Temporary conversation error")),
                                 {});
    QCOMPARE(conversation->threadId(), QStringLiteral("thread-new"));
    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());
    QCOMPARE(controller.errorMessage(), QStringLiteral("Temporary conversation error"));

    controller.clearError();
    controller.applyHistoryEvent(threadListErrorEvent(false, QStringLiteral("Temporary active error")), {});
    QCOMPARE(conversation->threadId(), QStringLiteral("thread-new"));
    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());
    QCOMPARE(controller.errorMessage(), QStringLiteral("Temporary active error"));

    controller.applyHistoryEvent(threadPageEvent(QStringLiteral("New conversation")), {});
    QCOMPARE(controller.threads()->rowCount(), 1);
    QCOMPARE(conversation->threadId(), QStringLiteral("thread-new"));
    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());

    controller.applyHistoryEvent(threadPageEvent(QStringLiteral("New conversation"), false, false), {});
    QCOMPARE(controller.threads()->rowCount(), 0);
    QVERIFY(conversation->threadId().isEmpty());
    QCOMPARE(conversation->timeline()->rowCount(), 0);
    QVERIFY(!controller.changingThreadLifecycle());
    QVERIFY(!controller.loadingThreads());
}

void
CodexHistoryControllerTest::keepsLifecyclePendingAfterHistoryDecodingFailure()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    controller.pendingLifecycleThreadId_ = QStringLiteral("thread-new");
    controller.changingThreadLifecycle_ = true;
    controller.loadingThreads_ = true;

    controller.applyHistoryEvent({}, QStringLiteral("Malformed history event"));

    QCOMPARE(conversation->threadId(), QStringLiteral("thread-new"));
    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());
    QCOMPARE(controller.errorMessage(), QStringLiteral("Malformed history event"));
}

void
CodexHistoryControllerTest::keepsLifecyclePendingAfterMalformedThreadUpdates()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    controller.pendingLifecycleThreadId_ = QStringLiteral("thread-new");
    controller.changingThreadLifecycle_ = true;
    controller.loadingThreads_ = true;

    HistoryEvent missingScope;
    missingScope.setKind(HistoryEventKind::HISTORY_EVENT_KIND_THREADS_UPDATED);
    missingScope.setThreadPage({});
    controller.applyHistoryEvent(std::move(missingScope), {});

    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());

    HistoryEvent missingPage;
    missingPage.setKind(HistoryEventKind::HISTORY_EVENT_KIND_THREADS_UPDATED);
    missingPage.setArchived(false);
    controller.applyHistoryEvent(std::move(missingPage), {});

    QCOMPARE(conversation->threadId(), QStringLiteral("thread-new"));
    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());
}

void
CodexHistoryControllerTest::rejectsUnscopedThreadRecoveryAndErrorEvents()
{
    CodexHistoryController controller(nullptr, nullptr);
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    controller.pendingLifecycleThreadId_ = QStringLiteral("thread-new");
    controller.changingThreadLifecycle_ = true;
    controller.loadingThreads_ = true;
    controller.clearError();
    const QString missingScopeError =
      QStringLiteral("Ward Core returned a thread-list event without its history scope.");

    HistoryEvent recovery;
    recovery.setKind(HistoryEventKind::HISTORY_EVENT_KIND_THREADS_RECOVERED);
    controller.applyHistoryEvent(std::move(recovery), {});

    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());
    QCOMPARE(controller.errorMessage(), missingScopeError);

    controller.clearError();
    HistoryEvent error;
    error.setKind(HistoryEventKind::HISTORY_EVENT_KIND_THREADS_ERROR);
    error.setErrorMessage(QStringLiteral("Unscoped list error"));
    controller.applyHistoryEvent(std::move(error), {});

    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());
    QCOMPARE(controller.errorMessage(), missingScopeError);
}

void
CodexHistoryControllerTest::reportsDedicatedThreadLifecycleErrors()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    controller.pendingLifecycleThreadId_ = QStringLiteral("thread-new");
    controller.changingThreadLifecycle_ = true;
    controller.loadingThreads_ = true;

    controller.applyHistoryEvent(
      turnOutcomeEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_LIFECYCLE_ERROR, QStringLiteral("Archive failed")),
      {});

    QCOMPARE(conversation->threadId(), QStringLiteral("thread-new"));
    QVERIFY(!controller.changingThreadLifecycle());
    QVERIFY(!controller.loadingThreads());
    QCOMPARE(controller.errorMessage(), QStringLiteral("Archive failed"));
}

void
CodexHistoryControllerTest::keepsArchivedConversationsReadOnly()
{
    CodexHistoryController controller(nullptr, nullptr);
    CodexConversationController* conversation = controller.conversation();
    controller.showingArchived_ = true;
    controller.applyHistoryEvent(threadPageEvent(QStringLiteral("Archived conversation"), true), {});
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED,
                                                   {},
                                                   QStringLiteral("Archived conversation")),
                                 {});
    controller.clearError();

    QVERIFY(controller.showingArchived());
    QVERIFY(!controller.renameSelectedThread(QStringLiteral("Renamed")));
    QVERIFY(!controller.startThread(QUrl::fromLocalFile(QStringLiteral("/workspace"))));
    conversation->acquireWriteAccess();
    QCOMPARE(conversation->writeAvailability(), CodexConversationController::WriteAvailability::NotRequested);
    conversation->setWriteAvailability(CodexConversationController::WriteAvailability::Writable);
    QVERIFY(!conversation->startTurn(QStringLiteral("Continue"), {}));
    QVERIFY(controller.errorMessage().isEmpty());
}

QTEST_GUILESS_MAIN(CodexHistoryControllerTest)

#include "codexhistorycontrollertest.moc"
