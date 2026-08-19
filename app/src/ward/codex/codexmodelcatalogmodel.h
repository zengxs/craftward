// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "history.qpb.h"

#include <QAbstractListModel>
#include <QList>
#include <QVariantList>
#include <QtQml/qqmlregistration.h>

using CodexModelInfo = ward::codex::v1::ModelInfo;

class CodexConversationController;

class CodexModelCatalogModel : public QAbstractListModel
{
    Q_OBJECT
    QML_ANONYMOUS

  public:
    enum Role
    {
        ModelIdRole = Qt::UserRole + 1,
        ModelRole,
        DisplayNameRole,
        DescriptionRole,
        DefaultRole,
        DefaultReasoningEffortRole,
        SupportedReasoningEffortsRole,
    };

    explicit CodexModelCatalogModel(QObject* parent = nullptr);

    [[nodiscard]] int rowCount(const QModelIndex& parent = {}) const override;
    [[nodiscard]] QVariant data(const QModelIndex& index, int role) const override;
    [[nodiscard]] QHash<int, QByteArray> roleNames() const override;
    void replaceModels(QList<CodexModelInfo> models);

  private:
    friend class CodexConversationController;

    [[nodiscard]] const CodexModelInfo* findModel(const QString& model) const;
    [[nodiscard]] bool containsModel(const QString& model) const;
    [[nodiscard]] QVariantList reasoningEffortsForModel(const QString& model) const;
    [[nodiscard]] bool supportsReasoningEffort(const QString& model, const QString& effort) const;
    [[nodiscard]] QString resolveReasoningEffort(const QString& model, const QString& preferredEffort) const;
    [[nodiscard]] static QVariantList reasoningEfforts(const CodexModelInfo& model);

    QList<CodexModelInfo> models_;
};
