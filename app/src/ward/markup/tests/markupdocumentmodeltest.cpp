// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/markup/markupdocumentmodel.h"

#include <QtTest/QSignalSpy>
#include <QtTest/QTest>

class MarkupDocumentModelTest : public QObject
{
    Q_OBJECT

  private slots:
    void preservesPlainText();
    void exposesMarkdownCodeAsAnIndependentBlock();
    void updatesOnlyTheGrowingTail();
    void insertsANewlyCompletedBlockWithoutResettingThePrefix();
    void coalescesRapidStreamingSnapshots();
    void reparsesOnlyTheBoundedMutableTailBeforeFinalization();
};

void
MarkupDocumentModelTest::preservesPlainText()
{
    MarkupDocumentModel model;

    QVERIFY(model.reconcileSource(QStringLiteral("# Not a heading"), MarkupDocumentModel::SourceFormat::PlainText));

    QCOMPARE(model.rowCount(), 1);
    const QModelIndex row = model.index(0);
    QCOMPARE(model.data(row, MarkupDocumentModel::BlockTextRole).toString(), QStringLiteral("# Not a heading"));
    QVERIFY(!model.data(row, MarkupDocumentModel::MarkdownRole).toBool());
    QVERIFY(!model.data(row, MarkupDocumentModel::CodeBlockRole).toBool());
}

void
MarkupDocumentModelTest::exposesMarkdownCodeAsAnIndependentBlock()
{
    MarkupDocumentModel model;
    const QString source = QStringLiteral("Before\n\n```rust\nfn main() {}\n```\n\nAfter");

    QVERIFY(model.reconcileSource(source, MarkupDocumentModel::SourceFormat::Markdown));

    QTRY_COMPARE(model.rowCount(), 3);
    QVERIFY(model.data(model.index(0), MarkupDocumentModel::MarkdownRole).toBool());
    QVERIFY(model.data(model.index(1), MarkupDocumentModel::CodeBlockRole).toBool());
    QCOMPARE(model.data(model.index(1), MarkupDocumentModel::LanguageRole).toString(), QStringLiteral("rust"));
    QCOMPARE(model.data(model.index(1), MarkupDocumentModel::BlockTextRole).toString(),
             QStringLiteral("fn main() {}\n"));
    QCOMPARE(model.data(model.index(2), MarkupDocumentModel::PlainTextRole).toString(), QStringLiteral("After"));
}

void
MarkupDocumentModelTest::updatesOnlyTheGrowingTail()
{
    MarkupDocumentModel model;
    const QString prefix = QStringLiteral("Before\n\n```sh\necho ready\n```\n\n");
    QVERIFY(model.reconcileSource(prefix + QStringLiteral("After"), MarkupDocumentModel::SourceFormat::Markdown));
    QTRY_COMPARE(model.rowCount(), 3);
    const QString stableCodeId = model.data(model.index(1), MarkupDocumentModel::BlockIdRole).toString();
    QSignalSpy resetSpy(&model, &QAbstractItemModel::modelReset);
    QSignalSpy changedSpy(&model, &QAbstractItemModel::dataChanged);
    QSignalSpy insertedSpy(&model, &QAbstractItemModel::rowsInserted);
    QSignalSpy removedSpy(&model, &QAbstractItemModel::rowsRemoved);

    QVERIFY(model.reconcileSource(prefix + QStringLiteral("After more"), MarkupDocumentModel::SourceFormat::Markdown));

    QTRY_COMPARE(model.data(model.index(2), MarkupDocumentModel::PlainTextRole).toString(),
                 QStringLiteral("After more"));
    QCOMPARE(model.rowCount(), 3);
    QCOMPARE(model.data(model.index(1), MarkupDocumentModel::BlockIdRole).toString(), stableCodeId);
    QCOMPARE(model.data(model.index(2), MarkupDocumentModel::PlainTextRole).toString(), QStringLiteral("After more"));
    QCOMPARE(resetSpy.count(), 0);
    QCOMPARE(changedSpy.count(), 1);
    QCOMPARE(insertedSpy.count(), 0);
    QCOMPARE(removedSpy.count(), 0);
}

void
MarkupDocumentModelTest::insertsANewlyCompletedBlockWithoutResettingThePrefix()
{
    MarkupDocumentModel model;
    const QString initial = QStringLiteral("Before\n\n```sh\necho ready\n```\n\nAfter");
    QVERIFY(model.reconcileSource(initial, MarkupDocumentModel::SourceFormat::Markdown));
    QTRY_COMPARE(model.rowCount(), 3);
    const QString stableTailId = model.data(model.index(2), MarkupDocumentModel::BlockIdRole).toString();
    QSignalSpy resetSpy(&model, &QAbstractItemModel::modelReset);
    QSignalSpy insertedSpy(&model, &QAbstractItemModel::rowsInserted);

    QVERIFY(model.reconcileSource(initial + QStringLiteral("\n\n```text\nnew block\n```"),
                                  MarkupDocumentModel::SourceFormat::Markdown));

    QTRY_COMPARE(model.rowCount(), 4);
    QCOMPARE(model.data(model.index(2), MarkupDocumentModel::BlockIdRole).toString(), stableTailId);
    QVERIFY(model.data(model.index(3), MarkupDocumentModel::CodeBlockRole).toBool());
    QCOMPARE(resetSpy.count(), 0);
    QCOMPARE(insertedSpy.count(), 1);
}

void
MarkupDocumentModelTest::coalescesRapidStreamingSnapshots()
{
    MarkupDocumentModel model;
    QVERIFY(model.reconcileSource(QStringLiteral("Initial"), MarkupDocumentModel::SourceFormat::Markdown));
    QTRY_COMPARE(model.rowCount(), 1);
    QSignalSpy reconciledSpy(&model, &MarkupDocumentModel::documentReconciled);

    for (int snapshot = 1; snapshot <= 40; ++snapshot) {
        QVERIFY(model.reconcileSource(QStringLiteral("Initial %1").arg(snapshot),
                                      MarkupDocumentModel::SourceFormat::Markdown));
    }

    QCOMPARE(model.data(model.index(0), MarkupDocumentModel::BlockTextRole).toString(), QStringLiteral("Initial"));
    QTRY_COMPARE(model.data(model.index(0), MarkupDocumentModel::BlockTextRole).toString(),
                 QStringLiteral("Initial 40"));
    QCOMPARE(reconciledSpy.count(), 1);
}

void
MarkupDocumentModelTest::reparsesOnlyTheBoundedMutableTailBeforeFinalization()
{
    MarkupDocumentModel model;
    QString source = QStringLiteral("One.\n\nTwo.\n\nThree.\n\nFour.\n\nFive.\n\nSix.\n\nSeven.\n\nEight.\n\nNine.");
    QVERIFY(model.reconcileSource(source, MarkupDocumentModel::SourceFormat::Markdown, false));
    QTRY_COMPARE(model.rowCount(), 3);
    const QString firstGroupId = model.data(model.index(0), MarkupDocumentModel::BlockIdRole).toString();
    const QString secondGroupId = model.data(model.index(1), MarkupDocumentModel::BlockIdRole).toString();

    source += QStringLiteral("\n\nTen.\n\nEleven.\n\nTwelve.\n\nThirteen.");
    QVERIFY(model.reconcileSource(source, MarkupDocumentModel::SourceFormat::Markdown, false));
    QTRY_COMPARE(model.rowCount(), 4);
    QCOMPARE(model.data(model.index(0), MarkupDocumentModel::BlockIdRole).toString(), firstGroupId);
    QCOMPARE(model.data(model.index(1), MarkupDocumentModel::BlockIdRole).toString(), secondGroupId);
    QCOMPARE(model.data(model.index(2), MarkupDocumentModel::SourceStartRole).toULongLong(),
             static_cast<qulonglong>(source.toUtf8().indexOf("Nine.")));

    QSignalSpy resetSpy(&model, &QAbstractItemModel::modelReset);
    QSignalSpy reconciledSpy(&model, &MarkupDocumentModel::documentReconciled);
    QVERIFY(model.reconcileSource(source, MarkupDocumentModel::SourceFormat::Markdown, true));
    QTRY_COMPARE(reconciledSpy.count(), 1);
    QCOMPARE(resetSpy.count(), 0);
}

QTEST_GUILESS_MAIN(MarkupDocumentModelTest)

#include "markupdocumentmodeltest.moc"
