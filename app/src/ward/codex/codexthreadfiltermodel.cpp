// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "codexthreadfiltermodel.h"

CodexThreadFilterModel::CodexThreadFilterModel(QObject* parent)
  : QSortFilterProxyModel(parent)
{
}

void
CodexThreadFilterModel::setSearchText(const QString& text)
{
    if (searchText_ == text)
        return;
    beginFilterChange();
    searchText_ = text;
    endFilterChange(Direction::Rows);
    emit searchTextChanged();
}

void
CodexThreadFilterModel::setSourceModel(QAbstractItemModel* source)
{
    if (source == sourceModel())
        return;
    disconnect(sourceDataChanged_);
    QSortFilterProxyModel::setSourceModel(source);
    if (source) {
        setFilterRole(source->roleNames().key(QByteArrayLiteral("title"), Qt::DisplayRole));
        sourceDataChanged_ =
          connect(source,
                  &QAbstractItemModel::dataChanged,
                  this,
                  [this](const QModelIndex&, const QModelIndex&, const QList<int>& roles) {
                      // The search reads multiple roles, beyond the proxy's default filter role.
                      if (!searchText_.isEmpty() && !roles.isEmpty() && !roles.contains(filterRole())) {
                          beginFilterChange();
                          endFilterChange(Direction::Rows);
                      }
                  });
    }
}

bool
CodexThreadFilterModel::filterAcceptsRow(int sourceRow, const QModelIndex& sourceParent) const
{
    if (searchText_.isEmpty())
        return true;
    const auto* source = sourceModel();
    if (!source)
        return false;
    const auto roles = source->roleNames();
    const QModelIndex index = source->index(sourceRow, 0, sourceParent);
    const QString title = source->data(index, roles.key(QByteArrayLiteral("title"), -1)).toString();
    const QString directory = source->data(index, roles.key(QByteArrayLiteral("workingDirectory"), -1)).toString();
    return (title + u' ' + directory).contains(searchText_, Qt::CaseInsensitive);
}
