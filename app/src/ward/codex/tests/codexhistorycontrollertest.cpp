// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexhistorycontroller.h"

#include <QtTest/QSignalSpy>
#include <QtTest/QTest>

namespace {
using Conversation = ward::codex::v1::Conversation;
using HistoryEvent = ward::codex::v1::HistoryEvent;
using HistoryEventKind = ward::codex::v1::HistoryEventKindGadget::HistoryEventKind;
using Message = ward::codex::v1::Message;
using MessagePhase = ward::codex::v1::MessagePhaseGadget::MessagePhase;
using MessageRole = ward::codex::v1::MessageRoleGadget::MessageRole;
using ThreadPage = ward::codex::v1::ThreadPage;
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

HistoryEvent
conversationEvent(HistoryEventKind kind,
                  QList<TimelineItem> timeline,
                  const QString& title = QStringLiteral("New conversation"),
                  const QStringList& forkableTurnIds = {})
{
    Conversation conversation;
    conversation.setTitle(title);
    conversation.setTimeline(std::move(timeline));
    conversation.setForkableTurnIds(forkableTurnIds);

    HistoryEvent event;
    event.setKind(kind);
    event.setThreadId(QStringLiteral("thread-new"));
    event.setConversation(std::move(conversation));
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
};

void
CodexHistoryControllerTest::appliesModelCatalogUpdatesAndErrors()
{
    CodexHistoryController controller(nullptr);
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
    CodexHistoryController controller(nullptr);
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
    CodexHistoryController controller(nullptr);
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
    CodexHistoryController controller(nullptr);
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
}

void
CodexHistoryControllerTest::confirmsAcceptedTurnGuidance()
{
    CodexHistoryController controller(nullptr);
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
    CodexHistoryController controller(nullptr);
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
    CodexHistoryController controller(nullptr);
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
    CodexHistoryController controller(nullptr);
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
    CodexHistoryController controller(nullptr);
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
    CodexHistoryController controller(nullptr);
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
    CodexHistoryController controller(nullptr);
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
    CodexHistoryController controller(nullptr);
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
    CodexHistoryController controller(nullptr);
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
    CodexHistoryController controller(nullptr);
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
    CodexHistoryController controller(nullptr);
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
    CodexHistoryController controller(nullptr);
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
    CodexHistoryController controller(nullptr);
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
    QVERIFY(!conversation->startTurn(QStringLiteral("Continue")));
    QVERIFY(controller.errorMessage().isEmpty());
}

QTEST_APPLESS_MAIN(CodexHistoryControllerTest)

#include "codexhistorycontrollertest.moc"
