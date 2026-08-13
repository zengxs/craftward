// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codexthreadmodel.h"

#include <QDateTime>

#include <algorithm>
#include <utility>

namespace {
bool
sameThread(const CodexThreadSummary& left, const CodexThreadSummary& right)
{
    return left.threadId() == right.threadId() && left.hasName() == right.hasName() &&
           (!left.hasName() || left.name() == right.name()) && left.preview() == right.preview() &&
           left.workingDirectory() == right.workingDirectory() &&
           left.createdAtUnixSeconds() == right.createdAtUnixSeconds() &&
           left.updatedAtUnixSeconds() == right.updatedAtUnixSeconds();
}
}

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
CodexThreadModel::reconcileThreads(QList<CodexThreadSummary> threads)
{
    if (threads_.isEmpty() && !threads.isEmpty()) {
        beginInsertRows({}, 0, int(threads.size() - 1));
        threads_ = std::move(threads);
        endInsertRows();
        return;
    }

    for (qsizetype targetIndex = 0; targetIndex < threads.size(); ++targetIndex) {
        const QString& threadId = threads.at(targetIndex).threadId();
        const auto match =
          std::find_if(threads_.cbegin() + std::min(targetIndex, threads_.size()),
                       threads_.cend(),
                       [&threadId](const CodexThreadSummary& thread) { return thread.threadId() == threadId; });
        if (match == threads_.cend()) {
            beginInsertRows({}, int(targetIndex), int(targetIndex));
            threads_.insert(targetIndex, threads.at(targetIndex));
            endInsertRows();
        } else {
            const qsizetype currentIndex = std::distance(threads_.cbegin(), match);
            if (currentIndex != targetIndex) {
                beginMoveRows({}, int(currentIndex), int(currentIndex), {}, int(targetIndex));
                threads_.move(currentIndex, targetIndex);
                endMoveRows();
            }
        }

        if (!sameThread(threads_.at(targetIndex), threads.at(targetIndex))) {
            threads_[targetIndex] = threads.at(targetIndex);
            const QModelIndex changedIndex = index(int(targetIndex));
            emit dataChanged(changedIndex, changedIndex);
        }
    }

    if (threads_.size() > threads.size()) {
        beginRemoveRows({}, int(threads.size()), int(threads_.size() - 1));
        threads_.remove(threads.size(), threads_.size() - threads.size());
        endRemoveRows();
    }
}
