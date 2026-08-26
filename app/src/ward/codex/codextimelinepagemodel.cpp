// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codextimelinepagemodel.h"

#include <QByteArray>
#include <QSet>

#include <algorithm>
#include <utility>

namespace {
constexpr auto entryIdRoleName = "entryId";
constexpr auto turnIdRoleName = "turnId";
constexpr auto activityGroupRoleName = "activityGroup";
constexpr auto standaloneActivityRoleName = "standaloneActivity";
constexpr auto fromUserRoleName = "fromUser";
constexpr auto commentaryRoleName = "commentary";
constexpr auto finalAnswerRoleName = "finalAnswer";
constexpr auto detailRowRoleName = "detailRow";
constexpr auto firstDetailInTurnRoleName = "firstDetailInTurn";
constexpr auto detailCountInTurnRoleName = "detailCountInTurn";
}

CodexTimelinePageModel::CodexTimelinePageModel(QObject* parent)
  : QIdentityProxyModel(parent)
{
}

void
CodexTimelinePageModel::setSourceModel(QAbstractItemModel* model)
{
    if (sourceModel() == model)
        return;

    disconnectSourceSignals();
    QIdentityProxyModel::setSourceModel(model);
    reconnectRoles();
    recomputeSnapshot();
    connectSourceSignals(model);
    advanceRevision();
    emit statisticsChanged();
}

int
CodexTimelinePageModel::turnsPerPage() const
{
    return turnsPerPage_;
}

void
CodexTimelinePageModel::setTurnsPerPage(int turns)
{
    turns = std::max(1, turns);
    if (turnsPerPage_ == turns)
        return;

    turnsPerPage_ = turns;
    rebuildPages();
    advanceRevision();
    emit turnsPerPageChanged();
    emit statisticsChanged();
}

int
CodexTimelinePageModel::totalRowCount() const
{
    return rowCount();
}

int
CodexTimelinePageModel::pageCount() const
{
    return pages_.size();
}

int
CodexTimelinePageModel::revision() const
{
    return revision_;
}

QVariant
CodexTimelinePageModel::data(const QModelIndex& index, int role) const
{
    if (!checkIndex(index,
                    QAbstractItemModel::CheckIndexOption::IndexIsValid |
                      QAbstractItemModel::CheckIndexOption::ParentIsInvalid))
        return {};

    const RowMetadata metadata = metadata_.value(index.row());
    switch (role) {
        case DetailRowRole:
            return metadata.detail;
        case FirstDetailInTurnRole:
            return metadata.firstDetailInTurn;
        case DetailCountInTurnRole:
            return metadata.detailCountInTurn;
        default:
            return QIdentityProxyModel::data(index, role);
    }
}

QHash<int, QByteArray>
CodexTimelinePageModel::roleNames() const
{
    QHash<int, QByteArray> roles = QIdentityProxyModel::roleNames();
    roles.insert(DetailRowRole, detailRowRoleName);
    roles.insert(FirstDetailInTurnRole, firstDetailInTurnRoleName);
    roles.insert(DetailCountInTurnRole, detailCountInTurnRoleName);
    return roles;
}

QVariant
CodexTimelinePageModel::valueAt(int row, const QString& roleName) const
{
    if (row < 0 || row >= rowCount())
        return {};

    const int role = roleForName(roleName.toUtf8());
    return role < 0 ? QVariant() : data(index(row, 0), role);
}

int
CodexTimelinePageModel::pageFirstRow(int page) const
{
    return page >= 0 && page < pages_.size() ? pages_.at(page).firstRow : -1;
}

int
CodexTimelinePageModel::pageRowCount(int page) const
{
    return page >= 0 && page < pages_.size() ? pages_.at(page).rowCount : 0;
}

QString
CodexTimelinePageModel::pageId(int page) const
{
    return page >= 0 && page < pages_.size() ? pages_.at(page).id : QString();
}

QVariant
CodexTimelinePageModel::sourceValue(int sourceRow, const QByteArray& roleName) const
{
    QAbstractItemModel* source = sourceModel();
    const int role = roleForName(roleName);
    if (!source || sourceRow < 0 || sourceRow >= source->rowCount() || role < 0)
        return {};
    return source->data(source->index(sourceRow, 0), role);
}

int
CodexTimelinePageModel::roleForName(const QByteArray& roleName) const
{
    return rolesByName_.value(roleName, -1);
}

void
CodexTimelinePageModel::reconnectRoles()
{
    rolesByName_.clear();
    const auto roles = roleNames();
    for (auto it = roles.cbegin(); it != roles.cend(); ++it)
        rolesByName_.insert(it.value(), it.key());
}

void
CodexTimelinePageModel::recomputeSnapshot()
{
    metadata_.clear();
    metadata_.resize(rowCount());
    QHash<QString, int> detailCounts;
    for (int row = 0; row < rowCount(); ++row) {
        const QString entryId = sourceValue(row, entryIdRoleName).toString();
        const QString turnId = sourceValue(row, turnIdRoleName).toString();
        const bool activityGroup = sourceValue(row, activityGroupRoleName).toBool();
        const bool standaloneActivity = sourceValue(row, standaloneActivityRoleName).toBool();
        const bool detail =
          !standaloneActivity &&
          (activityGroup || sourceValue(row, commentaryRoleName).toBool() ||
           (!sourceValue(row, fromUserRoleName).toBool() && !sourceValue(row, finalAnswerRoleName).toBool()));
        metadata_[row].entryId = entryId;
        metadata_[row].detail = detail;
        if (detail)
            ++detailCounts[turnId];
    }

    QSet<QString> turnsWithDetailHeader;
    for (int row = 0; row < rowCount(); ++row) {
        const QString turnId = sourceValue(row, turnIdRoleName).toString();
        metadata_[row].detailCountInTurn = detailCounts.value(turnId);
        if (metadata_[row].detail && !turnsWithDetailHeader.contains(turnId)) {
            metadata_[row].firstDetailInTurn = true;
            turnsWithDetailHeader.insert(turnId);
        }
    }
    rebuildPages();
}

void
CodexTimelinePageModel::rebuildPages()
{
    pages_.clear();
    if (rowCount() == 0)
        return;

    int pageFirstRow = 0;
    int turnsInPage = 0;
    QString currentTurn;
    for (int row = 0; row < rowCount(); ++row) {
        const QString turnId = sourceValue(row, turnIdRoleName).toString();
        const bool startsTurn = row == 0 || turnId != currentTurn;
        if (startsTurn && turnsInPage == turnsPerPage_) {
            pages_.append(Page{
              .firstRow = pageFirstRow,
              .rowCount = row - pageFirstRow,
              .id = QStringLiteral("page:%1").arg(metadata_.at(pageFirstRow).entryId),
            });
            pageFirstRow = row;
            turnsInPage = 0;
        }
        if (startsTurn) {
            ++turnsInPage;
            currentTurn = turnId;
        }
    }
    pages_.append(Page{
      .firstRow = pageFirstRow,
      .rowCount = rowCount() - pageFirstRow,
      .id = QStringLiteral("page:%1").arg(metadata_.at(pageFirstRow).entryId),
    });
}

void
CodexTimelinePageModel::connectSourceSignals(QAbstractItemModel* model)
{
    if (!model)
        return;

    const auto refresh = [this] { refreshFromSource(); };
    sourceConnections_.append(connect(model, &QAbstractItemModel::modelReset, this, refresh));
    sourceConnections_.append(connect(model, &QAbstractItemModel::dataChanged, this, refresh));
    sourceConnections_.append(connect(model, &QAbstractItemModel::rowsInserted, this, refresh));
    sourceConnections_.append(connect(model, &QAbstractItemModel::rowsRemoved, this, refresh));
    sourceConnections_.append(connect(model, &QAbstractItemModel::rowsMoved, this, refresh));
    sourceConnections_.append(connect(model, &QAbstractItemModel::layoutChanged, this, refresh));
    sourceConnections_.append(connect(model, &QObject::destroyed, this, [this] {
        rolesByName_.clear();
        metadata_.clear();
        pages_.clear();
        advanceRevision();
        emit statisticsChanged();
    }));
}

void
CodexTimelinePageModel::disconnectSourceSignals()
{
    for (const QMetaObject::Connection& connection : std::as_const(sourceConnections_))
        disconnect(connection);
    sourceConnections_.clear();
}

void
CodexTimelinePageModel::refreshFromSource()
{
    reconnectRoles();
    recomputeSnapshot();
    if (rowCount() > 0) {
        emit dataChanged(index(0, 0),
                         index(rowCount() - 1, 0),
                         { DetailRowRole, FirstDetailInTurnRole, DetailCountInTurnRole });
    }
    advanceRevision();
    emit statisticsChanged();
}

void
CodexTimelinePageModel::advanceRevision()
{
    ++revision_;
    emit revisionChanged();
}
