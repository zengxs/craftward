// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "history.qpb.h"

#include <QAbstractListModel>
#include <QList>
#include <QSet>
#include <QString>
#include <QtQml/qqmlregistration.h>

using CodexPendingInteraction = ward::codex::v1::PendingInteraction;

class CodexInteractionModel : public QAbstractListModel
{
    Q_OBJECT
    QML_ANONYMOUS

  public:
    enum Role
    {
        InteractionIdRole = Qt::UserRole + 1,
        KindRole,
        CommandRole,
        WorkingDirectoryRole,
        ReasonRole,
        GrantRootRole,
        AvailableDecisionsRole,
        QuestionsRole,
        BlockingRole,
        ResolvingRole,
    };

    explicit CodexInteractionModel(QObject* parent = nullptr);

    [[nodiscard]] int rowCount(const QModelIndex& parent = {}) const override;
    [[nodiscard]] QVariant data(const QModelIndex& index, int role) const override;
    [[nodiscard]] QHash<int, QByteArray> roleNames() const override;

    void reconcileInteractions(QList<CodexPendingInteraction> interactions);
    void setResolving(const QString& interactionId, bool resolving);
    void clear();

  private:
    [[nodiscard]] QVariantList decisions(const CodexPendingInteraction& interaction) const;
    [[nodiscard]] QVariantList questions(const CodexPendingInteraction& interaction) const;

    QList<CodexPendingInteraction> interactions_;
    QSet<QString> resolvingInteractions_;
};
