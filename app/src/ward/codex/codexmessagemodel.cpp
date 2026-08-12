// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexmessagemodel.h"

#include <algorithm>
#include <utility>

CodexMessageModel::CodexMessageModel(QObject* parent)
  : QAbstractListModel(parent)
{
}

int
CodexMessageModel::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : messages_.size();
}

QVariant
CodexMessageModel::data(const QModelIndex& index, int role) const
{
    if (!checkIndex(index,
                    QAbstractItemModel::CheckIndexOption::IndexIsValid |
                      QAbstractItemModel::CheckIndexOption::ParentIsInvalid))
        return {};

    const CodexMessage& message = messages_.at(index.row());
    switch (role) {
        case MessageIdRole:
            return message.messageId();
        case FromUserRole:
            return message.role() == ward::codex::v1::MessageRoleGadget::MessageRole::MESSAGE_ROLE_USER;
        case CommentaryRole:
            return message.phase() == ward::codex::v1::MessagePhaseGadget::MessagePhase::MESSAGE_PHASE_COMMENTARY;
        case TextRole:
            return message.text();
        default:
            return {};
    }
}

QHash<int, QByteArray>
CodexMessageModel::roleNames() const
{
    return {
        { MessageIdRole, "messageId" },
        { FromUserRole, "fromUser" },
        { CommentaryRole, "commentary" },
        { TextRole, "text" },
    };
}

void
CodexMessageModel::replaceMessages(QList<CodexMessage> messages)
{
    beginResetModel();
    messages_ = std::move(messages);
    endResetModel();
}

void
CodexMessageModel::reconcileMessages(QList<CodexMessage> messages)
{
    const qsizetype sharedSize = std::min(messages_.size(), messages.size());
    qsizetype commonPrefix = 0;
    while (commonPrefix < sharedSize && !messages_.at(commonPrefix).messageId().isEmpty() &&
           messages_.at(commonPrefix).messageId() == messages.at(commonPrefix).messageId()) {
        ++commonPrefix;
    }

    if (commonPrefix != sharedSize) {
        replaceMessages(std::move(messages));
        return;
    }

    qsizetype firstChanged = -1;
    qsizetype lastChanged = -1;
    for (qsizetype index = 0; index < commonPrefix; ++index) {
        const CodexMessage& replacement = messages.at(index);
        CodexMessage& current = messages_[index];
        if (current.role() == replacement.role() && current.phase() == replacement.phase() &&
            current.text() == replacement.text()) {
            continue;
        }
        current = replacement;
        if (firstChanged < 0)
            firstChanged = index;
        lastChanged = index;
    }
    if (firstChanged >= 0) {
        emit dataChanged(
          this->index(firstChanged), this->index(lastChanged), { FromUserRole, CommentaryRole, TextRole });
    }

    if (messages.size() > messages_.size()) {
        const qsizetype first = messages_.size();
        const qsizetype last = messages.size() - 1;
        beginInsertRows({}, first, last);
        for (qsizetype index = first; index <= last; ++index)
            messages_.append(std::move(messages[index]));
        endInsertRows();
    } else if (messages.size() < messages_.size()) {
        const qsizetype first = messages.size();
        const qsizetype last = messages_.size() - 1;
        beginRemoveRows({}, first, last);
        messages_.remove(first, messages_.size() - first);
        endRemoveRows();
    }
}

void
CodexMessageModel::clear()
{
    replaceMessages({});
}
