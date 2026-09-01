// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codextimelinepresentationmodel.h"

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
    TextRole,
};

class CountingItemModel : public QStandardItemModel
{
  public:
    mutable int dataReadCount = 0;

    QVariant data(const QModelIndex& index, int role = Qt::DisplayRole) const override
    {
        ++dataReadCount;
        return QStandardItemModel::data(index, role);
    }
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
      { TextRole, "text" },
    });
}

void
appendRow(QStandardItemModel& model,
          const QString& entryId,
          const QString& turnId,
          bool activityGroup = false,
          bool standaloneActivity = false,
          bool fromUser = false,
          bool commentary = false,
          bool finalAnswer = false)
{
    auto* item = new QStandardItem;
    item->setData(entryId, EntryIdRole);
    item->setData(turnId, TurnIdRole);
    item->setData(activityGroup, ActivityGroupRole);
    item->setData(standaloneActivity, StandaloneActivityRole);
    item->setData(fromUser, FromUserRole);
    item->setData(commentary, CommentaryRole);
    item->setData(finalAnswer, FinalAnswerRole);
    model.appendRow(item);
}
}

class CodexTimelinePresentationModelTest : public QObject
{
    Q_OBJECT

  private slots:
    void filtersCollapsedDetailRowsWithoutKeepingZeroHeightEntries();
    void expandsAndCollapsesOneTurnIncrementally();
    void updatesDerivedVisibilityWhenAMessageBecomesFinal();
    void forwardsSourceChangesThroughStablePresentationRows();
    void forwardsContentUpdatesWithoutReadingPresentationRoles();
    void updatesOneTurnWithoutRescanningTheHistory();
};

void
CodexTimelinePresentationModelTest::filtersCollapsedDetailRowsWithoutKeepingZeroHeightEntries()
{
    QStandardItemModel source;
    configureRoles(source);
    appendRow(source, QStringLiteral("user"), QStringLiteral("turn-1"), false, false, true);
    appendRow(source, QStringLiteral("detail-1"), QStringLiteral("turn-1"), true);
    appendRow(source, QStringLiteral("detail-2"), QStringLiteral("turn-1"), true);
    appendRow(source, QStringLiteral("answer"), QStringLiteral("turn-1"), false, false, false, false, true);
    appendRow(source, QStringLiteral("compaction"), QStringLiteral("turn-2"), true, true);

    CodexTimelinePresentationModel model;
    model.setSourceModel(&source);

    QCOMPARE(model.rowCount(), 4);
    QCOMPARE(model.entryIdAt(0), QStringLiteral("user"));
    QCOMPARE(model.entryIdAt(1), QStringLiteral("detail-1"));
    QCOMPARE(model.entryIdAt(2), QStringLiteral("answer"));
    QCOMPARE(model.entryIdAt(3), QStringLiteral("compaction"));
    QVERIFY(model.valueAt(1, QStringLiteral("detailRow")).toBool());
    QVERIFY(model.valueAt(1, QStringLiteral("firstDetailInTurn")).toBool());
    QCOMPARE(model.valueAt(1, QStringLiteral("detailCountInTurn")).toInt(), 2);
    QVERIFY(!model.valueAt(3, QStringLiteral("detailRow")).toBool());
}

void
CodexTimelinePresentationModelTest::expandsAndCollapsesOneTurnIncrementally()
{
    QStandardItemModel source;
    configureRoles(source);
    appendRow(source, QStringLiteral("user"), QStringLiteral("turn-1"), false, false, true);
    appendRow(source, QStringLiteral("detail-1"), QStringLiteral("turn-1"), true);
    appendRow(source, QStringLiteral("detail-2"), QStringLiteral("turn-1"), true);
    appendRow(source, QStringLiteral("answer"), QStringLiteral("turn-1"), false, false, false, false, true);

    CodexTimelinePresentationModel model;
    model.setSourceModel(&source);
    QSignalSpy insertedSpy(&model, &QAbstractItemModel::rowsInserted);
    QSignalSpy removedSpy(&model, &QAbstractItemModel::rowsRemoved);

    model.setTurnExpanded(QStringLiteral("turn-1"), true);
    QCOMPARE(model.rowCount(), 4);
    QCOMPARE(model.entryIdAt(2), QStringLiteral("detail-2"));
    QVERIFY(model.valueAt(1, QStringLiteral("turnExpanded")).toBool());
    QCOMPARE(insertedSpy.size(), 1);

    model.setTurnExpanded(QStringLiteral("turn-1"), false);
    QCOMPARE(model.rowCount(), 3);
    QCOMPARE(model.entryIdAt(2), QStringLiteral("answer"));
    QCOMPARE(removedSpy.size(), 1);
}

void
CodexTimelinePresentationModelTest::updatesDerivedVisibilityWhenAMessageBecomesFinal()
{
    QStandardItemModel source;
    configureRoles(source);
    appendRow(source, QStringLiteral("user"), QStringLiteral("turn-1"), false, false, true);
    appendRow(source, QStringLiteral("reasoning"), QStringLiteral("turn-1"), true);
    appendRow(source, QStringLiteral("draft-answer"), QStringLiteral("turn-1"));

    CodexTimelinePresentationModel model;
    model.setSourceModel(&source);
    QCOMPARE(model.rowCount(), 2);
    QSignalSpy insertedSpy(&model, &QAbstractItemModel::rowsInserted);

    source.item(2)->setData(true, FinalAnswerRole);

    QCOMPARE(model.rowCount(), 3);
    QCOMPARE(model.entryIdAt(2), QStringLiteral("draft-answer"));
    QVERIFY(!model.valueAt(2, QStringLiteral("detailRow")).toBool());
    QCOMPARE(insertedSpy.size(), 1);
}

void
CodexTimelinePresentationModelTest::forwardsSourceChangesThroughStablePresentationRows()
{
    QStandardItemModel source;
    configureRoles(source);
    appendRow(source, QStringLiteral("user"), QStringLiteral("turn-1"), false, false, true);

    CodexTimelinePresentationModel model;
    model.setSourceModel(&source);
    const int previousRevision = model.revision();
    QSignalSpy insertedSpy(&model, &QAbstractItemModel::rowsInserted);

    appendRow(source, QStringLiteral("answer"), QStringLiteral("turn-1"), false, false, false, false, true);

    QCOMPARE(model.rowCount(), 2);
    QCOMPARE(model.entryIdAt(1), QStringLiteral("answer"));
    QCOMPARE(insertedSpy.size(), 1);
    QVERIFY(model.revision() > previousRevision);
}

void
CodexTimelinePresentationModelTest::forwardsContentUpdatesWithoutReadingPresentationRoles()
{
    CountingItemModel source;
    configureRoles(source);
    appendRow(source, QStringLiteral("answer"), QStringLiteral("turn-1"), false, false, false, false, true);

    CodexTimelinePresentationModel model;
    model.setSourceModel(&source);
    const int previousRevision = model.revision();
    source.dataReadCount = 0;

    source.item(0)->setData(QStringLiteral("Streaming content"), TextRole);

    QVERIFY(model.revision() > previousRevision);
    QCOMPARE(source.dataReadCount, 0);
}

void
CodexTimelinePresentationModelTest::updatesOneTurnWithoutRescanningTheHistory()
{
    CountingItemModel source;
    configureRoles(source);
    constexpr int rowCount = 2000;
    for (int row = 0; row < rowCount; ++row) {
        appendRow(source,
                  QStringLiteral("entry-%1").arg(row),
                  QStringLiteral("turn-%1").arg(row),
                  false,
                  false,
                  false,
                  false,
                  false);
    }

    CodexTimelinePresentationModel model;
    model.setSourceModel(&source);
    source.dataReadCount = 0;

    source.item(rowCount / 2)->setData(true, FinalAnswerRole);

    const int updateDataReadCount = source.dataReadCount;
    QVERIFY(model.indexOfEntryId(QStringLiteral("entry-1000")) >= 0);
    QVERIFY2(updateDataReadCount < 100,
             qPrintable(QStringLiteral("One row update caused %1 source data reads").arg(updateDataReadCount)));
}

QTEST_GUILESS_MAIN(CodexTimelinePresentationModelTest)

#include "codextimelinepresentationmodeltest.moc"
