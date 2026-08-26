// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codextimelinepagemodel.h"

#include <QSignalSpy>
#include <QStandardItem>
#include <QStandardItemModel>
#include <QtTest/QTest>

namespace {
enum SourceRole
{
    EntryIdRole = Qt::UserRole + 1,
    TurnIdRole,
    ActivityGroupRole,
    StandaloneActivityRole,
    FromUserRole,
    CommentaryRole,
    FinalAnswerRole,
    PassthroughRole,
};

void
configureRoles(QStandardItemModel& model)
{
    model.setItemRoleNames({
      { EntryIdRole, "entryId" },
      { TurnIdRole, "turnId" },
      { ActivityGroupRole, "activityGroup" },
      { StandaloneActivityRole, "standaloneActivity" },
      { FromUserRole, "fromUser" },
      { CommentaryRole, "commentary" },
      { FinalAnswerRole, "finalAnswer" },
      { PassthroughRole, "passthroughValue" },
    });
}

void
appendRow(QStandardItemModel& model,
          const QString& entryId,
          const QString& turnId,
          bool fromUser = true,
          bool commentary = false,
          bool finalAnswer = false,
          bool activityGroup = false,
          bool standaloneActivity = false)
{
    auto* item = new QStandardItem;
    item->setData(entryId, EntryIdRole);
    item->setData(turnId, TurnIdRole);
    item->setData(activityGroup, ActivityGroupRole);
    item->setData(standaloneActivity, StandaloneActivityRole);
    item->setData(fromUser, FromUserRole);
    item->setData(commentary, CommentaryRole);
    item->setData(finalAnswer, FinalAnswerRole);
    item->setData(QStringLiteral("value:%1").arg(entryId), PassthroughRole);
    model.appendRow(item);
}

void
populateTimeline(QStandardItemModel& model)
{
    configureRoles(model);
    appendRow(model, QStringLiteral("entry-1a"), QStringLiteral("turn-1"));
    appendRow(model, QStringLiteral("entry-1b"), QStringLiteral("turn-1"));
    appendRow(model, QStringLiteral("entry-2"), QStringLiteral("turn-2"));
    appendRow(model, QStringLiteral("entry-3a"), QStringLiteral("turn-3"));
    appendRow(model, QStringLiteral("entry-3b"), QStringLiteral("turn-3"));
    appendRow(model, QStringLiteral("entry-4"), QStringLiteral("turn-4"));
    appendRow(model, QStringLiteral("entry-5"), QStringLiteral("turn-5"));
}
}

class CodexTimelinePageModelTest : public QObject
{
    Q_OBJECT

  private slots:
    void partitionsWholeTurnsWithoutSplittingRows();
    void keepsPageIdsStableAsTheTailGrows();
    void derivesDetailRowsFromSemanticRoles();
    void forwardsIncrementalSourceChanges();
};

void
CodexTimelinePageModelTest::partitionsWholeTurnsWithoutSplittingRows()
{
    QStandardItemModel source;
    populateTimeline(source);
    CodexTimelinePageModel model;
    model.setTurnsPerPage(2);
    model.setSourceModel(&source);

    QCOMPARE(model.rowCount(), 7);
    QCOMPARE(model.totalRowCount(), 7);
    QCOMPARE(model.pageCount(), 3);
    QCOMPARE(model.pageFirstRow(0), 0);
    QCOMPARE(model.pageRowCount(0), 3);
    QCOMPARE(model.pageId(0), QStringLiteral("page:entry-1a"));
    QCOMPARE(model.pageFirstRow(1), 3);
    QCOMPARE(model.pageRowCount(1), 3);
    QCOMPARE(model.pageId(1), QStringLiteral("page:entry-3a"));
    QCOMPARE(model.pageFirstRow(2), 6);
    QCOMPARE(model.pageRowCount(2), 1);
    QCOMPARE(model.pageId(2), QStringLiteral("page:entry-5"));
}

void
CodexTimelinePageModelTest::keepsPageIdsStableAsTheTailGrows()
{
    QStandardItemModel source;
    populateTimeline(source);
    CodexTimelinePageModel model;
    model.setTurnsPerPage(2);
    model.setSourceModel(&source);

    const QString firstPageId = model.pageId(0);
    const QString tailPageId = model.pageId(2);
    const int initialRevision = model.revision();

    appendRow(source, QStringLiteral("entry-5b"), QStringLiteral("turn-5"));
    QCOMPARE(model.pageCount(), 3);
    QCOMPARE(model.pageRowCount(2), 2);
    QCOMPARE(model.pageId(0), firstPageId);
    QCOMPARE(model.pageId(2), tailPageId);

    appendRow(source, QStringLiteral("entry-6"), QStringLiteral("turn-6"));
    QCOMPARE(model.pageCount(), 3);
    QCOMPARE(model.pageRowCount(2), 3);
    QCOMPARE(model.pageId(2), tailPageId);

    appendRow(source, QStringLiteral("entry-7"), QStringLiteral("turn-7"));
    QCOMPARE(model.pageCount(), 4);
    QCOMPARE(model.pageFirstRow(3), 9);
    QCOMPARE(model.pageId(3), QStringLiteral("page:entry-7"));
    QVERIFY(model.revision() > initialRevision);
}

void
CodexTimelinePageModelTest::derivesDetailRowsFromSemanticRoles()
{
    QStandardItemModel source;
    configureRoles(source);
    appendRow(source, QStringLiteral("user"), QStringLiteral("turn-1"));
    appendRow(source, QStringLiteral("reasoning"), QStringLiteral("turn-1"), false, false, false, true);
    appendRow(source, QStringLiteral("commentary"), QStringLiteral("turn-1"), false, true);
    appendRow(source, QStringLiteral("answer"), QStringLiteral("turn-1"), false, false, true);
    appendRow(source, QStringLiteral("compaction"), QStringLiteral("turn-2"), false, false, false, true, true);

    CodexTimelinePageModel model;
    model.setSourceModel(&source);

    QVERIFY(!model.valueAt(0, QStringLiteral("detailRow")).toBool());
    QVERIFY(model.valueAt(1, QStringLiteral("detailRow")).toBool());
    QVERIFY(model.valueAt(1, QStringLiteral("firstDetailInTurn")).toBool());
    QCOMPARE(model.valueAt(1, QStringLiteral("detailCountInTurn")).toInt(), 2);
    QVERIFY(model.valueAt(2, QStringLiteral("detailRow")).toBool());
    QVERIFY(!model.valueAt(2, QStringLiteral("firstDetailInTurn")).toBool());
    QVERIFY(!model.valueAt(3, QStringLiteral("detailRow")).toBool());
    QVERIFY(!model.valueAt(4, QStringLiteral("detailRow")).toBool());
    QVERIFY(model.valueAt(4, QStringLiteral("standaloneActivity")).toBool());
}

void
CodexTimelinePageModelTest::forwardsIncrementalSourceChanges()
{
    QStandardItemModel source;
    populateTimeline(source);
    CodexTimelinePageModel model;
    model.setSourceModel(&source);
    QSignalSpy insertedSpy(&model, &QAbstractItemModel::rowsInserted);
    QSignalSpy removedSpy(&model, &QAbstractItemModel::rowsRemoved);
    QSignalSpy changedSpy(&model, &QAbstractItemModel::dataChanged);

    appendRow(source, QStringLiteral("entry-5b"), QStringLiteral("turn-5"), false, false, true);
    QCOMPARE(insertedSpy.size(), 1);
    QCOMPARE(model.rowCount(), 8);
    changedSpy.clear();

    const int changedRow = model.rowCount() - 1;
    const int previousRevision = model.revision();
    source.item(changedRow)->setData(false, FinalAnswerRole);
    QCOMPARE(changedSpy.size(), 2);
    bool derivedRolesWerePublished = false;
    for (const QList<QVariant>& change : changedSpy) {
        const QList<int> roles = change.at(2).value<QList<int>>();
        if (roles.contains(CodexTimelinePageModel::DetailRowRole) &&
            roles.contains(CodexTimelinePageModel::FirstDetailInTurnRole) &&
            roles.contains(CodexTimelinePageModel::DetailCountInTurnRole)) {
            derivedRolesWerePublished = true;
        }
    }
    QVERIFY(derivedRolesWerePublished);
    QVERIFY(model.valueAt(changedRow, QStringLiteral("detailRow")).toBool());
    QVERIFY(model.revision() > previousRevision);
    QCOMPARE(model.valueAt(changedRow, QStringLiteral("entryId")).toString(), QStringLiteral("entry-5b"));
    QCOMPARE(model.valueAt(changedRow, QStringLiteral("passthroughValue")).toString(),
             QStringLiteral("value:entry-5b"));

    source.removeRow(changedRow);
    QCOMPARE(removedSpy.size(), 1);
    QCOMPARE(model.rowCount(), 7);
}

QTEST_GUILESS_MAIN(CodexTimelinePageModelTest)

#include "codextimelinepagemodeltest.moc"
