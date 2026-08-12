// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "history.qpb.h"

#include <QAbstractListModel>
#include <QList>
#include <QString>
#include <QtQml/qqmlregistration.h>

using CodexThreadSummary = ward::codex::v1::ThreadSummary;

class CodexThreadModel : public QAbstractListModel
{
    Q_OBJECT
    QML_ANONYMOUS

  public:
    enum Role
    {
        ThreadIdRole = Qt::UserRole + 1,
        TitleRole,
        PreviewRole,
        WorkingDirectoryRole,
        CreatedAtRole,
        UpdatedAtRole,
    };

    explicit CodexThreadModel(QObject* parent = nullptr);

    [[nodiscard]] int rowCount(const QModelIndex& parent = {}) const override;
    [[nodiscard]] QVariant data(const QModelIndex& index, int role) const override;
    [[nodiscard]] QHash<int, QByteArray> roleNames() const override;

    void replaceThreads(QList<CodexThreadSummary> threads);

  private:
    QList<CodexThreadSummary> threads_;
};
