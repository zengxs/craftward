// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexmodelcatalogmodel.h"

#include <QVariantList>
#include <QVariantMap>

#include <algorithm>
#include <utility>

CodexModelCatalogModel::CodexModelCatalogModel(QObject* parent)
  : QAbstractListModel(parent)
{
}

int
CodexModelCatalogModel::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : models_.size();
}

QVariant
CodexModelCatalogModel::data(const QModelIndex& index, int role) const
{
    if (!checkIndex(index,
                    QAbstractItemModel::CheckIndexOption::IndexIsValid |
                      QAbstractItemModel::CheckIndexOption::ParentIsInvalid))
        return {};

    const CodexModelInfo& model = models_.at(index.row());
    switch (role) {
        case ModelIdRole:
            return model.modelId();
        case ModelRole:
            return model.model();
        case DisplayNameRole:
            return model.displayName();
        case DescriptionRole:
            return model.description();
        case DefaultRole:
            return model.isDefault();
        case DefaultReasoningEffortRole:
            return model.defaultReasoningEffort();
        case SupportedReasoningEffortsRole:
            return reasoningEfforts(model);
        default:
            return {};
    }
}

QHash<int, QByteArray>
CodexModelCatalogModel::roleNames() const
{
    return {
        { ModelIdRole, "modelId" },
        { ModelRole, "model" },
        { DisplayNameRole, "displayName" },
        { DescriptionRole, "description" },
        { DefaultRole, "isDefault" },
        { DefaultReasoningEffortRole, "defaultReasoningEffort" },
        { SupportedReasoningEffortsRole, "supportedReasoningEfforts" },
    };
}

const CodexModelInfo*
CodexModelCatalogModel::findModel(const QString& model) const
{
    const auto found = std::find_if(models_.cbegin(), models_.cend(), [&model](const CodexModelInfo& candidate) {
        return candidate.model() == model;
    });
    return found == models_.cend() ? nullptr : &*found;
}

bool
CodexModelCatalogModel::containsModel(const QString& model) const
{
    return findModel(model) != nullptr;
}

QVariantList
CodexModelCatalogModel::reasoningEffortsForModel(const QString& model) const
{
    const CodexModelInfo* modelInfo = findModel(model);
    return modelInfo == nullptr ? QVariantList{} : reasoningEfforts(*modelInfo);
}

bool
CodexModelCatalogModel::supportsReasoningEffort(const QString& model, const QString& effort) const
{
    if (effort.isEmpty())
        return false;
    const CodexModelInfo* modelInfo = findModel(model);
    if (modelInfo == nullptr)
        return false;
    const auto& efforts = modelInfo->supportedReasoningEfforts();
    return std::any_of(
      efforts.cbegin(), efforts.cend(), [&effort](const auto& option) { return option.reasoningEffort() == effort; });
}

QString
CodexModelCatalogModel::resolveReasoningEffort(const QString& model, const QString& preferredEffort) const
{
    const CodexModelInfo* modelInfo = findModel(model);
    if (modelInfo == nullptr)
        return {};
    return supportsReasoningEffort(model, preferredEffort) ? preferredEffort : modelInfo->defaultReasoningEffort();
}

QVariantList
CodexModelCatalogModel::reasoningEfforts(const CodexModelInfo& model)
{
    QVariantList efforts;
    efforts.reserve(model.supportedReasoningEfforts().size());
    for (const auto& effort : model.supportedReasoningEfforts()) {
        efforts.append(QVariantMap{
          { QStringLiteral("reasoningEffort"), effort.reasoningEffort() },
          { QStringLiteral("description"), effort.description() },
        });
    }
    return efforts;
}

void
CodexModelCatalogModel::replaceModels(QList<CodexModelInfo> models)
{
    if (models_ == models)
        return;
    beginResetModel();
    models_ = std::move(models);
    endResetModel();
}
