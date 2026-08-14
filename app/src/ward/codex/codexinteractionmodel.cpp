// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexinteractionmodel.h"

#include <QVariantMap>

#include <utility>

namespace {
QString
interactionId(const CodexPendingInteraction& interaction)
{
    return QString::number(interaction.interactionId());
}
}

CodexInteractionModel::CodexInteractionModel(QObject* parent)
  : QAbstractListModel(parent)
{
}

int
CodexInteractionModel::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : interactions_.size();
}

QVariant
CodexInteractionModel::data(const QModelIndex& index, int role) const
{
    if (!checkIndex(index,
                    QAbstractItemModel::CheckIndexOption::IndexIsValid |
                      QAbstractItemModel::CheckIndexOption::ParentIsInvalid))
        return {};

    const CodexPendingInteraction& interaction = interactions_.at(index.row());
    switch (role) {
        case InteractionIdRole:
            return interactionId(interaction);
        case KindRole:
            return static_cast<int>(interaction.kind());
        case CommandRole:
            return interaction.hasCommand() ? interaction.command() : QString();
        case WorkingDirectoryRole:
            return interaction.hasWorkingDirectory() ? interaction.workingDirectory() : QString();
        case ReasonRole:
            return interaction.hasReason() ? interaction.reason() : QString();
        case GrantRootRole:
            return interaction.hasGrantRoot() ? interaction.grantRoot() : QString();
        case AvailableDecisionsRole:
            return decisions(interaction);
        case QuestionsRole:
            return questions(interaction);
        case BlockingRole:
            return interaction.userInputIsBlocking();
        case ResolvingRole:
            return resolvingInteractions_.contains(interactionId(interaction));
        default:
            return {};
    }
}

QHash<int, QByteArray>
CodexInteractionModel::roleNames() const
{
    return {
        { InteractionIdRole, "interactionId" },
        { KindRole, "kind" },
        { CommandRole, "command" },
        { WorkingDirectoryRole, "workingDirectory" },
        { ReasonRole, "reason" },
        { GrantRootRole, "grantRoot" },
        { AvailableDecisionsRole, "availableDecisions" },
        { QuestionsRole, "questions" },
        { BlockingRole, "blocking" },
        { ResolvingRole, "resolving" },
    };
}

void
CodexInteractionModel::reconcileInteractions(QList<CodexPendingInteraction> interactions)
{
    const bool resolvingChanged = !resolvingInteractions_.isEmpty();
    resolvingInteractions_.clear();
    if (interactions_ == interactions) {
        if (resolvingChanged && !interactions_.isEmpty())
            emit dataChanged(index(0), index(interactions_.size() - 1), { ResolvingRole });
        return;
    }
    beginResetModel();
    interactions_ = std::move(interactions);
    endResetModel();
}

void
CodexInteractionModel::setResolving(const QString& targetInteractionId, bool resolving)
{
    const bool wasResolving = resolvingInteractions_.contains(targetInteractionId);
    if (resolving)
        resolvingInteractions_.insert(targetInteractionId);
    else
        resolvingInteractions_.remove(targetInteractionId);
    const bool changed = wasResolving != resolving;
    if (!changed)
        return;
    for (qsizetype row = 0; row < interactions_.size(); ++row) {
        if (interactionId(interactions_.at(row)) != targetInteractionId)
            continue;
        const QModelIndex changedIndex = index(static_cast<int>(row));
        emit dataChanged(changedIndex, changedIndex, { ResolvingRole });
        return;
    }
}

void
CodexInteractionModel::clear()
{
    if (interactions_.isEmpty() && resolvingInteractions_.isEmpty())
        return;
    beginResetModel();
    interactions_.clear();
    resolvingInteractions_.clear();
    endResetModel();
}

QVariantList
CodexInteractionModel::decisions(const CodexPendingInteraction& interaction) const
{
    QVariantList result;
    result.reserve(interaction.availableDecisions().size());
    for (const auto decision : interaction.availableDecisions())
        result.append(static_cast<int>(decision));
    return result;
}

QVariantList
CodexInteractionModel::questions(const CodexPendingInteraction& interaction) const
{
    QVariantList result;
    result.reserve(interaction.questions().size());
    for (const auto& question : interaction.questions()) {
        QVariantList options;
        options.reserve(question.options().size());
        for (const auto& option : question.options()) {
            options.append(QVariantMap{
              { QStringLiteral("label"), option.label() },
              { QStringLiteral("description"), option.description() },
            });
        }
        result.append(QVariantMap{
          { QStringLiteral("questionId"), question.questionId() },
          { QStringLiteral("header"), question.header() },
          { QStringLiteral("prompt"), question.prompt() },
          { QStringLiteral("options"), options },
          { QStringLiteral("allowsOther"), question.allowsOther() },
          { QStringLiteral("secret"), question.secret() },
        });
    }
    return result;
}
