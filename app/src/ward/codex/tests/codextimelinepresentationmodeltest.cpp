// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codextimelinepresentationmodel.h"

#include "timelinepresentationsourcemodelfixture.h"

#include <QAbstractItemModelTester>
#include <QSignalSpy>
#include <QStandardItem>
#include <QStandardItemModel>
#include <QtTest/QTest>

#include <memory>

namespace {
using namespace TimelinePresentationSourceFixture;

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

class CountingPresentationModel : public CodexTimelinePresentationModel
{
  public:
    mutable int indexRequestCount = 0;

    QModelIndex index(int row, int column, const QModelIndex& parent = {}) const override
    {
        ++indexRequestCount;
        return CodexTimelinePresentationModel::index(row, column, parent);
    }
};

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
    void publishesExpansionChangesWithoutIndexingTheWholeHistory();
    void looksUpEntryIdsWithoutReadingModelRows();
    void preservesSourceRolesThatMatchFormerInternalRole();
    void notifiesSourceRemovalAfterResettingRows();
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

    QCOMPARE(model.sourceModel(), &source);
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
    QAbstractItemModelTester modelTester(&model, QAbstractItemModelTester::FailureReportingMode::QtTest);
    QCOMPARE(model.rowCount(), 3);
    QSignalSpy insertedSpy(&model, &QAbstractItemModel::rowsInserted);
    QSignalSpy removedSpy(&model, &QAbstractItemModel::rowsRemoved);

    model.setTurnExpanded(QStringLiteral("turn-1"), true);
    QCOMPARE(model.rowCount(), 4);
    QCOMPARE(model.entryIdAt(2), QStringLiteral("detail-2"));
    QCOMPARE(model.indexOfEntryId(QStringLiteral("detail-2")), 2);
    QVERIFY(model.valueAt(1, QStringLiteral("turnExpanded")).toBool());
    QCOMPARE(insertedSpy.size(), 1);

    model.setTurnExpanded(QStringLiteral("turn-1"), false);
    QCOMPARE(model.rowCount(), 3);
    QCOMPARE(model.entryIdAt(2), QStringLiteral("answer"));
    QCOMPARE(model.indexOfEntryId(QStringLiteral("detail-2")), -1);
    QCOMPARE(model.indexOfEntryId(QStringLiteral("answer")), 2);
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

    source.item(1)->setData(QStringLiteral("updated-answer"), EntryIdRole);

    QCOMPARE(model.indexOfEntryId(QStringLiteral("answer")), -1);
    QCOMPARE(model.indexOfEntryId(QStringLiteral("updated-answer")), 1);
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

void
CodexTimelinePresentationModelTest::publishesExpansionChangesWithoutIndexingTheWholeHistory()
{
    QStandardItemModel source;
    configureRoles(source);
    constexpr int rowCount = 2000;
    for (int row = 0; row < rowCount; ++row) {
        if (row == rowCount / 2) {
            appendRow(source, QStringLiteral("detail-1"), QStringLiteral("target-turn"), true);
            appendRow(source, QStringLiteral("detail-2"), QStringLiteral("target-turn"), true);
            appendRow(source, QStringLiteral("detail-3"), QStringLiteral("target-turn"), true);
            continue;
        }
        appendRow(
          source, QStringLiteral("entry-%1").arg(row), QStringLiteral("turn-%1").arg(row), false, false, false, false);
    }

    CountingPresentationModel model;
    model.setSourceModel(&source);
    QCOMPARE(model.rowCount(), rowCount);
    model.indexRequestCount = 0;
    model.filterEvaluationCount_ = 0;

    model.setTurnExpanded(QStringLiteral("target-turn"), true);

    QCOMPARE(model.rowCount(), rowCount + 2);
    QVERIFY2(model.indexRequestCount < 20,
             qPrintable(QStringLiteral("One turn expansion requested %1 proxy indexes").arg(model.indexRequestCount)));
    QVERIFY2(
      model.filterEvaluationCount_ < 20,
      qPrintable(QStringLiteral("One turn expansion refiltered %1 source rows").arg(model.filterEvaluationCount_)));
}

void
CodexTimelinePresentationModelTest::looksUpEntryIdsWithoutReadingModelRows()
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
                  true);
    }

    CodexTimelinePresentationModel model;
    model.setSourceModel(&source);
    source.dataReadCount = 0;

    QCOMPARE(model.indexOfEntryId(QStringLiteral("entry-1999")), rowCount - 1);
    QCOMPARE(source.dataReadCount, 0);
}

void
CodexTimelinePresentationModelTest::preservesSourceRolesThatMatchFormerInternalRole()
{
    QStandardItemModel source;
    configureRoles(source);
    appendRow(source, QStringLiteral("answer"), QStringLiteral("turn-1"), false, false, false, false, true);

    CodexTimelinePresentationModel model;
    model.setSourceModel(&source);

    QCOMPARE(model.roleNames().value(CollisionProbeRole), QByteArray("collisionProbe"));
}

void
CodexTimelinePresentationModelTest::notifiesSourceRemovalAfterResettingRows()
{
    auto source = std::make_unique<QStandardItemModel>();
    configureRoles(*source);
    appendRow(*source, QStringLiteral("answer"), QStringLiteral("turn-1"), false, false, false, false, true);

    CodexTimelinePresentationModel model;
    model.setSourceModel(source.get());
    QCOMPARE(model.rowCount(), 1);
    QSignalSpy resetSpy(&model, &QAbstractItemModel::modelReset);
    int rowCountWhenSourceChanged = -1;
    connect(&model, &CodexTimelinePresentationModel::sourceModelChanged, &model, [&] {
        if (!model.sourceModel())
            rowCountWhenSourceChanged = model.rowCount();
    });

    source.reset();

    QCOMPARE(resetSpy.size(), 1);
    QCOMPARE(model.sourceModel(), nullptr);
    QCOMPARE(model.rowCount(), 0);
    QCOMPARE(rowCountWhenSourceChanged, 0);
}

QTEST_GUILESS_MAIN(CodexTimelinePresentationModelTest)

#include "codextimelinepresentationmodeltest.moc"
