// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/markup/markupdocumentmodel.h"
#include "ward/markup/markuprendermodel.h"

#include "document.qpb.h"
#include <ward_core.h>

#include <QByteArrayView>
#include <QScopeGuard>
#include <QSemaphore>
#include <QStandardItem>
#include <QStandardItemModel>
#include <QThreadPool>
#include <QtProtobuf/QProtobufSerializer>
#include <QtTest/QSignalSpy>
#include <QtTest/QTest>

#include <memory>

class MarkupDocumentModelTest : public QObject
{
    Q_OBJECT

  private slots:
    void preservesPlainText();
    void decodesSemanticSnapshotFromRust();
    void exposesMarkdownCodeAsAnIndependentBlock();
    void preparesOnlyTheRequestedDocumentBeforeLayout();
    void ignoresAsyncCompletionAfterLayoutPreparation_data();
    void ignoresAsyncCompletionAfterLayoutPreparation();
    void updatesOnlyTheGrowingTail();
    void insertsANewlyCompletedBlockWithoutResettingThePrefix();
    void coalescesRapidStreamingSnapshots();
    void reparsesOnlyTheBoundedMutableTailBeforeFinalization();
    void groupsAdjacentProseForRendering();
    void removesOneTerminalCodeLineEndingForRendering();
};

void
MarkupDocumentModelTest::decodesSemanticSnapshotFromRust()
{
    const QByteArray source =
      QStringLiteral("你好 👩‍💻 &amp; **bold** :codex-annotation{index=\"4\"}\n\n"
                     "0. [ ] task\n\n| A | B |\n|---|---:|\n| `a()` | [Ready][r] |\n\n[r]: /ready \"Status\"")
        .toUtf8();
    WardError* error = nullptr;
    const auto releaseError = qScopeGuard([&] {
        if (error)
            ward_core_error_destroy(error);
    });
    using Buffer = std::unique_ptr<WardOwnedBuffer, decltype(&ward_core_owned_buffer_destroy)>;
    Buffer buffer(
      ward_core_markup_parse_semantic(
        WardMarkupSourceFormatMarkdown, reinterpret_cast<const uint8_t*>(source.constData()), source.size(), &error),
      &ward_core_owned_buffer_destroy);
    QVERIFY(buffer);
    QVERIFY(!error);
    const QByteArrayView bytes(reinterpret_cast<const char*>(ward_core_owned_buffer_data(buffer.get())),
                               ward_core_owned_buffer_size(buffer.get()));
    ward::markup::v1::SemanticDocument document;
    QProtobufSerializer serializer;
    QVERIFY2(document.deserialize(&serializer, bytes), qPrintable(serializer.lastErrorString()));
    buffer.reset();
    QCOMPARE(document.blocks().size(), 3);
    const auto& intro = document.blocks().first().nodes();
    QVERIFY(!intro.first().hasParentIndex());
    QVERIFY(intro.at(1).hasParentIndex());
    QCOMPARE(intro.at(1).parentIndex(), 0u);
    const auto text = intro.at(1).text().value();
    QCOMPARE(text.text(), QStringLiteral("你好 👩‍💻 "));
    QCOMPARE(text.mappings().first().utf16End(), quint64(text.text().size()));
    QCOMPARE(text.mappings().first().source().end(), quint64(text.text().toUtf8().size()));
    QVERIFY(text.mappings().first().verbatim());
    bool annotation = false;
    bool link = false;
    bool uncheckedTask = false;
    bool bodyRow = false;
    bool zeroStart = false;
    bool entity = false;
    for (const auto& block : document.blocks()) {
        for (const auto& node : block.nodes()) {
            if (node.hasAnnotation()) {
                annotation = true;
                QCOMPARE(node.annotation().index(), 4u);
                QCOMPARE(node.annotation().label().text(), QStringLiteral("[4]"));
                QVERIFY(!node.annotation().label().mappings().first().verbatim());
            }
            if (node.hasLink()) {
                link = true;
                QCOMPARE(node.link().target(), QStringLiteral("/ready"));
                QCOMPARE(node.link().title(), QStringLiteral("Status"));
            }
            uncheckedTask |= node.hasTaskChecked() && !node.taskChecked();
            bodyRow |= node.hasTableRowHeader() && !node.tableRowHeader();
            zeroStart |= node.hasList() && node.list().hasStart() && node.list().start() == 0;
            if (node.hasText() && node.text().value().text() == QStringLiteral("&")) {
                entity = true;
                const auto mapping = node.text().value().mappings().first();
                QVERIFY(!mapping.verbatim());
                QCOMPARE(mapping.utf16End(), 1u);
                QCOMPARE(source.mid(mapping.source().start(), mapping.source().end() - mapping.source().start()),
                         QByteArray("&amp;"));
            }
        }
    }
    QVERIFY(annotation && link && uncheckedTask && bodyRow && zeroStart && entity);
}

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
MarkupDocumentModelTest::preparesOnlyTheRequestedDocumentBeforeLayout()
{
    MarkupDocumentModel visibleDocument;
    MarkupDocumentModel untouchedDocument;
    const QString source = QStringLiteral("Before\n\n```cpp\nreturn 0;\n```\n\nAfter");
    QVERIFY(visibleDocument.reconcileSource(source, MarkupDocumentModel::SourceFormat::Markdown));
    QVERIFY(untouchedDocument.reconcileSource(source, MarkupDocumentModel::SourceFormat::Markdown));
    QCOMPARE(visibleDocument.rowCount(), 0);
    QCOMPARE(untouchedDocument.rowCount(), 0);
    QSignalSpy reconciledSpy(&visibleDocument, &MarkupDocumentModel::documentReconciled);

    QVERIFY(QMetaObject::invokeMethod(&visibleDocument, "prepareForLayout", Qt::DirectConnection));

    QCOMPARE(visibleDocument.rowCount(), 3);
    QCOMPARE(untouchedDocument.rowCount(), 0);
    QCOMPARE(reconciledSpy.count(), 1);
    QVERIFY(QMetaObject::invokeMethod(&visibleDocument, "prepareForLayout", Qt::DirectConnection));
    QTRY_COMPARE(untouchedDocument.rowCount(), 3);
    QCOMPARE(reconciledSpy.count(), 1);

    QVERIFY(visibleDocument.reconcileSource(
      source + QStringLiteral(" more"), MarkupDocumentModel::SourceFormat::Markdown, false));
    QVERIFY(QMetaObject::invokeMethod(&visibleDocument, "prepareForLayout", Qt::DirectConnection));
    QCOMPARE(visibleDocument.data(visibleDocument.index(2), MarkupDocumentModel::PlainTextRole).toString(),
             QStringLiteral("After more"));
    QCOMPARE(reconciledSpy.count(), 2);
}

void
MarkupDocumentModelTest::ignoresAsyncCompletionAfterLayoutPreparation_data()
{
    QTest::addColumn<bool>("newerSnapshot");
    QTest::newRow("duplicate-generation") << false;
    QTest::newRow("stale-generation") << true;
}

void
MarkupDocumentModelTest::ignoresAsyncCompletionAfterLayoutPreparation()
{
    QFETCH(bool, newerSnapshot);
    QThreadPool* pool = QThreadPool::globalInstance();
    pool->waitForDone();
    const int previousThreadCount = pool->maxThreadCount();
    pool->setMaxThreadCount(1);
    QSemaphore workerStarted;
    QSemaphore releaseWorker;
    bool workerReleased = false;
    const auto restorePool = qScopeGuard([&] {
        if (!workerReleased)
            releaseWorker.release();
        pool->waitForDone();
        pool->setMaxThreadCount(previousThreadCount);
    });
    pool->start([&] {
        workerStarted.release();
        releaseWorker.acquire();
    });
    workerStarted.acquire();

    MarkupDocumentModel model;
    QString source = QStringLiteral("Before\n\n```cpp\nreturn 0;\n```\n\nAfter");
    QVERIFY(model.reconcileSource(source, MarkupDocumentModel::SourceFormat::Markdown));
    // Dispatch the parse while its worker is held behind the pool barrier.
    QTest::qWait(1);
    QCOMPARE(model.rowCount(), 0);
    if (newerSnapshot) {
        source += QStringLiteral(" more");
        QVERIFY(model.reconcileSource(source, MarkupDocumentModel::SourceFormat::Markdown));
    }
    QSignalSpy reconciledSpy(&model, &MarkupDocumentModel::documentReconciled);

    model.prepareForLayout();

    QCOMPARE(model.rowCount(), 3);
    QCOMPARE(reconciledSpy.count(), 1);
    releaseWorker.release();
    workerReleased = true;
    pool->waitForDone();
    QTest::qWait(1);
    QCOMPARE(model.data(model.index(2), MarkupDocumentModel::PlainTextRole).toString(),
             newerSnapshot ? QStringLiteral("After more") : QStringLiteral("After"));
    QCOMPARE(reconciledSpy.count(), 1);
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

void
MarkupDocumentModelTest::groupsAdjacentProseForRendering()
{
    enum Role
    {
        BlockIdRole = Qt::UserRole + 1,
        CodeBlockRole,
        BlockTextRole,
        LanguageRole,
        MarkdownRole,
    };
    QStandardItemModel source;
    source.setItemRoleNames({
      { BlockIdRole, "blockId" },
      { CodeBlockRole, "codeBlock" },
      { BlockTextRole, "blockText" },
      { LanguageRole, "language" },
      { MarkdownRole, "markdown" },
    });
    const auto appendRow =
      [&source](const QString& id, bool codeBlock, const QString& text, const QString& language, bool markdown) {
          auto* item = new QStandardItem;
          item->setData(id, BlockIdRole);
          item->setData(codeBlock, CodeBlockRole);
          item->setData(text, BlockTextRole);
          item->setData(language, LanguageRole);
          item->setData(markdown, MarkdownRole);
          source.appendRow(item);
      };
    appendRow(QStringLiteral("prose:0"), false, QStringLiteral("One"), {}, true);
    appendRow(QStringLiteral("prose:5"), false, QStringLiteral("Two"), {}, true);
    appendRow(QStringLiteral("code:10"), true, QStringLiteral("answer()"), QStringLiteral("cpp"), false);
    appendRow(QStringLiteral("prose:20"), false, QStringLiteral("Three"), {}, true);

    MarkupRenderModel renderModel;
    renderModel.setSourceModel(&source);

    QCOMPARE(renderModel.rowCount(), 3);
    QCOMPARE(renderModel.data(renderModel.index(0), MarkupRenderModel::SegmentTextRole).toString(),
             QStringLiteral("One\n\nTwo"));
    QVERIFY(renderModel.data(renderModel.index(1), MarkupRenderModel::CodeBlockRole).toBool());
    QCOMPARE(renderModel.data(renderModel.index(1), MarkupRenderModel::LanguageRole).toString(), QStringLiteral("cpp"));
    QCOMPARE(renderModel.data(renderModel.index(2), MarkupRenderModel::SegmentTextRole).toString(),
             QStringLiteral("Three"));
}

void
MarkupDocumentModelTest::removesOneTerminalCodeLineEndingForRendering()
{
    enum Role
    {
        BlockIdRole = Qt::UserRole + 1,
        CodeBlockRole,
        BlockTextRole,
        LanguageRole,
        MarkdownRole,
    };
    QStandardItemModel source;
    source.setItemRoleNames({
      { BlockIdRole, "blockId" },
      { CodeBlockRole, "codeBlock" },
      { BlockTextRole, "blockText" },
      { LanguageRole, "language" },
      { MarkdownRole, "markdown" },
    });
    const auto appendCode = [&source](const QString& id, const QString& text) {
        auto* item = new QStandardItem;
        item->setData(id, BlockIdRole);
        item->setData(true, CodeBlockRole);
        item->setData(text, BlockTextRole);
        item->setData(QStringLiteral("text"), LanguageRole);
        item->setData(false, MarkdownRole);
        source.appendRow(item);
    };
    appendCode(QStringLiteral("code:0"), QStringLiteral("answer()\n"));
    appendCode(QStringLiteral("code:10"), QStringLiteral("answer()\n\n"));
    appendCode(QStringLiteral("code:20"), QStringLiteral("\nanswer()"));

    MarkupRenderModel renderModel;
    renderModel.setSourceModel(&source);

    QCOMPARE(renderModel.data(renderModel.index(0), MarkupRenderModel::SegmentTextRole).toString(),
             QStringLiteral("answer()"));
    QCOMPARE(renderModel.data(renderModel.index(1), MarkupRenderModel::SegmentTextRole).toString(),
             QStringLiteral("answer()\n"));
    QCOMPARE(renderModel.data(renderModel.index(2), MarkupRenderModel::SegmentTextRole).toString(),
             QStringLiteral("\nanswer()"));
}

QTEST_GUILESS_MAIN(MarkupDocumentModelTest)

#include "markupdocumentmodeltest.moc"
