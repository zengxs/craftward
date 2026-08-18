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
                  const QString& title = QStringLiteral("New conversation"))
{
    Conversation conversation;
    conversation.setTitle(title);
    conversation.setTimeline(std::move(timeline));

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
    void confirmsAcceptedTurnGuidance();
    void reportsRejectedTurnGuidanceWithoutConfirmation();
    void validatesConversationRenamesBeforeDispatch();
    void appliesRenamedConversationTitles();
    void ignoresStaleThreadPagesAndWaitsForLifecycleConfirmation();
    void keepsLifecyclePendingAfterHistoryDecodingFailure();
    void keepsLifecyclePendingAfterMalformedThreadUpdates();
    void rejectsUnscopedThreadRecoveryAndErrorEvents();
    void reportsDedicatedThreadLifecycleErrors();
    void keepsArchivedConversationsReadOnly();
};

void
CodexHistoryControllerTest::replacesLiveFirstTurnWithItsPersistedSnapshot()
{
    CodexHistoryController controller(nullptr);
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

    CodexTimelineModel* timeline = controller.timeline();
    QCOMPARE(timeline->rowCount(), 2);
    QCOMPARE(timeline->data(timeline->index(0), CodexTimelineModel::EntryIdRole).toString(),
             QStringLiteral("message:live-turn-1:live-user-1"));
    QCOMPARE(timeline->data(timeline->index(1), CodexTimelineModel::EntryIdRole).toString(),
             QStringLiteral("message:live-turn-1:live-agent-1"));

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
                                                   }),
                                 {});

    QCOMPARE(timeline->rowCount(), 2);
    QCOMPARE(timeline->data(timeline->index(0), CodexTimelineModel::EntryIdRole).toString(),
             QStringLiteral("message:persisted-turn-1:persisted-user-1"));
    QCOMPARE(timeline->data(timeline->index(0), CodexTimelineModel::TextRole).toString(), QStringLiteral("Hello"));
    QCOMPARE(timeline->data(timeline->index(1), CodexTimelineModel::EntryIdRole).toString(),
             QStringLiteral("message:persisted-turn-1:persisted-agent-1"));
    QCOMPARE(timeline->data(timeline->index(1), CodexTimelineModel::TextRole).toString(), QStringLiteral("Done."));
}

void
CodexHistoryControllerTest::confirmsAcceptedTurnGuidance()
{
    CodexHistoryController controller(nullptr);
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    QSignalSpy steeredSpy(&controller, &CodexHistoryController::turnSteered);
    controller.setSteeringTurn(true);

    controller.applyHistoryEvent(turnOutcomeEvent(HistoryEventKind::HISTORY_EVENT_KIND_TURN_STEERED), {});

    QVERIFY(!controller.steeringTurn());
    QCOMPARE(steeredSpy.count(), 1);
}

void
CodexHistoryControllerTest::reportsRejectedTurnGuidanceWithoutConfirmation()
{
    CodexHistoryController controller(nullptr);
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    QSignalSpy steeredSpy(&controller, &CodexHistoryController::turnSteered);
    controller.setSteeringTurn(true);

    controller.applyHistoryEvent(
      turnOutcomeEvent(HistoryEventKind::HISTORY_EVENT_KIND_TURN_STEER_ERROR, QStringLiteral("The turn finished.")),
      {});

    QVERIFY(!controller.steeringTurn());
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
CodexHistoryControllerTest::appliesRenamedConversationTitles()
{
    CodexHistoryController controller(nullptr);
    controller.applyHistoryEvent(threadPageEvent(QStringLiteral("New conversation")), {});
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    QSignalSpy selectionSpy(&controller, &CodexHistoryController::selectionChanged);

    controller.applyHistoryEvent(threadPageEvent(QStringLiteral("Focused work")), {});
    controller.applyHistoryEvent(
      conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_UPDATED, {}, QStringLiteral("Focused work")),
      {});

    QCOMPARE(controller.threads()->rowCount(), 1);
    QCOMPARE(controller.threads()->data(controller.threads()->index(0), CodexThreadModel::TitleRole).toString(),
             QStringLiteral("Focused work"));
    QCOMPARE(controller.selectedThreadTitle(), QStringLiteral("Focused work"));
    QCOMPARE(selectionSpy.count(), 1);
}

void
CodexHistoryControllerTest::ignoresStaleThreadPagesAndWaitsForLifecycleConfirmation()
{
    CodexHistoryController controller(nullptr);
    controller.applyHistoryEvent(threadPageEvent(QStringLiteral("New conversation")), {});
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    controller.pendingLifecycleThreadId_ = QStringLiteral("thread-new");
    controller.changingThreadLifecycle_ = true;
    controller.loadingThreads_ = true;

    controller.applyHistoryEvent(threadPageEvent(QStringLiteral("New conversation"), true, false), {});
    QCOMPARE(controller.threads()->rowCount(), 1);
    QCOMPARE(controller.selectedThreadId(), QStringLiteral("thread-new"));
    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());

    controller.applyHistoryEvent(threadListErrorEvent(true, QStringLiteral("Stale archived error")), {});
    QCOMPARE(controller.selectedThreadId(), QStringLiteral("thread-new"));
    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());
    QVERIFY(controller.errorMessage().isEmpty());

    controller.applyHistoryEvent(turnOutcomeEvent(HistoryEventKind::HISTORY_EVENT_KIND_CONVERSATION_ERROR,
                                                  QStringLiteral("Temporary conversation error")),
                                 {});
    QCOMPARE(controller.selectedThreadId(), QStringLiteral("thread-new"));
    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());
    QCOMPARE(controller.errorMessage(), QStringLiteral("Temporary conversation error"));

    controller.clearError();
    controller.applyHistoryEvent(threadListErrorEvent(false, QStringLiteral("Temporary active error")), {});
    QCOMPARE(controller.selectedThreadId(), QStringLiteral("thread-new"));
    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());
    QCOMPARE(controller.errorMessage(), QStringLiteral("Temporary active error"));

    controller.applyHistoryEvent(threadPageEvent(QStringLiteral("New conversation")), {});
    QCOMPARE(controller.threads()->rowCount(), 1);
    QCOMPARE(controller.selectedThreadId(), QStringLiteral("thread-new"));
    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());

    controller.applyHistoryEvent(threadPageEvent(QStringLiteral("New conversation"), false, false), {});
    QCOMPARE(controller.threads()->rowCount(), 0);
    QVERIFY(controller.selectedThreadId().isEmpty());
    QCOMPARE(controller.timeline()->rowCount(), 0);
    QVERIFY(!controller.changingThreadLifecycle());
    QVERIFY(!controller.loadingThreads());
}

void
CodexHistoryControllerTest::keepsLifecyclePendingAfterHistoryDecodingFailure()
{
    CodexHistoryController controller(nullptr);
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    controller.pendingLifecycleThreadId_ = QStringLiteral("thread-new");
    controller.changingThreadLifecycle_ = true;
    controller.loadingThreads_ = true;

    controller.applyHistoryEvent({}, QStringLiteral("Malformed history event"));

    QCOMPARE(controller.selectedThreadId(), QStringLiteral("thread-new"));
    QVERIFY(controller.changingThreadLifecycle());
    QVERIFY(controller.loadingThreads());
    QCOMPARE(controller.errorMessage(), QStringLiteral("Malformed history event"));
}

void
CodexHistoryControllerTest::keepsLifecyclePendingAfterMalformedThreadUpdates()
{
    CodexHistoryController controller(nullptr);
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

    QCOMPARE(controller.selectedThreadId(), QStringLiteral("thread-new"));
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
    controller.applyHistoryEvent(conversationEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_STARTED, {}), {});
    controller.pendingLifecycleThreadId_ = QStringLiteral("thread-new");
    controller.changingThreadLifecycle_ = true;
    controller.loadingThreads_ = true;

    controller.applyHistoryEvent(
      turnOutcomeEvent(HistoryEventKind::HISTORY_EVENT_KIND_THREAD_LIFECYCLE_ERROR, QStringLiteral("Archive failed")),
      {});

    QCOMPARE(controller.selectedThreadId(), QStringLiteral("thread-new"));
    QVERIFY(!controller.changingThreadLifecycle());
    QVERIFY(!controller.loadingThreads());
    QCOMPARE(controller.errorMessage(), QStringLiteral("Archive failed"));
}

void
CodexHistoryControllerTest::keepsArchivedConversationsReadOnly()
{
    CodexHistoryController controller(nullptr);
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
    controller.acquireWriteAccess();
    QCOMPARE(controller.writeAvailability(), CodexHistoryController::WriteAvailability::NotRequested);
    controller.setWriteAvailability(CodexHistoryController::WriteAvailability::Writable);
    QVERIFY(!controller.startTurn(QStringLiteral("Continue")));
    QVERIFY(controller.errorMessage().isEmpty());
}

QTEST_APPLESS_MAIN(CodexHistoryControllerTest)

#include "codexhistorycontrollertest.moc"
