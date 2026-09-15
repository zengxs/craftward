// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QSortFilterProxyModel>
#include <QtQml/qqmlregistration.h>

class CodexThreadFilterModel : public QSortFilterProxyModel
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QString searchText READ searchText WRITE setSearchText NOTIFY searchTextChanged)

  public:
    explicit CodexThreadFilterModel(QObject* parent = nullptr);
    QString searchText() const { return searchText_; }
    void setSearchText(const QString& text);
    void setSourceModel(QAbstractItemModel* source) override;

  signals:
    void searchTextChanged();

  protected:
    bool filterAcceptsRow(int sourceRow, const QModelIndex& sourceParent) const override;

  private:
    QString searchText_;
    QMetaObject::Connection sourceDataChanged_;
};
