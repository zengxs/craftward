// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codextimelineviewportmodel.h"

#include <QVariant>

#include <algorithm>
#include <functional>
#include <utility>

namespace {
constexpr auto entryIdRoleName = "entryId";
constexpr auto markupDocumentRoleName = "markupDocument";
constexpr auto blockIdRoleName = "blockId";
constexpr auto segmentIdRoleName = "segmentId";
constexpr auto codeBlockRoleName = "codeBlock";
constexpr auto blockTextRoleName = "blockText";
constexpr auto segmentTextRoleName = "segmentText";
constexpr auto plainTextRoleName = "plainText";
constexpr auto languageRoleName = "language";
constexpr auto markdownRoleName = "markdown";
constexpr auto sourceEntryIdRoleName = "sourceEntryId";
constexpr auto semanticBlockRoleName = "semanticBlock";
constexpr auto blockIndexRoleName = "blockIndex";
constexpr auto blockCountRoleName = "blockCount";
constexpr auto firstBlockInEntryRoleName = "firstBlockInEntry";
constexpr auto lastBlockInEntryRoleName = "lastBlockInEntry";

int
roleForName(const QAbstractItemModel* model, const QByteArray& roleName)
{
    return model ? model->roleNames().key(roleName, -1) : -1;
}

int
semanticIdRole(const QAbstractItemModel* model)
{
    const int blockIdRole = roleForName(model, blockIdRoleName);
    return blockIdRole >= 0 ? blockIdRole : roleForName(model, segmentIdRoleName);
}

int
semanticTextRole(const QAbstractItemModel* model, const QByteArray& roleName)
{
    const int directRole = roleForName(model, roleName);
    if (directRole >= 0)
        return directRole;
    if (roleName == blockTextRoleName)
        return roleForName(model, segmentTextRoleName);
    return -1;
}

QString
viewportEntryId(const QString& sourceEntryId, const QAbstractItemModel* blockModel, int blockRow, int blockIdRole)
{
    if (blockRow == 0)
        return sourceEntryId;

    const QString blockId = blockIdRole < 0 ? QStringLiteral("row:%1").arg(blockRow)
                                            : blockModel->data(blockModel->index(blockRow, 0), blockIdRole).toString();
    return QStringLiteral("%1/markup/%2").arg(sourceEntryId, blockId);
}
}

CodexTimelineViewportModel::CodexTimelineViewportModel(QObject* parent)
  : QAbstractListModel(parent)
{
}

QAbstractItemModel*
CodexTimelineViewportModel::sourceModel() const
{
    return sourceModel_;
}

void
CodexTimelineViewportModel::setSourceModel(QAbstractItemModel* model)
{
    if (sourceModel_ == model)
        return;

    disconnectModels();
    sourceModel_ = model;
    if (sourceModel_) {
        sourceConnections_.append(
          connect(sourceModel_, &QAbstractItemModel::modelReset, this, &CodexTimelineViewportModel::rebuild));
        sourceConnections_.append(connect(
          sourceModel_, &QAbstractItemModel::rowsInserted, this, &CodexTimelineViewportModel::insertSourceRows));
        sourceConnections_.append(
          connect(sourceModel_, &QAbstractItemModel::rowsRemoved, this, &CodexTimelineViewportModel::removeSourceRows));
        sourceConnections_.append(
          connect(sourceModel_, &QAbstractItemModel::rowsMoved, this, &CodexTimelineViewportModel::rebuild));
        sourceConnections_.append(
          connect(sourceModel_, &QAbstractItemModel::layoutChanged, this, &CodexTimelineViewportModel::rebuild));
        sourceConnections_.append(
          connect(sourceModel_,
                  &QAbstractItemModel::dataChanged,
                  this,
                  [this](const QModelIndex& topLeft, const QModelIndex& bottomRight, const QList<int>& roles) {
                      forwardSourceDataChanged(topLeft, bottomRight, roles);
                  }));
        sourceConnections_.append(connect(sourceModel_, &QObject::destroyed, this, [this] {
            sourceModel_ = nullptr;
            rebuild();
        }));
    }
    rebuild();
    emit sourceModelChanged();
}

int
CodexTimelineViewportModel::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : rows_.size();
}

int
CodexTimelineViewportModel::totalRowCount() const
{
    return rowCount();
}

int
CodexTimelineViewportModel::revision() const
{
    return revision_;
}

QVariant
CodexTimelineViewportModel::data(const QModelIndex& index, int role) const
{
    if (!checkIndex(index,
                    QAbstractItemModel::CheckIndexOption::IndexIsValid |
                      QAbstractItemModel::CheckIndexOption::ParentIsInvalid))
        return {};

    const ViewportRow& row = rows_.at(index.row());
    switch (role) {
        case SourceEntryIdRole:
            return row.sourceEntryId;
        case SemanticBlockRole:
            return row.blockRow >= 0;
        case BlockIdRole:
            return blockValue(row, blockIdRoleName);
        case BlockIndexRole:
            return row.blockRow;
        case BlockCountRole:
            return row.blockModel ? row.blockModel->rowCount() : 0;
        case CodeBlockRole:
            return blockValue(row, codeBlockRoleName);
        case BlockTextRole:
            return blockValue(row, blockTextRoleName);
        case PlainTextRole:
            return blockValue(row, plainTextRoleName);
        case LanguageRole:
            return blockValue(row, languageRoleName);
        case MarkdownRole:
            return blockValue(row, markdownRoleName);
        case FirstBlockInEntryRole:
            return row.blockRow == 0;
        case LastBlockInEntryRole:
            return row.blockModel && row.blockRow + 1 == row.blockModel->rowCount();
        default:
            break;
    }
    if (role == entryIdRole_)
        return row.entryId;
    return sourceValue(row, role);
}

QHash<int, QByteArray>
CodexTimelineViewportModel::roleNames() const
{
    QHash<int, QByteArray> roles = sourceModel_ ? sourceModel_->roleNames() : QHash<int, QByteArray>{};
    roles.insert(SourceEntryIdRole, sourceEntryIdRoleName);
    roles.insert(SemanticBlockRole, semanticBlockRoleName);
    roles.insert(BlockIdRole, blockIdRoleName);
    roles.insert(BlockIndexRole, blockIndexRoleName);
    roles.insert(BlockCountRole, blockCountRoleName);
    roles.insert(CodeBlockRole, codeBlockRoleName);
    roles.insert(BlockTextRole, blockTextRoleName);
    roles.insert(PlainTextRole, plainTextRoleName);
    roles.insert(LanguageRole, languageRoleName);
    roles.insert(MarkdownRole, markdownRoleName);
    roles.insert(FirstBlockInEntryRole, firstBlockInEntryRoleName);
    roles.insert(LastBlockInEntryRole, lastBlockInEntryRoleName);
    return roles;
}

QVariant
CodexTimelineViewportModel::valueAt(int row, const QString& roleName) const
{
    if (row < 0 || row >= rowCount())
        return {};
    const int role = roleForName(roleName.toUtf8());
    return role < 0 ? QVariant{} : data(index(row), role);
}

QString
CodexTimelineViewportModel::entryIdAt(int row) const
{
    return row >= 0 && row < rows_.size() ? rows_.at(row).entryId : QString();
}

int
CodexTimelineViewportModel::indexOfEntryId(const QString& entryId) const
{
    for (int row = 0; row < rows_.size(); ++row) {
        if (rows_.at(row).entryId == entryId)
            return row;
    }
    return -1;
}

int
CodexTimelineViewportModel::roleForName(const QByteArray& roleName) const
{
    return roleNames().key(roleName, -1);
}

QVariant
CodexTimelineViewportModel::sourceValue(const ViewportRow& row, int role) const
{
    if (!sourceModel_ || row.sourceRow < 0 || row.sourceRow >= sourceModel_->rowCount())
        return {};
    return sourceModel_->data(sourceModel_->index(row.sourceRow, 0), role);
}

QVariant
CodexTimelineViewportModel::blockValue(const ViewportRow& row, const QByteArray& roleName) const
{
    if (!row.blockModel || row.blockRow < 0 || row.blockRow >= row.blockModel->rowCount())
        return {};
    const int role =
      roleName == blockIdRoleName ? semanticIdRole(row.blockModel) : semanticTextRole(row.blockModel, roleName);
    return role < 0 ? QVariant{} : row.blockModel->data(row.blockModel->index(row.blockRow, 0), role);
}

void
CodexTimelineViewportModel::forwardSourceDataChanged(const QModelIndex& topLeft,
                                                     const QModelIndex& bottomRight,
                                                     const QList<int>& roles)
{
    if (!sourceModel_ || !topLeft.isValid() || !bottomRight.isValid())
        return;
    if (roles.contains(entryIdRole_) || roles.contains(markupDocumentRole_)) {
        rebuild();
        return;
    }

    const auto firstChanged =
      std::lower_bound(rows_.cbegin(), rows_.cend(), topLeft.row(), [](const ViewportRow& row, int sourceRow) {
          return row.sourceRow < sourceRow;
      });
    const auto firstUnchanged =
      std::lower_bound(firstChanged, rows_.cend(), bottomRight.row() + 1, [](const ViewportRow& row, int sourceRow) {
          return row.sourceRow < sourceRow;
      });
    if (firstChanged != firstUnchanged) {
        const int firstChangedRow = static_cast<int>(std::distance(rows_.cbegin(), firstChanged));
        const int lastChangedRow = static_cast<int>(std::distance(rows_.cbegin(), firstUnchanged)) - 1;
        emit dataChanged(index(firstChangedRow), index(lastChangedRow), roles);
    }
    ++revision_;
    emit revisionChanged();
}

void
CodexTimelineViewportModel::reconcileBlockModel(QAbstractItemModel* blockModel)
{
    if (!blockModel)
        return;

    QList<int> sourceRows = blockSourceRows_.value(blockModel);
    std::sort(sourceRows.begin(), sourceRows.end(), std::greater<>());
    bool rowCountChanged = false;
    const int blockIdRole = semanticIdRole(blockModel);
    for (int sourceRow : std::as_const(sourceRows)) {
        const auto firstSourceRow =
          std::lower_bound(rows_.cbegin(), rows_.cend(), sourceRow, [](const ViewportRow& row, int candidateSourceRow) {
              return row.sourceRow < candidateSourceRow;
          });
        const auto firstNextSourceRow = std::lower_bound(
          firstSourceRow, rows_.cend(), sourceRow + 1, [](const ViewportRow& row, int candidateSourceRow) {
              return row.sourceRow < candidateSourceRow;
          });
        if (firstSourceRow == firstNextSourceRow) {
            rebuild();
            return;
        }

        const int firstViewportRow = static_cast<int>(std::distance(rows_.cbegin(), firstSourceRow));
        const int oldRowCount = static_cast<int>(std::distance(firstSourceRow, firstNextSourceRow));
        const QString sourceEntryId = rows_.at(firstViewportRow).sourceEntryId;
        QList<ViewportRow> replacementRows;
        if (blockModel->rowCount() == 0) {
            replacementRows.append(ViewportRow{
              .sourceRow = sourceRow,
              .blockModel = blockModel,
              .entryId = sourceEntryId,
              .sourceEntryId = sourceEntryId,
            });
        } else {
            replacementRows.reserve(blockModel->rowCount());
            for (int blockRow = 0; blockRow < blockModel->rowCount(); ++blockRow) {
                replacementRows.append(ViewportRow{
                  .sourceRow = sourceRow,
                  .blockModel = blockModel,
                  .blockRow = blockRow,
                  .entryId = viewportEntryId(sourceEntryId, blockModel, blockRow, blockIdRole),
                  .sourceEntryId = sourceEntryId,
                });
            }
        }

        const int replacementRowCount = static_cast<int>(replacementRows.size());
        rowCountChanged = rowCountChanged || oldRowCount != replacementRowCount;
        const int sharedRowCount = std::min(oldRowCount, replacementRowCount);
        int commonPrefix = 0;
        while (commonPrefix < sharedRowCount &&
               rows_.at(firstViewportRow + commonPrefix).entryId == replacementRows.at(commonPrefix).entryId) {
            ++commonPrefix;
        }
        for (int offset = 0; offset < commonPrefix; ++offset)
            rows_[firstViewportRow + offset] = replacementRows.at(offset);

        if (commonPrefix < oldRowCount) {
            beginRemoveRows({}, firstViewportRow + commonPrefix, firstViewportRow + oldRowCount - 1);
            for (int row = firstViewportRow + oldRowCount - 1; row >= firstViewportRow + commonPrefix; --row)
                rows_.removeAt(row);
            endRemoveRows();
        }
        if (commonPrefix < replacementRowCount) {
            beginInsertRows({}, firstViewportRow + commonPrefix, firstViewportRow + replacementRowCount - 1);
            for (int offset = commonPrefix; offset < replacementRowCount; ++offset)
                rows_.insert(firstViewportRow + offset, replacementRows.at(offset));
            endInsertRows();
        }
        if (commonPrefix > 0) {
            emit dataChanged(index(firstViewportRow),
                             index(firstViewportRow + commonPrefix - 1),
                             { SemanticBlockRole,
                               BlockIdRole,
                               BlockIndexRole,
                               BlockCountRole,
                               CodeBlockRole,
                               BlockTextRole,
                               PlainTextRole,
                               LanguageRole,
                               MarkdownRole,
                               FirstBlockInEntryRole,
                               LastBlockInEntryRole });
        }
    }
    ++revision_;
    emit revisionChanged();
    if (rowCountChanged)
        emit statisticsChanged();
}

void
CodexTimelineViewportModel::insertSourceRows(const QModelIndex& parent, int first, int last)
{
    if (!sourceModel_ || parent.isValid() || first < 0 || last < first) {
        rebuild();
        return;
    }

    const int insertedSourceCount = last - first + 1;
    int insertionRow = rows_.size();
    for (int row = 0; row < rows_.size(); ++row) {
        if (rows_.at(row).sourceRow >= first) {
            insertionRow = row;
            break;
        }
    }
    for (ViewportRow& row : rows_) {
        if (row.sourceRow >= first)
            row.sourceRow += insertedSourceCount;
    }

    QList<ViewportRow> insertedRows;
    for (int sourceRow = first; sourceRow <= last; ++sourceRow)
        insertedRows.append(viewportRowsForSourceRow(sourceRow));
    if (insertedRows.isEmpty())
        return;

    beginInsertRows({}, insertionRow, insertionRow + insertedRows.size() - 1);
    for (qsizetype offset = 0; offset < insertedRows.size(); ++offset)
        rows_.insert(insertionRow + offset, std::move(insertedRows[offset]));
    endInsertRows();
    reconnectBlockModels();
    ++revision_;
    emit revisionChanged();
    emit statisticsChanged();
}

void
CodexTimelineViewportModel::removeSourceRows(const QModelIndex& parent, int first, int last)
{
    if (!sourceModel_ || parent.isValid() || first < 0 || last < first) {
        rebuild();
        return;
    }

    int firstViewportRow = -1;
    int lastViewportRow = -1;
    for (int row = 0; row < rows_.size(); ++row) {
        const int sourceRow = rows_.at(row).sourceRow;
        if (sourceRow < first || sourceRow > last)
            continue;
        if (firstViewportRow < 0)
            firstViewportRow = row;
        lastViewportRow = row;
    }
    if (firstViewportRow < 0) {
        rebuild();
        return;
    }

    beginRemoveRows({}, firstViewportRow, lastViewportRow);
    for (int row = lastViewportRow; row >= firstViewportRow; --row)
        rows_.removeAt(row);
    endRemoveRows();

    const int removedSourceCount = last - first + 1;
    for (ViewportRow& row : rows_) {
        if (row.sourceRow > last)
            row.sourceRow -= removedSourceCount;
    }
    reconnectBlockModels();
    ++revision_;
    emit revisionChanged();
    emit statisticsChanged();
}

QList<CodexTimelineViewportModel::ViewportRow>
CodexTimelineViewportModel::viewportRowsForSourceRow(int sourceRow)
{
    if (!sourceModel_ || sourceRow < 0 || sourceRow >= sourceModel_->rowCount())
        return {};

    const QModelIndex sourceIndex = sourceModel_->index(sourceRow, 0);
    const QString sourceEntryId =
      entryIdRole_ < 0 ? QString() : sourceModel_->data(sourceIndex, entryIdRole_).toString();
    QObject* documentObject =
      markupDocumentRole_ < 0 ? nullptr : sourceModel_->data(sourceIndex, markupDocumentRole_).value<QObject*>();
    auto* blockModel = documentObject ? documentObject->property("renderModel").value<QAbstractItemModel*>() : nullptr;
    if (!blockModel)
        blockModel = qobject_cast<QAbstractItemModel*>(documentObject);
    if (!blockModel || blockModel->rowCount() == 0) {
        if (blockModel)
            connectBlockModel(blockModel, sourceRow);
        return {
            ViewportRow{
              .sourceRow = sourceRow,
              .blockModel = blockModel,
              .entryId = sourceEntryId,
              .sourceEntryId = sourceEntryId,
            },
        };
    }

    connectBlockModel(blockModel, sourceRow);
    QList<ViewportRow> rows;
    rows.reserve(blockModel->rowCount());
    const int blockIdRole = semanticIdRole(blockModel);
    for (int blockRow = 0; blockRow < blockModel->rowCount(); ++blockRow) {
        rows.append(ViewportRow{
          .sourceRow = sourceRow,
          .blockModel = blockModel,
          .blockRow = blockRow,
          .entryId = viewportEntryId(sourceEntryId, blockModel, blockRow, blockIdRole),
          .sourceEntryId = sourceEntryId,
        });
    }
    return rows;
}

void
CodexTimelineViewportModel::connectBlockModel(QAbstractItemModel* blockModel, int sourceRow)
{
    if (!blockModel)
        return;
    QList<int>& sourceRows = blockSourceRows_[blockModel];
    if (!sourceRows.contains(sourceRow))
        sourceRows.append(sourceRow);
    if (connectedBlockModels_.contains(blockModel))
        return;
    connectedBlockModels_.insert(blockModel);
    const auto reconcile = [this, blockModel] { reconcileBlockModel(blockModel); };
    blockConnections_.append(connect(blockModel, &QAbstractItemModel::modelReset, this, reconcile));
    blockConnections_.append(connect(blockModel, &QAbstractItemModel::rowsInserted, this, reconcile));
    blockConnections_.append(connect(blockModel, &QAbstractItemModel::rowsRemoved, this, reconcile));
    blockConnections_.append(connect(blockModel, &QAbstractItemModel::rowsMoved, this, reconcile));
    blockConnections_.append(connect(blockModel, &QAbstractItemModel::layoutChanged, this, reconcile));
    blockConnections_.append(connect(blockModel, &QAbstractItemModel::dataChanged, this, reconcile));
    blockConnections_.append(connect(blockModel, &QObject::destroyed, this, &CodexTimelineViewportModel::rebuild));
}

void
CodexTimelineViewportModel::reconnectBlockModels()
{
    for (const QMetaObject::Connection& connection : std::as_const(blockConnections_))
        disconnect(connection);
    blockConnections_.clear();
    connectedBlockModels_.clear();
    blockSourceRows_.clear();
    for (const ViewportRow& row : std::as_const(rows_)) {
        if (row.blockModel)
            connectBlockModel(row.blockModel, row.sourceRow);
    }
}

void
CodexTimelineViewportModel::reconnectSourceRoles()
{
    sourceRolesByName_.clear();
    if (sourceModel_) {
        const auto roles = sourceModel_->roleNames();
        for (auto iterator = roles.cbegin(); iterator != roles.cend(); ++iterator)
            sourceRolesByName_.insert(iterator.value(), iterator.key());
    }
    entryIdRole_ = sourceRolesByName_.value(entryIdRoleName, -1);
    markupDocumentRole_ = sourceRolesByName_.value(markupDocumentRoleName, -1);
}

void
CodexTimelineViewportModel::disconnectModels()
{
    for (const QMetaObject::Connection& connection : std::as_const(sourceConnections_))
        disconnect(connection);
    for (const QMetaObject::Connection& connection : std::as_const(blockConnections_))
        disconnect(connection);
    sourceConnections_.clear();
    blockConnections_.clear();
    connectedBlockModels_.clear();
    blockSourceRows_.clear();
}

void
CodexTimelineViewportModel::rebuild()
{
    for (const QMetaObject::Connection& connection : std::as_const(blockConnections_))
        disconnect(connection);
    blockConnections_.clear();
    connectedBlockModels_.clear();
    blockSourceRows_.clear();

    reconnectSourceRoles();
    QList<ViewportRow> rows;
    if (sourceModel_) {
        for (int sourceRow = 0; sourceRow < sourceModel_->rowCount(); ++sourceRow)
            rows.append(viewportRowsForSourceRow(sourceRow));
    }

    beginResetModel();
    rows_ = std::move(rows);
    endResetModel();
    ++revision_;
    emit revisionChanged();
    emit statisticsChanged();
}
