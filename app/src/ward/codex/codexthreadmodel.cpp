// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexthreadmodel.h"

#include <QDateTime>

#include <utility>

CodexThreadModel::CodexThreadModel(QObject* parent)
  : QAbstractListModel(parent)
{
}

int
CodexThreadModel::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : threads_.size();
}

QVariant
CodexThreadModel::data(const QModelIndex& index, int role) const
{
    if (!checkIndex(index,
                    QAbstractItemModel::CheckIndexOption::IndexIsValid |
                      QAbstractItemModel::CheckIndexOption::ParentIsInvalid))
        return {};

    const CodexThreadSummary& thread = threads_.at(index.row());
    switch (role) {
        case ThreadIdRole:
            return thread.threadId();
        case TitleRole: {
            const QString name = thread.hasName() ? thread.name() : QString();
            return name.trimmed().isEmpty() ? thread.preview().simplified() : name;
        }
        case PreviewRole:
            return thread.preview();
        case WorkingDirectoryRole:
            return thread.workingDirectory();
        case CreatedAtRole:
            return QDateTime::fromSecsSinceEpoch(thread.createdAtUnixSeconds());
        case UpdatedAtRole:
            return QDateTime::fromSecsSinceEpoch(thread.updatedAtUnixSeconds());
        default:
            return {};
    }
}

QHash<int, QByteArray>
CodexThreadModel::roleNames() const
{
    return {
        { ThreadIdRole, "threadId" },   { TitleRole, "title" },
        { PreviewRole, "preview" },     { WorkingDirectoryRole, "workingDirectory" },
        { CreatedAtRole, "createdAt" }, { UpdatedAtRole, "updatedAt" },
    };
}

void
CodexThreadModel::replaceThreads(QList<CodexThreadSummary> threads)
{
    beginResetModel();
    threads_ = std::move(threads);
    endResetModel();
}
