// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/markup/markuprendermodel.h"

#include <QByteArray>
#include <QHash>
#include <QVariant>

#include <utility>

namespace {
int
roleForName(const QHash<int, QByteArray>& roles, const QByteArray& name)
{
    return roles.key(name, -1);
}

void
appendFragment(QString* target, const QString& fragment)
{
    if (fragment.isEmpty())
        return;
    if (!target->isEmpty())
        target->append(QStringLiteral("\n\n"));
    target->append(fragment);
}

QString
codeDisplayText(QString text)
{
    if (text.endsWith(QStringLiteral("\r\n")))
        text.chop(2);
    else if (text.endsWith(QLatin1Char('\n')) || text.endsWith(QLatin1Char('\r')))
        text.chop(1);
    return text;
}
}

MarkupRenderModel::MarkupRenderModel(QObject* parent)
  : QAbstractListModel(parent)
{
}

QAbstractItemModel*
MarkupRenderModel::sourceModel() const
{
    return sourceModel_;
}

void
MarkupRenderModel::setSourceModel(QAbstractItemModel* model)
{
    if (sourceModel_ == model)
        return;

    if (sourceModel_)
        disconnect(sourceModel_, nullptr, this, nullptr);
    sourceModel_ = model;

    if (sourceModel_) {
        connect(sourceModel_, &QAbstractItemModel::modelReset, this, &MarkupRenderModel::rebuild);
        connect(sourceModel_, &QAbstractItemModel::dataChanged, this, [this] { rebuild(); });
        connect(sourceModel_, &QAbstractItemModel::rowsInserted, this, [this] { rebuild(); });
        connect(sourceModel_, &QAbstractItemModel::rowsRemoved, this, [this] { rebuild(); });
        connect(sourceModel_, &QAbstractItemModel::rowsMoved, this, [this] { rebuild(); });
        connect(sourceModel_, &QAbstractItemModel::layoutChanged, this, [this] { rebuild(); });
        connect(sourceModel_, &QObject::destroyed, this, [this] {
            sourceModel_ = nullptr;
            rebuild();
            emit sourceModelChanged();
        });
    }

    rebuild();
    emit sourceModelChanged();
}

int
MarkupRenderModel::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : rows_.size();
}

QVariant
MarkupRenderModel::data(const QModelIndex& index, int role) const
{
    if (!checkIndex(index,
                    QAbstractItemModel::CheckIndexOption::IndexIsValid |
                      QAbstractItemModel::CheckIndexOption::ParentIsInvalid))
        return {};

    const Segment& segment = rows_.at(index.row());
    switch (role) {
        case SegmentIdRole:
            return segment.id;
        case CodeBlockRole:
            return segment.codeBlock;
        case SegmentTextRole:
            return segment.text;
        case LanguageRole:
            return segment.language;
        case MarkdownRole:
            return segment.markdown;
        default:
            return {};
    }
}

QHash<int, QByteArray>
MarkupRenderModel::roleNames() const
{
    return {
        { SegmentIdRole, "segmentId" }, { CodeBlockRole, "codeBlock" }, { SegmentTextRole, "segmentText" },
        { LanguageRole, "language" },   { MarkdownRole, "markdown" },
    };
}

void
MarkupRenderModel::rebuild()
{
    QList<Segment> segments;
    if (sourceModel_) {
        const QHash<int, QByteArray> roles = sourceModel_->roleNames();
        const int blockIdRole = roleForName(roles, QByteArrayLiteral("blockId"));
        const int codeBlockRole = roleForName(roles, QByteArrayLiteral("codeBlock"));
        const int blockTextRole = roleForName(roles, QByteArrayLiteral("blockText"));
        const int languageRole = roleForName(roles, QByteArrayLiteral("language"));
        const int markdownRole = roleForName(roles, QByteArrayLiteral("markdown"));

        if (blockIdRole >= 0 && codeBlockRole >= 0 && blockTextRole >= 0 && languageRole >= 0 && markdownRole >= 0) {
            segments.reserve(sourceModel_->rowCount());
            for (int row = 0; row < sourceModel_->rowCount(); ++row) {
                const QModelIndex index = sourceModel_->index(row, 0);
                Segment segment{
                    .id = sourceModel_->data(index, blockIdRole).toString(),
                    .codeBlock = sourceModel_->data(index, codeBlockRole).toBool(),
                    .text = sourceModel_->data(index, blockTextRole).toString(),
                    .language = sourceModel_->data(index, languageRole).toString(),
                    .markdown = sourceModel_->data(index, markdownRole).toBool(),
                };
                if (segment.codeBlock)
                    segment.text = codeDisplayText(std::move(segment.text));
                if (!segment.codeBlock && !segments.isEmpty() && !segments.constLast().codeBlock &&
                    segments.constLast().markdown == segment.markdown) {
                    Segment& previous = segments.last();
                    appendFragment(&previous.text, segment.text);
                    continue;
                }
                segments.append(std::move(segment));
            }
        }
    }

    beginResetModel();
    rows_ = std::move(segments);
    endResetModel();
}
