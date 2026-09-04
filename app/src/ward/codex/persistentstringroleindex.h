// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QAbstractItemModel>
#include <QHash>
#include <QPersistentModelIndex>
#include <QPointer>
#include <QString>

#include <algorithm>

class PersistentStringRoleIndex
{
  public:
    void reset(QAbstractItemModel* model, int role)
    {
        model_ = model;
        role_ = role;
        rebuild();
    }

    void rebuild()
    {
        indexes_.clear();
        rememberRows(0, model_ ? model_->rowCount() - 1 : -1);
    }

    void clear() { indexes_.clear(); }

    void rememberRows(int first, int last)
    {
        if (!model_ || role_ < 0)
            return;
        const int firstRow = std::max(0, first);
        const int lastRow = std::min(model_->rowCount() - 1, last);
        for (int row = firstRow; row <= lastRow; ++row) {
            const QModelIndex modelIndex = model_->index(row, 0);
            const QString key = model_->data(modelIndex, role_).toString();
            if (!key.isEmpty())
                indexes_.insert(key, QPersistentModelIndex(modelIndex));
        }
    }

    void forgetRows(int first, int last)
    {
        if (!model_ || role_ < 0)
            return;
        const int firstRow = std::max(0, first);
        const int lastRow = std::min(model_->rowCount() - 1, last);
        for (int row = firstRow; row <= lastRow; ++row)
            indexes_.remove(model_->data(model_->index(row, 0), role_).toString());
    }

    [[nodiscard]] QPersistentModelIndex find(const QString& key) const
    {
        const auto entry = indexes_.constFind(key);
        return entry == indexes_.cend() || !entry->isValid() ? QPersistentModelIndex{} : *entry;
    }

  private:
    QPointer<QAbstractItemModel> model_;
    QHash<QString, QPersistentModelIndex> indexes_;
    int role_ = -1;
};
