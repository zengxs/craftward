// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codextimelinepresentationmodel.h"

#include <QAbstractItemModel>
#include <QSet>

#include <iterator>
#include <utility>

namespace {
constexpr auto turnIdRoleName = "turnId";
constexpr auto entryIdRoleName = "entryId";
constexpr auto activityGroupRoleName = "activityGroup";
constexpr auto standaloneActivityRoleName = "standaloneActivity";
constexpr auto fromUserRoleName = "fromUser";
constexpr auto commentaryRoleName = "commentary";
constexpr auto finalAnswerRoleName = "finalAnswer";
constexpr auto detailRowRoleName = "detailRow";
constexpr auto firstDetailInTurnRoleName = "firstDetailInTurn";
constexpr auto detailCountInTurnRoleName = "detailCountInTurn";
constexpr auto turnExpandedRoleName = "turnExpanded";
}

CodexTimelinePresentationModel::CodexTimelinePresentationModel(QObject* parent)
  : QSortFilterProxyModel(parent)
{
    setDynamicSortFilter(true);

    const auto rowsChanged = [this] {
        advanceRevision();
        emit statisticsChanged();
    };
    connect(this, &QAbstractItemModel::modelReset, this, rowsChanged);
    connect(this, &QAbstractItemModel::rowsInserted, this, rowsChanged);
    connect(this, &QAbstractItemModel::rowsRemoved, this, rowsChanged);
    connect(this, &QAbstractItemModel::rowsMoved, this, rowsChanged);
    connect(this, &QAbstractItemModel::layoutChanged, this, rowsChanged);
    connect(this, &QAbstractItemModel::dataChanged, this, [this] { advanceRevision(); });
}

void
CodexTimelinePresentationModel::setSourceModel(QAbstractItemModel* model)
{
    if (sourceModel() == model)
        return;

    disconnectSourceSignals();
    reconnectRoles(model);
    recomputeMetadata(model);
    connectSourceSignals(model);
    QSortFilterProxyModel::setSourceModel(model);
    advanceRevision();
    emit statisticsChanged();
}

int
CodexTimelinePresentationModel::totalRowCount() const
{
    return rowCount();
}

int
CodexTimelinePresentationModel::revision() const
{
    return revision_;
}

QVariant
CodexTimelinePresentationModel::data(const QModelIndex& index, int role) const
{
    const QModelIndex sourceIndex = mapToSource(index);
    const RowMetadata metadata = metadata_.value(sourceIndex.row());
    switch (role) {
        case DetailRowRole:
            return sourceIndex.isValid() && metadata.detail;
        case FirstDetailInTurnRole:
            return sourceIndex.isValid() && metadata.firstDetailInTurn;
        case DetailCountInTurnRole:
            return sourceIndex.isValid() ? metadata.detailCountInTurn : 0;
        default:
            break;
    }
    if (role == TurnExpandedRole) {
        return sourceIndex.isValid() && expandedTurns_.contains(metadata.turnId);
    }
    return QSortFilterProxyModel::data(index, role);
}

QHash<int, QByteArray>
CodexTimelinePresentationModel::roleNames() const
{
    QHash<int, QByteArray> roles = QSortFilterProxyModel::roleNames();
    roles.insert(DetailRowRole, detailRowRoleName);
    roles.insert(FirstDetailInTurnRole, firstDetailInTurnRoleName);
    roles.insert(DetailCountInTurnRole, detailCountInTurnRoleName);
    roles.insert(TurnExpandedRole, turnExpandedRoleName);
    return roles;
}

QVariant
CodexTimelinePresentationModel::valueAt(int row, const QString& roleName) const
{
    if (row < 0 || row >= rowCount())
        return {};
    const int role = roleForName(roleName.toUtf8());
    return role < 0 ? QVariant() : data(index(row, 0), role);
}

QString
CodexTimelinePresentationModel::entryIdAt(int row) const
{
    return valueAt(row, QString::fromLatin1(entryIdRoleName)).toString();
}

int
CodexTimelinePresentationModel::indexOfEntryId(const QString& entryId) const
{
    for (int row = 0; row < rowCount(); ++row) {
        if (entryIdAt(row) == entryId)
            return row;
    }
    return -1;
}

bool
CodexTimelinePresentationModel::turnExpanded(const QString& turnId) const
{
    return expandedTurns_.contains(turnId);
}

void
CodexTimelinePresentationModel::setTurnExpanded(const QString& turnId, bool expanded)
{
    if (turnId.isEmpty() || expandedTurns_.contains(turnId) == expanded)
        return;

    beginFilterChange();
    if (expanded)
        expandedTurns_.insert(turnId);
    else
        expandedTurns_.remove(turnId);
    endFilterChange(Direction::Rows);
    publishExpansionChange(turnId);
}

void
CodexTimelinePresentationModel::toggleTurn(const QString& turnId)
{
    setTurnExpanded(turnId, !turnExpanded(turnId));
}

void
CodexTimelinePresentationModel::clearExpandedTurns()
{
    if (expandedTurns_.isEmpty())
        return;

    beginFilterChange();
    expandedTurns_.clear();
    endFilterChange(Direction::Rows);
    if (rowCount() > 0)
        emit dataChanged(index(0, 0), index(rowCount() - 1, 0), { TurnExpandedRole });
}

bool
CodexTimelinePresentationModel::filterAcceptsRow(int sourceRow, const QModelIndex& sourceParent) const
{
    if (sourceParent.isValid() || !sourceModel())
        return false;
    const RowMetadata metadata = metadata_.value(sourceRow);
    if (!metadata.detail)
        return true;
    if (metadata.firstDetailInTurn)
        return true;
    return expandedTurns_.contains(metadata.turnId);
}

int
CodexTimelinePresentationModel::roleForName(const QByteArray& roleName) const
{
    if (roleName == detailRowRoleName)
        return DetailRowRole;
    if (roleName == firstDetailInTurnRoleName)
        return FirstDetailInTurnRole;
    if (roleName == detailCountInTurnRoleName)
        return DetailCountInTurnRole;
    if (roleName == turnExpandedRoleName)
        return TurnExpandedRole;
    return rolesByName_.value(roleName, -1);
}

QVariant
CodexTimelinePresentationModel::sourceValue(int sourceRow, int role) const
{
    QAbstractItemModel* source = sourceModel();
    if (!source || sourceRow < 0 || sourceRow >= source->rowCount() || role < 0)
        return {};
    return source->data(source->index(sourceRow, 0), role);
}

CodexTimelinePresentationModel::RowMetadata
CodexTimelinePresentationModel::sourceRowMetadata(QAbstractItemModel* model, int sourceRow) const
{
    const auto value = [model, sourceRow](int role) {
        return role < 0 ? QVariant() : model->data(model->index(sourceRow, 0), role);
    };
    const bool standaloneActivity = value(standaloneActivityRole_).toBool();
    return RowMetadata{
        .turnId = value(turnIdRole_).toString(),
        .detail = !standaloneActivity && (value(activityGroupRole_).toBool() || value(commentaryRole_).toBool() ||
                                          (!value(fromUserRole_).toBool() && !value(finalAnswerRole_).toBool())),
    };
}

bool
CodexTimelinePresentationModel::rolesAffectPresentation(const QList<int>& roles) const
{
    if (roles.isEmpty())
        return true;
    const int presentationRoles[] = {
        turnIdRole_, activityGroupRole_, standaloneActivityRole_, fromUserRole_, commentaryRole_, finalAnswerRole_,
    };
    for (int role : roles) {
        if (std::find(std::begin(presentationRoles), std::end(presentationRoles), role) !=
            std::end(presentationRoles)) {
            return true;
        }
    }
    return false;
}

void
CodexTimelinePresentationModel::reconnectRoles(QAbstractItemModel* model)
{
    rolesByName_.clear();
    if (model) {
        const auto roles = model->roleNames();
        for (auto it = roles.cbegin(); it != roles.cend(); ++it)
            rolesByName_.insert(it.value(), it.key());
    }
    turnIdRole_ = rolesByName_.value(turnIdRoleName, -1);
    activityGroupRole_ = rolesByName_.value(activityGroupRoleName, -1);
    standaloneActivityRole_ = rolesByName_.value(standaloneActivityRoleName, -1);
    fromUserRole_ = rolesByName_.value(fromUserRoleName, -1);
    commentaryRole_ = rolesByName_.value(commentaryRoleName, -1);
    finalAnswerRole_ = rolesByName_.value(finalAnswerRoleName, -1);
}

void
CodexTimelinePresentationModel::recomputeMetadata(QAbstractItemModel* model)
{
    metadata_.clear();
    rowsByTurn_.clear();
    if (!model)
        return;

    metadata_.resize(model->rowCount());
    for (int row = 0; row < model->rowCount(); ++row) {
        metadata_[row] = sourceRowMetadata(model, row);
        rowsByTurn_[metadata_.at(row).turnId].append(row);
    }

    const QList<QString> turnIds = rowsByTurn_.keys();
    for (const QString& turnId : turnIds)
        recomputeTurnPresentation(turnId);
}

void
CodexTimelinePresentationModel::refreshChangedRows(QAbstractItemModel* model,
                                                   const QModelIndex& topLeft,
                                                   const QModelIndex& bottomRight,
                                                   const QList<int>& roles)
{
    if (!model || !rolesAffectPresentation(roles))
        return;
    if (metadata_.size() != model->rowCount() || !topLeft.isValid() || !bottomRight.isValid()) {
        recomputeMetadata(model);
        return;
    }

    const int firstRow = std::max(0, topLeft.row());
    const int lastRow = std::min(model->rowCount() - 1, bottomRight.row());
    QSet<QString> affectedTurns;
    beginFilterChange();
    for (int row = firstRow; row <= lastRow; ++row) {
        const QString previousTurnId = metadata_.at(row).turnId;
        RowMetadata updated = sourceRowMetadata(model, row);
        affectedTurns.insert(previousTurnId);
        affectedTurns.insert(updated.turnId);
        if (previousTurnId != updated.turnId) {
            auto previousRows = rowsByTurn_.find(previousTurnId);
            if (previousRows != rowsByTurn_.end()) {
                previousRows->removeOne(row);
                if (previousRows->isEmpty())
                    rowsByTurn_.erase(previousRows);
            }
            QVector<int>& currentRows = rowsByTurn_[updated.turnId];
            currentRows.insert(std::lower_bound(currentRows.begin(), currentRows.end(), row), row);
        }
        metadata_[row] = std::move(updated);
    }
    for (const QString& turnId : std::as_const(affectedTurns))
        recomputeTurnPresentation(turnId);
    endFilterChange(Direction::Rows);
}

void
CodexTimelinePresentationModel::recomputeTurnPresentation(const QString& turnId)
{
    const auto rows = rowsByTurn_.constFind(turnId);
    if (rows == rowsByTurn_.cend())
        return;

    int detailCount = 0;
    for (int row : *rows) {
        if (metadata_.at(row).detail)
            ++detailCount;
    }
    bool hasDetailHeader = false;
    for (int row : *rows) {
        RowMetadata& metadata = metadata_[row];
        metadata.detailCountInTurn = detailCount;
        metadata.firstDetailInTurn = metadata.detail && !hasDetailHeader;
        hasDetailHeader = hasDetailHeader || metadata.detail;
    }
}

void
CodexTimelinePresentationModel::connectSourceSignals(QAbstractItemModel* model)
{
    if (!model)
        return;

    const auto refresh = [this, model] {
        reconnectRoles(model);
        recomputeMetadata(model);
    };
    sourceConnections_.append(connect(model, &QAbstractItemModel::modelReset, this, refresh));
    sourceConnections_.append(
      connect(model,
              &QAbstractItemModel::dataChanged,
              this,
              [this, model](const QModelIndex& topLeft, const QModelIndex& bottomRight, const QList<int>& roles) {
                  refreshChangedRows(model, topLeft, bottomRight, roles);
              }));
    sourceConnections_.append(connect(model, &QAbstractItemModel::rowsInserted, this, refresh));
    sourceConnections_.append(connect(model, &QAbstractItemModel::rowsRemoved, this, refresh));
    sourceConnections_.append(connect(model, &QAbstractItemModel::rowsMoved, this, refresh));
    sourceConnections_.append(connect(model, &QAbstractItemModel::layoutChanged, this, refresh));
    sourceConnections_.append(connect(model, &QObject::destroyed, this, [this] {
        rolesByName_.clear();
        metadata_.clear();
        rowsByTurn_.clear();
    }));
}

void
CodexTimelinePresentationModel::disconnectSourceSignals()
{
    for (const QMetaObject::Connection& connection : std::as_const(sourceConnections_))
        disconnect(connection);
    sourceConnections_.clear();
}

void
CodexTimelinePresentationModel::publishExpansionChange(const QString& turnId)
{
    int firstChangedRow = -1;
    for (int row = 0; row < rowCount(); ++row) {
        const QModelIndex proxyIndex = index(row, 0);
        const QModelIndex sourceIndex = mapToSource(proxyIndex);
        if (sourceIndex.isValid() && metadata_.value(sourceIndex.row()).turnId == turnId) {
            if (firstChangedRow < 0)
                firstChangedRow = row;
            continue;
        }
        if (firstChangedRow >= 0) {
            emit dataChanged(index(firstChangedRow, 0), index(row - 1, 0), { TurnExpandedRole });
            firstChangedRow = -1;
        }
    }
    if (firstChangedRow >= 0)
        emit dataChanged(index(firstChangedRow, 0), index(rowCount() - 1, 0), { TurnExpandedRole });
}

void
CodexTimelinePresentationModel::advanceRevision()
{
    ++revision_;
    emit revisionChanged();
}
