// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codextimelinepresentationmodel.h"

#include <QAbstractItemModel>
#include <QIdentityProxyModel>
#include <QSet>
#include <QSortFilterProxyModel>

#include <algorithm>
#include <functional>
#include <iterator>
#include <limits>
#include <utility>

class TimelineExpansionInvalidationModel final : public QIdentityProxyModel
{
  public:
    using QIdentityProxyModel::QIdentityProxyModel;

    void setInvalidationRole(int role) { invalidationRole_ = role; }

    [[nodiscard]] int invalidationRole() const { return invalidationRole_; }

    [[nodiscard]] QHash<int, QByteArray> roleNames() const override
    {
        auto roles = QIdentityProxyModel::roleNames();
        roles.insert(invalidationRole_, "_timelineExpansionInvalidation");
        return roles;
    }

    void invalidateRows(const QVector<int>& sourceRows)
    {
        int firstRow = -1;
        int lastRow = -1;
        const auto publishRange = [this, &firstRow, &lastRow] {
            if (firstRow < 0)
                return;
            emit dataChanged(index(firstRow, 0), index(lastRow, 0), { invalidationRole_ });
            firstRow = -1;
            lastRow = -1;
        };
        for (const int sourceRow : sourceRows) {
            if (sourceRow < 0 || sourceRow >= rowCount())
                continue;
            if (firstRow < 0) {
                firstRow = sourceRow;
                lastRow = sourceRow;
                continue;
            }
            if (sourceRow == lastRow + 1) {
                lastRow = sourceRow;
                continue;
            }
            publishRange();
            firstRow = sourceRow;
            lastRow = sourceRow;
        }
        publishRange();
    }

  private:
    int invalidationRole_ = Qt::UserRole;
};

class TimelinePresentationFilterModel final : public QSortFilterProxyModel
{
  public:
    using RowPredicate = std::function<bool(int)>;

    explicit TimelinePresentationFilterModel(RowPredicate rowPredicate)
      : rowPredicate_(std::move(rowPredicate))
    {
    }

  protected:
    [[nodiscard]] bool filterAcceptsRow(int sourceRow, const QModelIndex& sourceParent) const override
    {
        return !sourceParent.isValid() && rowPredicate_(sourceRow);
    }

  private:
    RowPredicate rowPredicate_;
};

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
  : QAbstractListModel(parent)
  , expansionInvalidationModel_(std::make_unique<TimelineExpansionInvalidationModel>())
  , filterModel_(
      std::make_unique<TimelinePresentationFilterModel>([this](int sourceRow) { return acceptsSourceRow(sourceRow); }))
{
    connectFilterSignals();
    filterModel_->setDynamicSortFilter(true);
    filterModel_->setFilterRole(expansionInvalidationModel_->invalidationRole());
    filterModel_->setSourceModel(expansionInvalidationModel_.get());

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

CodexTimelinePresentationModel::~CodexTimelinePresentationModel() = default;

void
CodexTimelinePresentationModel::setSourceModel(QAbstractItemModel* model)
{
    if (presentationSourceModel_ == model)
        return;

    disconnectSourceSignals();
    expansionInvalidationModel_->setSourceModel(nullptr);
    presentationSourceModel_ = model;
    reconnectRoles(model);
    recomputeMetadata(model);
    entryIndex_.reset(presentationSourceModel_, rolesByName_.value(entryIdRoleName, -1));
    const int invalidationRole = chooseInvalidationRole(model);
    expansionInvalidationModel_->setInvalidationRole(invalidationRole);
    filterModel_->setFilterRole(invalidationRole);
    connectSourceSignals(model);
    expansionInvalidationModel_->setSourceModel(model);
    connectSourceDestructionSignal(model);
    advanceRevision();
    emit statisticsChanged();
    emit sourceModelChanged();
}

QAbstractItemModel*
CodexTimelinePresentationModel::sourceModel() const
{
    return presentationSourceModel_;
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

int
CodexTimelinePresentationModel::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : filterModel_->rowCount();
}

QVariant
CodexTimelinePresentationModel::data(const QModelIndex& index, int role) const
{
    if (!index.isValid() || index.parent().isValid() || index.row() < 0 || index.row() >= rowCount())
        return {};
    const QModelIndex filterIndex = filterModel_->index(index.row(), index.column());
    const QModelIndex adapterIndex = filterModel_->mapToSource(filterIndex);
    const RowMetadata metadata = metadata_.value(adapterIndex.row());
    switch (role) {
        case DetailRowRole:
            return adapterIndex.isValid() && metadata.detail;
        case FirstDetailInTurnRole:
            return adapterIndex.isValid() && metadata.firstDetailInTurn;
        case DetailCountInTurnRole:
            return adapterIndex.isValid() ? metadata.detailCountInTurn : 0;
        default:
            break;
    }
    if (role == TurnExpandedRole) {
        return adapterIndex.isValid() && expandedTurns_.contains(metadata.turnId);
    }
    if (!presentationSourceModel_)
        return {};
    const QModelIndex sourceIndex = expansionInvalidationModel_->mapToSource(adapterIndex);
    return sourceIndex.isValid() ? presentationSourceModel_->data(sourceIndex, role) : QVariant();
}

QHash<int, QByteArray>
CodexTimelinePresentationModel::roleNames() const
{
    QHash<int, QByteArray> roles = filterModel_->roleNames();
    roles.remove(expansionInvalidationModel_->invalidationRole());
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
    const QPersistentModelIndex entry = entryIndex_.find(entryId);
    if (!entry.isValid())
        return -1;
    const QModelIndex adapterIndex = expansionInvalidationModel_->mapFromSource(entry);
    const QModelIndex proxyIndex = filterModel_->mapFromSource(adapterIndex);
    return proxyIndex.isValid() ? proxyIndex.row() : -1;
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

    if (expanded)
        expandedTurns_.insert(turnId);
    else
        expandedTurns_.remove(turnId);
    invalidateTurns({ turnId });
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

    const QSet<QString> affectedTurns = expandedTurns_;
    expandedTurns_.clear();
    invalidateTurns(affectedTurns);
    for (const QString& turnId : affectedTurns)
        publishExpansionChange(turnId);
}

bool
CodexTimelinePresentationModel::acceptsSourceRow(int sourceRow) const
{
    ++filterEvaluationCount_;
    if (!presentationSourceModel_)
        return false;
    const RowMetadata metadata = metadata_.value(sourceRow);
    if (!metadata.detail)
        return true;
    if (metadata.firstDetailInTurn)
        return true;
    return expandedTurns_.contains(metadata.turnId);
}

int
CodexTimelinePresentationModel::chooseInvalidationRole(QAbstractItemModel* model) const
{
    int maximumRole = TurnExpandedRole;
    if (model) {
        const auto roles = model->roleNames();
        for (auto role = roles.cbegin(); role != roles.cend(); ++role)
            maximumRole = std::max(maximumRole, role.key());
    }
    if (maximumRole < std::numeric_limits<int>::max())
        return maximumRole + 1;

    QSet<int> occupiedRoles;
    if (model) {
        const auto roles = model->roleNames();
        for (auto role = roles.cbegin(); role != roles.cend(); ++role)
            occupiedRoles.insert(role.key());
    }
    occupiedRoles.insert(DetailRowRole);
    occupiedRoles.insert(FirstDetailInTurnRole);
    occupiedRoles.insert(DetailCountInTurnRole);
    occupiedRoles.insert(TurnExpandedRole);
    for (int candidate = Qt::UserRole; candidate < std::numeric_limits<int>::max(); ++candidate) {
        if (!occupiedRoles.contains(candidate))
            return candidate;
    }
    return Qt::DisplayRole;
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
    invalidateTurns(affectedTurns);
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
        entryIndex_.reset(presentationSourceModel_, rolesByName_.value(entryIdRoleName, -1));
    };
    sourceConnections_.append(connect(model, &QAbstractItemModel::modelReset, this, refresh));
    sourceConnections_.append(
      connect(model,
              &QAbstractItemModel::dataChanged,
              this,
              [this, model](const QModelIndex& topLeft, const QModelIndex& bottomRight, const QList<int>& roles) {
                  refreshChangedRows(model, topLeft, bottomRight, roles);
                  refreshEntryIndexes(topLeft, bottomRight, roles);
              }));
    sourceConnections_.append(connect(model, &QAbstractItemModel::rowsInserted, this, refresh));
    sourceConnections_.append(connect(model, &QAbstractItemModel::rowsRemoved, this, refresh));
    sourceConnections_.append(connect(model, &QAbstractItemModel::rowsMoved, this, refresh));
    sourceConnections_.append(connect(model, &QAbstractItemModel::layoutChanged, this, refresh));
}

void
CodexTimelinePresentationModel::connectSourceDestructionSignal(QAbstractItemModel* model)
{
    if (!model)
        return;
    sourceConnections_.append(connect(model, &QObject::destroyed, this, [this] {
        filterModel_->invalidate();
        presentationSourceModel_ = nullptr;
        rolesByName_.clear();
        entryIndex_.reset(nullptr, -1);
        metadata_.clear();
        rowsByTurn_.clear();
        emit sourceModelChanged();
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
CodexTimelinePresentationModel::connectFilterSignals()
{
    connect(filterModel_.get(), &QAbstractItemModel::modelAboutToBeReset, this, [this] { beginResetModel(); });
    connect(filterModel_.get(), &QAbstractItemModel::modelReset, this, [this] { endResetModel(); });

    connect(filterModel_.get(),
            &QAbstractItemModel::rowsAboutToBeInserted,
            this,
            [this](const QModelIndex& parent, int first, int last) {
                Q_ASSERT(!parent.isValid());
                beginInsertRows({}, first, last);
            });
    connect(filterModel_.get(), &QAbstractItemModel::rowsInserted, this, [this](const QModelIndex&, int, int) {
        endInsertRows();
    });
    connect(filterModel_.get(),
            &QAbstractItemModel::rowsAboutToBeRemoved,
            this,
            [this](const QModelIndex& parent, int first, int last) {
                Q_ASSERT(!parent.isValid());
                beginRemoveRows({}, first, last);
            });
    connect(filterModel_.get(), &QAbstractItemModel::rowsRemoved, this, [this](const QModelIndex&, int, int) {
        endRemoveRows();
    });
    connect(filterModel_.get(),
            &QAbstractItemModel::rowsAboutToBeMoved,
            this,
            [this](const QModelIndex& sourceParent,
                   int sourceFirst,
                   int sourceLast,
                   const QModelIndex& destinationParent,
                   int destinationRow) {
                Q_ASSERT(!sourceParent.isValid());
                Q_ASSERT(!destinationParent.isValid());
                beginMoveRows({}, sourceFirst, sourceLast, {}, destinationRow);
            });
    connect(filterModel_.get(),
            &QAbstractItemModel::rowsMoved,
            this,
            [this](const QModelIndex&, int, int, const QModelIndex&, int) { endMoveRows(); });
    connect(filterModel_.get(),
            &QAbstractItemModel::dataChanged,
            this,
            [this](const QModelIndex& topLeft, const QModelIndex& bottomRight, const QList<int>& roles) {
                if (!topLeft.isValid() || !bottomRight.isValid())
                    return;
                QList<int> exposedRoles = roles;
                exposedRoles.removeAll(expansionInvalidationModel_->invalidationRole());
                if (!roles.isEmpty() && exposedRoles.isEmpty())
                    return;
                emit dataChanged(
                  index(topLeft.row(), topLeft.column()), index(bottomRight.row(), bottomRight.column()), exposedRoles);
            });
    connect(filterModel_.get(),
            &QAbstractItemModel::layoutAboutToBeChanged,
            this,
            [this](const QList<QPersistentModelIndex>&, QAbstractItemModel::LayoutChangeHint) {
                if (forwardingLayoutReset_)
                    return;
                forwardingLayoutReset_ = true;
                beginResetModel();
            });
    connect(filterModel_.get(),
            &QAbstractItemModel::layoutChanged,
            this,
            [this](const QList<QPersistentModelIndex>&, QAbstractItemModel::LayoutChangeHint) {
                if (!forwardingLayoutReset_)
                    return;
                forwardingLayoutReset_ = false;
                endResetModel();
            });
    connect(
      filterModel_.get(),
      &QAbstractItemModel::headerDataChanged,
      this,
      [this](Qt::Orientation orientation, int first, int last) { emit headerDataChanged(orientation, first, last); });
}

void
CodexTimelinePresentationModel::invalidateTurns(const QSet<QString>& turnIds)
{
    QVector<int> sourceRows;
    for (const QString& turnId : turnIds) {
        const auto rows = rowsByTurn_.constFind(turnId);
        if (rows != rowsByTurn_.cend())
            sourceRows.append(*rows);
    }
    std::sort(sourceRows.begin(), sourceRows.end());
    sourceRows.erase(std::unique(sourceRows.begin(), sourceRows.end()), sourceRows.end());
    expansionInvalidationModel_->invalidateRows(sourceRows);
}

void
CodexTimelinePresentationModel::publishExpansionChange(const QString& turnId)
{
    const auto sourceRows = rowsByTurn_.constFind(turnId);
    if (!presentationSourceModel_ || sourceRows == rowsByTurn_.cend())
        return;

    int firstChangedRow = -1;
    int lastChangedRow = -1;
    for (const int sourceRow : *sourceRows) {
        const QModelIndex adapterIndex = expansionInvalidationModel_->index(sourceRow, 0);
        const QModelIndex proxyIndex = filterModel_->mapFromSource(adapterIndex);
        if (!proxyIndex.isValid())
            continue;
        const int proxyRow = proxyIndex.row();
        if (firstChangedRow < 0) {
            firstChangedRow = proxyRow;
            lastChangedRow = proxyRow;
            continue;
        }
        if (proxyRow == lastChangedRow + 1) {
            lastChangedRow = proxyRow;
            continue;
        }
        emit dataChanged(index(firstChangedRow, 0), index(lastChangedRow, 0), { TurnExpandedRole });
        firstChangedRow = proxyRow;
        lastChangedRow = proxyRow;
    }
    if (firstChangedRow >= 0)
        emit dataChanged(index(firstChangedRow, 0), index(lastChangedRow, 0), { TurnExpandedRole });
}

void
CodexTimelinePresentationModel::advanceRevision()
{
    ++revision_;
    emit revisionChanged();
}

void
CodexTimelinePresentationModel::refreshEntryIndexes(const QModelIndex& topLeft,
                                                    const QModelIndex& bottomRight,
                                                    const QList<int>& roles)
{
    if (!topLeft.isValid() || !bottomRight.isValid())
        return;
    const int entryIdRole = rolesByName_.value(entryIdRoleName, -1);
    if (entryIdRole < 0 || (!roles.isEmpty() && !roles.contains(entryIdRole)))
        return;
    entryIndex_.rebuild();
}
