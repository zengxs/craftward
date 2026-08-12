// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexmessagemodel.h"

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
CodexMessageModel::clear()
{
    replaceMessages({});
}
