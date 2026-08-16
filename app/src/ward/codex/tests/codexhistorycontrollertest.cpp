// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexhistorycontroller.h"

#include <QtTest/QTest>

namespace {
using Conversation = ward::codex::v1::Conversation;
using HistoryEvent = ward::codex::v1::HistoryEvent;
using HistoryEventKind = ward::codex::v1::HistoryEventKindGadget::HistoryEventKind;
using Message = ward::codex::v1::Message;
using MessagePhase = ward::codex::v1::MessagePhaseGadget::MessagePhase;
using MessageRole = ward::codex::v1::MessageRoleGadget::MessageRole;
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
conversationEvent(HistoryEventKind kind, QList<TimelineItem> timeline)
{
    Conversation conversation;
    conversation.setTitle(QStringLiteral("New conversation"));
    conversation.setTimeline(std::move(timeline));

    HistoryEvent event;
    event.setKind(kind);
    event.setThreadId(QStringLiteral("thread-new"));
    event.setConversation(std::move(conversation));
    return event;
}
}

class CodexHistoryControllerTest : public QObject
{
    Q_OBJECT

  private slots:
    void replacesLiveFirstTurnWithItsPersistedSnapshot();
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

QTEST_APPLESS_MAIN(CodexHistoryControllerTest)

#include "codexhistorycontrollertest.moc"
