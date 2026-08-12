// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "history.qpb.h"

#include <QAbstractListModel>
#include <QList>
#include <QString>
#include <QtQml/qqmlregistration.h>

using CodexMessage = ward::codex::v1::Message;

class CodexMessageModel : public QAbstractListModel
{
    Q_OBJECT
    QML_ANONYMOUS

  public:
    enum Role
    {
        MessageIdRole = Qt::UserRole + 1,
        FromUserRole,
        CommentaryRole,
        TextRole,
    };

    explicit CodexMessageModel(QObject* parent = nullptr);

    [[nodiscard]] int rowCount(const QModelIndex& parent = {}) const override;
    [[nodiscard]] QVariant data(const QModelIndex& index, int role) const override;
    [[nodiscard]] QHash<int, QByteArray> roleNames() const override;

    void reconcileMessages(QList<CodexMessage> messages);
    void clear();

  private:
    void replaceMessages(QList<CodexMessage> messages);

    QList<CodexMessage> messages_;
};
