// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codextimelineviewportmodel.h"
#include "ward/markup/markupdocumentmodel.h"

#include <QAbstractItemModelTester>
#include <QMetaMethod>
#include <QSignalSpy>
#include <QStandardItem>
#include <QStandardItemModel>
#include <QtTest/QTest>

#include <utility>

namespace {
enum SourceRole
{
    EntryIdRole = Qt::UserRole + 1,
    TurnIdRole,
    MarkupDocumentRole,
    ActivityLabelRole,
    DetailRowRole,
    TurnExpandedRole,
};

enum BlockRole
{
    BlockIdRole = Qt::UserRole + 1,
    CodeBlockRole,
    BlockTextRole,
    PlainTextRole,
    LanguageRole,
    MarkdownRole,
};

constexpr int SegmentIdRole = Qt::UserRole + 101;
constexpr int SegmentCodeBlockRole = Qt::UserRole + 102;
constexpr int SegmentTextRole = Qt::UserRole + 103;
constexpr int SegmentLanguageRole = Qt::UserRole + 104;
constexpr int SegmentMarkdownRole = Qt::UserRole + 105;

void
configureSourceRoles(QStandardItemModel& model)
{
    model.setItemRoleNames({
      { EntryIdRole, "entryId" },
      { TurnIdRole, "turnId" },
      { MarkupDocumentRole, "markupDocument" },
      { ActivityLabelRole, "activityLabel" },
      { DetailRowRole, "detailRow" },
      { TurnExpandedRole, "turnExpanded" },
    });
}

void
configureBlockRoles(QStandardItemModel& model)
{
    model.setItemRoleNames({
      { BlockIdRole, "blockId" },
      { CodeBlockRole, "codeBlock" },
      { BlockTextRole, "blockText" },
      { PlainTextRole, "plainText" },
      { LanguageRole, "language" },
      { MarkdownRole, "markdown" },
    });
}

void
configureRenderRoles(QStandardItemModel& model)
{
    model.setItemRoleNames({
      { SegmentIdRole, "segmentId" },
      { SegmentCodeBlockRole, "codeBlock" },
      { SegmentTextRole, "segmentText" },
      { SegmentLanguageRole, "language" },
      { SegmentMarkdownRole, "markdown" },
    });
}

void
appendBlock(QStandardItemModel& model,
            const QString& blockId,
            const QString& text,
            bool codeBlock = false,
            const QString& language = {})
{
    auto* item = new QStandardItem;
    item->setData(blockId, BlockIdRole);
    item->setData(codeBlock, CodeBlockRole);
    item->setData(text, BlockTextRole);
    item->setData(text, PlainTextRole);
    item->setData(language, LanguageRole);
    item->setData(!codeBlock, MarkdownRole);
    model.appendRow(item);
}

void
appendSegment(QStandardItemModel& model,
              const QString& segmentId,
              const QString& text,
              bool codeBlock = false,
              const QString& language = {})
{
    auto* item = new QStandardItem;
    item->setData(segmentId, SegmentIdRole);
    item->setData(codeBlock, SegmentCodeBlockRole);
    item->setData(text, SegmentTextRole);
    item->setData(language, SegmentLanguageRole);
    item->setData(!codeBlock, SegmentMarkdownRole);
    model.appendRow(item);
}

class InspectableSourceModel : public QStandardItemModel
{
  public:
    using QStandardItemModel::QStandardItemModel;

    QVariant data(const QModelIndex& index, int role) const override
    {
        ++dataReadCount_;
        return QStandardItemModel::data(index, role);
    }

    void notifyAllRolesChanged(int row) { emit dataChanged(index(row, 0), index(row, 0)); }

    void resetDataReadCount() const { dataReadCount_ = 0; }

    [[nodiscard]] int dataReadCount() const { return dataReadCount_; }

  private:
    mutable int dataReadCount_ = 0;
};

class ConnectionCountingBlockModel : public QStandardItemModel
{
  public:
    using QStandardItemModel::QStandardItemModel;

    [[nodiscard]] int connectionNotificationCount() const { return connectionNotificationCount_; }
    [[nodiscard]] int disconnectionNotificationCount() const { return disconnectionNotificationCount_; }

  protected:
    void connectNotify(const QMetaMethod& signal) override
    {
        ++connectionNotificationCount_;
        QStandardItemModel::connectNotify(signal);
    }

    void disconnectNotify(const QMetaMethod& signal) override
    {
        ++disconnectionNotificationCount_;
        QStandardItemModel::disconnectNotify(signal);
    }

  private:
    int connectionNotificationCount_ = 0;
    int disconnectionNotificationCount_ = 0;
};

class ResettableRenderModel : public QAbstractListModel
{
  public:
    struct Segment
    {
        QString id;
        QString text;
        bool codeBlock = false;
        QString language;
    };

    [[nodiscard]] int rowCount(const QModelIndex& parent = {}) const override
    {
        return parent.isValid() ? 0 : segments_.size();
    }

    [[nodiscard]] QVariant data(const QModelIndex& index, int role) const override
    {
        if (!index.isValid() || index.row() < 0 || index.row() >= segments_.size())
            return {};
        const Segment& segment = segments_.at(index.row());
        switch (role) {
            case SegmentIdRole:
                return segment.id;
            case SegmentCodeBlockRole:
                return segment.codeBlock;
            case SegmentTextRole:
                return segment.text;
            case SegmentLanguageRole:
                return segment.language;
            case SegmentMarkdownRole:
                return !segment.codeBlock;
            default:
                return {};
        }
    }

    [[nodiscard]] QHash<int, QByteArray> roleNames() const override
    {
        return {
            { SegmentIdRole, "segmentId" },      { SegmentCodeBlockRole, "codeBlock" },
            { SegmentTextRole, "segmentText" },  { SegmentLanguageRole, "language" },
            { SegmentMarkdownRole, "markdown" },
        };
    }

    void replaceSegments(QList<Segment> segments)
    {
        beginResetModel();
        segments_ = std::move(segments);
        endResetModel();
    }

  private:
    QList<Segment> segments_;
};
}

class CodexTimelineViewportModelTest : public QObject
{
    Q_OBJECT

  private slots:
    void expandsOneMessageIntoStableSemanticBlockRows();
    void expandsAndCollapsesOneDetailWithoutResettingViewport();
    void replacesOneDocumentWithoutReconnectingUnchangedBlockModels();
    void retainsOneSharedBlockModelSubscriptionForRemainingRows();
    void usesDocumentRenderSegmentsAsViewportRows();
    void reconcilesOneRenderResetWithoutResettingViewport();
    void integratesOneRealMarkupDocumentWithoutResettingViewport();
    void updatesOneMutableBlockWithoutResettingStableRows();
    void insertsOneCompletedBlockWithoutResettingStableRows();
    void forwardsOneSourceUpdateWithoutResettingRows();
    void appendsOneSourceMessageWithoutResettingExistingRows();
    void promotesOnePendingMessageToItsFirstBlockWithoutChangingIdentity();
    void removesOneTrailingBlockWithoutResettingStableRows();
    void removesOneSourceMessageWithoutResettingOtherRows();
    void forwardsOneAllRoleUpdateWithoutReadingUnchangedHistory();
};

void
CodexTimelineViewportModelTest::expandsOneMessageIntoStableSemanticBlockRows()
{
    QStandardItemModel document;
    configureBlockRoles(document);
    appendBlock(document, QStringLiteral("prose:0"), QStringLiteral("Before"));
    appendBlock(document, QStringLiteral("code:8"), QStringLiteral("int main();"), true, QStringLiteral("cpp"));
    appendBlock(document, QStringLiteral("prose:32"), QStringLiteral("After"));

    QStandardItemModel source;
    configureSourceRoles(source);
    auto* message = new QStandardItem;
    message->setData(QStringLiteral("message:turn-1:answer"), EntryIdRole);
    message->setData(QStringLiteral("turn-1"), TurnIdRole);
    message->setData(QVariant::fromValue(static_cast<QObject*>(&document)), MarkupDocumentRole);
    source.appendRow(message);

    CodexTimelineViewportModel model;
    model.setSourceModel(&source);

    QCOMPARE(model.rowCount(), 3);
    QCOMPARE(model.entryIdAt(0), QStringLiteral("message:turn-1:answer"));
    QCOMPARE(model.entryIdAt(1), QStringLiteral("message:turn-1:answer/markup/code:8"));
    QCOMPARE(model.entryIdAt(2), QStringLiteral("message:turn-1:answer/markup/prose:32"));
    QCOMPARE(model.indexOfEntryId(QStringLiteral("message:turn-1:answer/markup/code:8")), 1);
    QCOMPARE(model.valueAt(1, QStringLiteral("turnId")).toString(), QStringLiteral("turn-1"));
    QVERIFY(model.valueAt(1, QStringLiteral("semanticBlock")).toBool());
    QVERIFY(model.valueAt(1, QStringLiteral("codeBlock")).toBool());
    QCOMPARE(model.valueAt(1, QStringLiteral("blockText")).toString(), QStringLiteral("int main();"));
    QCOMPARE(model.valueAt(1, QStringLiteral("language")).toString(), QStringLiteral("cpp"));
    QVERIFY(model.valueAt(0, QStringLiteral("firstBlockInEntry")).toBool());
    QVERIFY(model.valueAt(2, QStringLiteral("lastBlockInEntry")).toBool());
}

void
CodexTimelineViewportModelTest::expandsAndCollapsesOneDetailWithoutResettingViewport()
{
    QStandardItemModel document;
    configureBlockRoles(document);
    appendBlock(document, QStringLiteral("prose:0"), QStringLiteral("Before"));
    appendBlock(document, QStringLiteral("code:8"), QStringLiteral("return 0;"), true, QStringLiteral("cpp"));
    appendBlock(document, QStringLiteral("prose:24"), QStringLiteral("After"));

    QStandardItemModel source;
    configureSourceRoles(source);
    auto* detail = new QStandardItem;
    detail->setData(QStringLiteral("commentary:turn-1:item-1"), EntryIdRole);
    detail->setData(QStringLiteral("turn-1"), TurnIdRole);
    detail->setData(QVariant::fromValue(static_cast<QObject*>(&document)), MarkupDocumentRole);
    detail->setData(true, DetailRowRole);
    detail->setData(false, TurnExpandedRole);
    source.appendRow(detail);

    CodexTimelineViewportModel model;
    model.setSourceModel(&source);
    QAbstractItemModelTester modelTester(&model, QAbstractItemModelTester::FailureReportingMode::QtTest);
    const QString stableEntryId = model.entryIdAt(0);
    QSignalSpy resetSpy(&model, &QAbstractItemModel::modelReset);
    QSignalSpy insertedSpy(&model, &QAbstractItemModel::rowsInserted);
    QSignalSpy removedSpy(&model, &QAbstractItemModel::rowsRemoved);

    QCOMPARE(model.rowCount(), 1);
    QVERIFY(!model.valueAt(0, QStringLiteral("semanticBlock")).toBool());

    detail->setData(true, TurnExpandedRole);

    QCOMPARE(resetSpy.size(), 0);
    QCOMPARE(insertedSpy.size(), 1);
    QCOMPARE(model.rowCount(), 3);
    QCOMPARE(model.entryIdAt(0), stableEntryId);
    QVERIFY(model.valueAt(0, QStringLiteral("semanticBlock")).toBool());
    QVERIFY(model.valueAt(2, QStringLiteral("lastBlockInEntry")).toBool());

    detail->setData(false, TurnExpandedRole);

    QCOMPARE(resetSpy.size(), 0);
    QCOMPARE(removedSpy.size(), 1);
    QCOMPARE(model.rowCount(), 1);
    QCOMPARE(model.entryIdAt(0), stableEntryId);
    QVERIFY(!model.valueAt(0, QStringLiteral("semanticBlock")).toBool());
}

void
CodexTimelineViewportModelTest::replacesOneDocumentWithoutReconnectingUnchangedBlockModels()
{
    ConnectionCountingBlockModel firstDocument;
    configureBlockRoles(firstDocument);
    appendBlock(firstDocument, QStringLiteral("prose:0"), QStringLiteral("First"));
    ConnectionCountingBlockModel secondDocument;
    configureBlockRoles(secondDocument);
    appendBlock(secondDocument, QStringLiteral("prose:0"), QStringLiteral("Second"));
    ConnectionCountingBlockModel replacementDocument;
    configureBlockRoles(replacementDocument);
    appendBlock(replacementDocument, QStringLiteral("prose:0"), QStringLiteral("Replacement"));

    QStandardItemModel source;
    configureSourceRoles(source);
    auto* firstMessage = new QStandardItem;
    firstMessage->setData(QStringLiteral("message:turn-1:answer"), EntryIdRole);
    firstMessage->setData(QVariant::fromValue(static_cast<QObject*>(&firstDocument)), MarkupDocumentRole);
    source.appendRow(firstMessage);
    auto* secondMessage = new QStandardItem;
    secondMessage->setData(QStringLiteral("message:turn-2:answer"), EntryIdRole);
    secondMessage->setData(QVariant::fromValue(static_cast<QObject*>(&secondDocument)), MarkupDocumentRole);
    source.appendRow(secondMessage);

    CodexTimelineViewportModel model;
    model.setSourceModel(&source);
    const int retainedConnectionNotifications = firstDocument.connectionNotificationCount();
    const int retainedDisconnectionNotifications = firstDocument.disconnectionNotificationCount();
    const int replacedDisconnectionNotifications = secondDocument.disconnectionNotificationCount();

    secondMessage->setData(QVariant::fromValue(static_cast<QObject*>(&replacementDocument)), MarkupDocumentRole);

    QCOMPARE(model.valueAt(1, QStringLiteral("blockText")).toString(), QStringLiteral("Replacement"));
    QCOMPARE(firstDocument.connectionNotificationCount(), retainedConnectionNotifications);
    QCOMPARE(firstDocument.disconnectionNotificationCount(), retainedDisconnectionNotifications);
    QVERIFY(secondDocument.disconnectionNotificationCount() > replacedDisconnectionNotifications);
    QVERIFY(replacementDocument.connectionNotificationCount() > 0);

    QSignalSpy changedSpy(&model, &QAbstractItemModel::dataChanged);
    replacementDocument.item(0)->setData(QStringLiteral("Updated replacement"), BlockTextRole);

    QCOMPARE(changedSpy.size(), 1);
    QCOMPARE(model.valueAt(1, QStringLiteral("blockText")).toString(), QStringLiteral("Updated replacement"));

    changedSpy.clear();
    secondDocument.item(0)->setData(QStringLiteral("Detached document"), BlockTextRole);

    QCOMPARE(changedSpy.size(), 0);
}

void
CodexTimelineViewportModelTest::retainsOneSharedBlockModelSubscriptionForRemainingRows()
{
    ConnectionCountingBlockModel sharedDocument;
    configureBlockRoles(sharedDocument);
    appendBlock(sharedDocument, QStringLiteral("prose:0"), QStringLiteral("Shared"));
    ConnectionCountingBlockModel replacementDocument;
    configureBlockRoles(replacementDocument);
    appendBlock(replacementDocument, QStringLiteral("prose:0"), QStringLiteral("Replacement"));

    QStandardItemModel source;
    configureSourceRoles(source);
    auto* firstMessage = new QStandardItem;
    firstMessage->setData(QStringLiteral("message:turn-1:answer"), EntryIdRole);
    firstMessage->setData(QVariant::fromValue(static_cast<QObject*>(&sharedDocument)), MarkupDocumentRole);
    source.appendRow(firstMessage);
    auto* secondMessage = new QStandardItem;
    secondMessage->setData(QStringLiteral("message:turn-2:answer"), EntryIdRole);
    secondMessage->setData(QVariant::fromValue(static_cast<QObject*>(&sharedDocument)), MarkupDocumentRole);
    source.appendRow(secondMessage);

    CodexTimelineViewportModel model;
    model.setSourceModel(&source);
    const int retainedConnectionNotifications = sharedDocument.connectionNotificationCount();
    const int retainedDisconnectionNotifications = sharedDocument.disconnectionNotificationCount();

    secondMessage->setData(QVariant::fromValue(static_cast<QObject*>(&replacementDocument)), MarkupDocumentRole);

    QCOMPARE(sharedDocument.connectionNotificationCount(), retainedConnectionNotifications);
    QCOMPARE(sharedDocument.disconnectionNotificationCount(), retainedDisconnectionNotifications);

    QSignalSpy changedSpy(&model, &QAbstractItemModel::dataChanged);
    sharedDocument.item(0)->setData(QStringLiteral("Updated shared"), BlockTextRole);

    QCOMPARE(changedSpy.size(), 1);
    const QList<QVariant> arguments = changedSpy.takeFirst();
    QCOMPARE(qvariant_cast<QModelIndex>(arguments.at(0)).row(), 0);
    QCOMPARE(qvariant_cast<QModelIndex>(arguments.at(1)).row(), 0);
    QCOMPARE(model.valueAt(0, QStringLiteral("blockText")).toString(), QStringLiteral("Updated shared"));
    QCOMPARE(model.valueAt(1, QStringLiteral("blockText")).toString(), QStringLiteral("Replacement"));
}

void
CodexTimelineViewportModelTest::usesDocumentRenderSegmentsAsViewportRows()
{
    QStandardItemModel renderModel;
    configureRenderRoles(renderModel);
    appendSegment(renderModel, QStringLiteral("prose:0"), QStringLiteral("First\n\nSecond"));
    appendSegment(renderModel, QStringLiteral("code:24"), QStringLiteral("return 0;"), true, QStringLiteral("cpp"));
    QObject document;
    document.setProperty("renderModel", QVariant::fromValue(static_cast<QAbstractItemModel*>(&renderModel)));

    QStandardItemModel source;
    configureSourceRoles(source);
    auto* message = new QStandardItem;
    message->setData(QStringLiteral("message:turn-1:answer"), EntryIdRole);
    message->setData(QVariant::fromValue(&document), MarkupDocumentRole);
    source.appendRow(message);

    CodexTimelineViewportModel model;
    model.setSourceModel(&source);

    QCOMPARE(model.rowCount(), 2);
    QCOMPARE(model.entryIdAt(0), QStringLiteral("message:turn-1:answer"));
    QCOMPARE(model.entryIdAt(1), QStringLiteral("message:turn-1:answer/markup/code:24"));
    QCOMPARE(model.valueAt(0, QStringLiteral("blockText")).toString(), QStringLiteral("First\n\nSecond"));
    QCOMPARE(model.valueAt(1, QStringLiteral("blockText")).toString(), QStringLiteral("return 0;"));
    QVERIFY(model.valueAt(1, QStringLiteral("codeBlock")).toBool());
}

void
CodexTimelineViewportModelTest::reconcilesOneRenderResetWithoutResettingViewport()
{
    ResettableRenderModel renderModel;
    renderModel.replaceSegments({
      { .id = QStringLiteral("prose:0"), .text = QStringLiteral("Growing answer") },
      { .id = QStringLiteral("code:24"), .text = QStringLiteral("return 0;"), .codeBlock = true },
    });
    QObject document;
    document.setProperty("renderModel", QVariant::fromValue(static_cast<QAbstractItemModel*>(&renderModel)));

    QStandardItemModel source;
    configureSourceRoles(source);
    auto* message = new QStandardItem;
    message->setData(QStringLiteral("message:turn-1:answer"), EntryIdRole);
    message->setData(QVariant::fromValue(&document), MarkupDocumentRole);
    source.appendRow(message);

    CodexTimelineViewportModel model;
    model.setSourceModel(&source);
    const QString stableProseId = model.entryIdAt(0);
    const QString stableCodeId = model.entryIdAt(1);
    QSignalSpy resetSpy(&model, &QAbstractItemModel::modelReset);
    QSignalSpy insertedSpy(&model, &QAbstractItemModel::rowsInserted);
    QSignalSpy removedSpy(&model, &QAbstractItemModel::rowsRemoved);
    QSignalSpy changedSpy(&model, &QAbstractItemModel::dataChanged);

    renderModel.replaceSegments({
      { .id = QStringLiteral("prose:0"), .text = QStringLiteral("Growing answer with more content") },
      { .id = QStringLiteral("code:24"), .text = QStringLiteral("return 0;"), .codeBlock = true },
    });

    QCOMPARE(resetSpy.size(), 0);
    QCOMPARE(insertedSpy.size(), 0);
    QCOMPARE(removedSpy.size(), 0);
    QCOMPARE(changedSpy.size(), 1);
    QCOMPARE(model.entryIdAt(0), stableProseId);
    QCOMPARE(model.entryIdAt(1), stableCodeId);
    QCOMPARE(model.valueAt(0, QStringLiteral("blockText")).toString(),
             QStringLiteral("Growing answer with more content"));
}

void
CodexTimelineViewportModelTest::integratesOneRealMarkupDocumentWithoutResettingViewport()
{
    MarkupDocumentModel document;
    QVERIFY(document.reconcileSource(QStringLiteral("Before\n\n```cpp\nreturn 0;\n```\n\nAfter"),
                                     MarkupDocumentModel::SourceFormat::Markdown));

    QStandardItemModel source;
    configureSourceRoles(source);
    auto* message = new QStandardItem;
    message->setData(QStringLiteral("message:turn-1:answer"), EntryIdRole);
    message->setData(QVariant::fromValue(static_cast<QObject*>(&document)), MarkupDocumentRole);
    source.appendRow(message);

    CodexTimelineViewportModel model;
    model.setSourceModel(&source);
    QAbstractItemModelTester modelTester(&model, QAbstractItemModelTester::FailureReportingMode::QtTest);
    QSignalSpy resetSpy(&model, &QAbstractItemModel::modelReset);

    QTRY_COMPARE_WITH_TIMEOUT(model.rowCount(), 3, 5000);
    QCOMPARE(resetSpy.size(), 0);
    QCOMPARE(model.entryIdAt(0), QStringLiteral("message:turn-1:answer"));
    QVERIFY(model.valueAt(1, QStringLiteral("codeBlock")).toBool());
    QCOMPARE(model.valueAt(1, QStringLiteral("blockText")).toString(), QStringLiteral("return 0;"));
    QVERIFY(model.valueAt(2, QStringLiteral("lastBlockInEntry")).toBool());
}

void
CodexTimelineViewportModelTest::updatesOneMutableBlockWithoutResettingStableRows()
{
    QStandardItemModel document;
    configureBlockRoles(document);
    appendBlock(document, QStringLiteral("prose:0"), QStringLiteral("Stable prefix"));
    appendBlock(document, QStringLiteral("prose:16"), QStringLiteral("Growing tail"));

    QStandardItemModel source;
    configureSourceRoles(source);
    auto* message = new QStandardItem;
    message->setData(QStringLiteral("message:turn-1:answer"), EntryIdRole);
    message->setData(QStringLiteral("turn-1"), TurnIdRole);
    message->setData(QVariant::fromValue(static_cast<QObject*>(&document)), MarkupDocumentRole);
    source.appendRow(message);

    CodexTimelineViewportModel model;
    model.setSourceModel(&source);
    const QString stablePrefixId = model.entryIdAt(0);
    const QString stableTailId = model.entryIdAt(1);
    QSignalSpy resetSpy(&model, &QAbstractItemModel::modelReset);
    QSignalSpy changedSpy(&model, &QAbstractItemModel::dataChanged);

    document.item(1)->setData(QStringLiteral("Growing tail with more content"), BlockTextRole);

    QCOMPARE(resetSpy.size(), 0);
    QCOMPARE(changedSpy.size(), 1);
    QCOMPARE(model.entryIdAt(0), stablePrefixId);
    QCOMPARE(model.entryIdAt(1), stableTailId);
    QCOMPARE(model.valueAt(1, QStringLiteral("blockText")).toString(),
             QStringLiteral("Growing tail with more content"));
}

void
CodexTimelineViewportModelTest::insertsOneCompletedBlockWithoutResettingStableRows()
{
    QStandardItemModel document;
    configureBlockRoles(document);
    appendBlock(document, QStringLiteral("prose:0"), QStringLiteral("Stable prefix"));
    appendBlock(document, QStringLiteral("prose:16"), QStringLiteral("Existing tail"));

    QStandardItemModel source;
    configureSourceRoles(source);
    auto* message = new QStandardItem;
    message->setData(QStringLiteral("message:turn-1:answer"), EntryIdRole);
    message->setData(QStringLiteral("turn-1"), TurnIdRole);
    message->setData(QVariant::fromValue(static_cast<QObject*>(&document)), MarkupDocumentRole);
    source.appendRow(message);

    CodexTimelineViewportModel model;
    model.setSourceModel(&source);
    const QString stablePrefixId = model.entryIdAt(0);
    const QString stableTailId = model.entryIdAt(1);
    QSignalSpy resetSpy(&model, &QAbstractItemModel::modelReset);
    QSignalSpy insertedSpy(&model, &QAbstractItemModel::rowsInserted);

    appendBlock(document, QStringLiteral("code:32"), QStringLiteral("return 0;"), true, QStringLiteral("cpp"));

    QCOMPARE(resetSpy.size(), 0);
    QCOMPARE(insertedSpy.size(), 1);
    QCOMPARE(model.rowCount(), 3);
    QCOMPARE(model.entryIdAt(0), stablePrefixId);
    QCOMPARE(model.entryIdAt(1), stableTailId);
    QCOMPARE(model.entryIdAt(2), QStringLiteral("message:turn-1:answer/markup/code:32"));
    QCOMPARE(model.indexOfEntryId(stableTailId), 1);
    QCOMPARE(model.indexOfEntryId(QStringLiteral("message:turn-1:answer/markup/code:32")), 2);
    QVERIFY(model.valueAt(2, QStringLiteral("lastBlockInEntry")).toBool());
}

void
CodexTimelineViewportModelTest::forwardsOneSourceUpdateWithoutResettingRows()
{
    QStandardItemModel source;
    configureSourceRoles(source);
    auto* activity = new QStandardItem;
    activity->setData(QStringLiteral("activity:turn-1:command"), EntryIdRole);
    activity->setData(QStringLiteral("turn-1"), TurnIdRole);
    activity->setData(QStringLiteral("Running command"), ActivityLabelRole);
    source.appendRow(activity);

    CodexTimelineViewportModel model;
    model.setSourceModel(&source);
    const QString stableEntryId = model.entryIdAt(0);
    QSignalSpy resetSpy(&model, &QAbstractItemModel::modelReset);
    QSignalSpy changedSpy(&model, &QAbstractItemModel::dataChanged);

    activity->setData(QStringLiteral("Completed command"), ActivityLabelRole);

    QCOMPARE(resetSpy.size(), 0);
    QCOMPARE(changedSpy.size(), 1);
    QCOMPARE(model.entryIdAt(0), stableEntryId);
    QCOMPARE(model.valueAt(0, QStringLiteral("activityLabel")).toString(), QStringLiteral("Completed command"));
    QVERIFY(!model.valueAt(0, QStringLiteral("semanticBlock")).toBool());
}

void
CodexTimelineViewportModelTest::appendsOneSourceMessageWithoutResettingExistingRows()
{
    QStandardItemModel source;
    configureSourceRoles(source);
    auto* activity = new QStandardItem;
    activity->setData(QStringLiteral("activity:turn-1:command"), EntryIdRole);
    activity->setData(QStringLiteral("turn-1"), TurnIdRole);
    source.appendRow(activity);

    CodexTimelineViewportModel model;
    model.setSourceModel(&source);
    const QString stableActivityId = model.entryIdAt(0);
    QSignalSpy resetSpy(&model, &QAbstractItemModel::modelReset);
    QSignalSpy insertedSpy(&model, &QAbstractItemModel::rowsInserted);

    QStandardItemModel document;
    configureBlockRoles(document);
    appendBlock(document, QStringLiteral("prose:0"), QStringLiteral("Answer"));
    appendBlock(document, QStringLiteral("code:8"), QStringLiteral("return 0;"), true, QStringLiteral("cpp"));
    auto* message = new QStandardItem;
    message->setData(QStringLiteral("message:turn-2:answer"), EntryIdRole);
    message->setData(QStringLiteral("turn-2"), TurnIdRole);
    message->setData(QVariant::fromValue(static_cast<QObject*>(&document)), MarkupDocumentRole);
    source.appendRow(message);

    QCOMPARE(resetSpy.size(), 0);
    QCOMPARE(insertedSpy.size(), 1);
    QCOMPARE(model.rowCount(), 3);
    QCOMPARE(model.entryIdAt(0), stableActivityId);
    QCOMPARE(model.entryIdAt(1), QStringLiteral("message:turn-2:answer"));
    QCOMPARE(model.entryIdAt(2), QStringLiteral("message:turn-2:answer/markup/code:8"));
    QCOMPARE(model.indexOfEntryId(stableActivityId), 0);
    QCOMPARE(model.indexOfEntryId(QStringLiteral("message:turn-2:answer/markup/code:8")), 2);
}

void
CodexTimelineViewportModelTest::promotesOnePendingMessageToItsFirstBlockWithoutChangingIdentity()
{
    QStandardItemModel document;
    configureBlockRoles(document);

    QStandardItemModel source;
    configureSourceRoles(source);
    auto* message = new QStandardItem;
    message->setData(QStringLiteral("message:turn-1:answer"), EntryIdRole);
    message->setData(QStringLiteral("turn-1"), TurnIdRole);
    message->setData(QVariant::fromValue(static_cast<QObject*>(&document)), MarkupDocumentRole);
    source.appendRow(message);

    CodexTimelineViewportModel model;
    model.setSourceModel(&source);
    QCOMPARE(model.rowCount(), 1);
    const QString pendingEntryId = model.entryIdAt(0);
    QSignalSpy resetSpy(&model, &QAbstractItemModel::modelReset);
    QSignalSpy insertedSpy(&model, &QAbstractItemModel::rowsInserted);
    QSignalSpy changedSpy(&model, &QAbstractItemModel::dataChanged);

    appendBlock(document, QStringLiteral("prose:0"), QStringLiteral("Parsed answer"));

    QCOMPARE(resetSpy.size(), 0);
    QCOMPARE(insertedSpy.size(), 0);
    QCOMPARE(changedSpy.size(), 1);
    QCOMPARE(model.rowCount(), 1);
    QCOMPARE(model.entryIdAt(0), pendingEntryId);
    QVERIFY(model.valueAt(0, QStringLiteral("semanticBlock")).toBool());
    QCOMPARE(model.valueAt(0, QStringLiteral("blockText")).toString(), QStringLiteral("Parsed answer"));
}

void
CodexTimelineViewportModelTest::removesOneTrailingBlockWithoutResettingStableRows()
{
    QStandardItemModel document;
    configureBlockRoles(document);
    appendBlock(document, QStringLiteral("prose:0"), QStringLiteral("Stable prefix"));
    appendBlock(document, QStringLiteral("code:16"), QStringLiteral("return 0;"), true, QStringLiteral("cpp"));
    appendBlock(document, QStringLiteral("prose:32"), QStringLiteral("Mutable tail"));

    QStandardItemModel source;
    configureSourceRoles(source);
    auto* message = new QStandardItem;
    message->setData(QStringLiteral("message:turn-1:answer"), EntryIdRole);
    message->setData(QVariant::fromValue(static_cast<QObject*>(&document)), MarkupDocumentRole);
    source.appendRow(message);

    CodexTimelineViewportModel model;
    model.setSourceModel(&source);
    const QString stablePrefixId = model.entryIdAt(0);
    const QString stableCodeId = model.entryIdAt(1);
    QSignalSpy resetSpy(&model, &QAbstractItemModel::modelReset);
    QSignalSpy removedSpy(&model, &QAbstractItemModel::rowsRemoved);

    document.removeRow(2);

    QCOMPARE(resetSpy.size(), 0);
    QCOMPARE(removedSpy.size(), 1);
    QCOMPARE(model.rowCount(), 2);
    QCOMPARE(model.entryIdAt(0), stablePrefixId);
    QCOMPARE(model.entryIdAt(1), stableCodeId);
    QCOMPARE(model.indexOfEntryId(stableCodeId), 1);
    QVERIFY(model.valueAt(1, QStringLiteral("lastBlockInEntry")).toBool());
}

void
CodexTimelineViewportModelTest::removesOneSourceMessageWithoutResettingOtherRows()
{
    QStandardItemModel document;
    configureBlockRoles(document);
    appendBlock(document, QStringLiteral("prose:0"), QStringLiteral("Answer"));
    appendBlock(document, QStringLiteral("code:8"), QStringLiteral("return 0;"), true, QStringLiteral("cpp"));

    QStandardItemModel source;
    configureSourceRoles(source);
    auto* firstActivity = new QStandardItem;
    firstActivity->setData(QStringLiteral("activity:turn-1:command"), EntryIdRole);
    firstActivity->setData(QStringLiteral("turn-1"), TurnIdRole);
    source.appendRow(firstActivity);
    auto* message = new QStandardItem;
    message->setData(QStringLiteral("message:turn-2:answer"), EntryIdRole);
    message->setData(QStringLiteral("turn-2"), TurnIdRole);
    message->setData(QVariant::fromValue(static_cast<QObject*>(&document)), MarkupDocumentRole);
    source.appendRow(message);
    auto* lastActivity = new QStandardItem;
    lastActivity->setData(QStringLiteral("activity:turn-3:command"), EntryIdRole);
    lastActivity->setData(QStringLiteral("turn-3"), TurnIdRole);
    source.appendRow(lastActivity);

    CodexTimelineViewportModel model;
    model.setSourceModel(&source);
    const QString stableFirstId = model.entryIdAt(0);
    const QString stableLastId = model.entryIdAt(3);
    QSignalSpy resetSpy(&model, &QAbstractItemModel::modelReset);
    QSignalSpy removedSpy(&model, &QAbstractItemModel::rowsRemoved);

    source.removeRow(1);

    QCOMPARE(resetSpy.size(), 0);
    QCOMPARE(removedSpy.size(), 1);
    QCOMPARE(model.rowCount(), 2);
    QCOMPARE(model.entryIdAt(0), stableFirstId);
    QCOMPARE(model.entryIdAt(1), stableLastId);
    QCOMPARE(model.indexOfEntryId(stableLastId), 1);
    QCOMPARE(model.valueAt(1, QStringLiteral("turnId")).toString(), QStringLiteral("turn-3"));
}

void
CodexTimelineViewportModelTest::forwardsOneAllRoleUpdateWithoutReadingUnchangedHistory()
{
    InspectableSourceModel source;
    configureSourceRoles(source);
    constexpr int sourceRowCount = 128;
    for (int row = 0; row < sourceRowCount; ++row) {
        auto* activity = new QStandardItem;
        activity->setData(QStringLiteral("activity:turn-%1:command").arg(row), EntryIdRole);
        source.appendRow(activity);
    }

    CodexTimelineViewportModel model;
    model.setSourceModel(&source);
    QSignalSpy resetSpy(&model, &QAbstractItemModel::modelReset);
    QSignalSpy changedSpy(&model, &QAbstractItemModel::dataChanged);
    source.resetDataReadCount();

    source.notifyAllRolesChanged(sourceRowCount - 1);

    QCOMPARE(resetSpy.size(), 0);
    QCOMPARE(changedSpy.size(), 1);
    QVERIFY(source.dataReadCount() <= 2);
    const QList<QVariant> arguments = changedSpy.takeFirst();
    QCOMPARE(qvariant_cast<QModelIndex>(arguments.at(0)).row(), sourceRowCount - 1);
    QCOMPARE(qvariant_cast<QModelIndex>(arguments.at(1)).row(), sourceRowCount - 1);
}

QTEST_GUILESS_MAIN(CodexTimelineViewportModelTest)

#include "codextimelineviewportmodeltest.moc"
