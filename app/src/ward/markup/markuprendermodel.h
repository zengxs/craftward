// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QAbstractListModel>
#include <QList>
#include <QPointer>
#include <QString>

class MarkupRenderModel : public QAbstractListModel
{
    Q_OBJECT
    Q_PROPERTY(QAbstractItemModel* sourceModel READ sourceModel WRITE setSourceModel NOTIFY sourceModelChanged)

  public:
    enum Role
    {
        SegmentIdRole = Qt::UserRole + 1,
        CodeBlockRole,
        SegmentTextRole,
        LanguageRole,
        MarkdownRole,
    };

    explicit MarkupRenderModel(QObject* parent = nullptr);

    [[nodiscard]] QAbstractItemModel* sourceModel() const;
    void setSourceModel(QAbstractItemModel* model);

    [[nodiscard]] int rowCount(const QModelIndex& parent = {}) const override;
    [[nodiscard]] QVariant data(const QModelIndex& index, int role) const override;
    [[nodiscard]] QHash<int, QByteArray> roleNames() const override;

  signals:
    void sourceModelChanged();

  private:
    struct Segment
    {
        QString id;
        bool codeBlock = false;
        QString text;
        QString language;
        bool markdown = false;
    };

    void rebuild();

    QPointer<QAbstractItemModel> sourceModel_;
    QList<Segment> rows_;
};
