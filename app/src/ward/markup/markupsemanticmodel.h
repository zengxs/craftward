// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "document.qpb.h"
#include "ward/markup/markupdocumentmodel.h"

#include <QFutureWatcher>
#include <QTimer>

/// Retains semantic data, never text layouts, for one complete message.
class MarkupSemanticModel : public QAbstractListModel
{
    Q_OBJECT

  public:
    enum Role
    {
        SegmentIdRole = Qt::UserRole + 1,
        CodeBlockRole,
        SegmentTextRole,
        PlainTextRole,
        LanguageRole,
        MarkdownRole,
        SemanticSegmentRole,
    };

    explicit MarkupSemanticModel(QObject* parent = nullptr);
    void reconcileSource(const QString& source, MarkupDocumentModel::SourceFormat format, bool finalized);

    [[nodiscard]] int rowCount(const QModelIndex& parent = {}) const override;
    [[nodiscard]] QVariant data(const QModelIndex& index, int role) const override;
    [[nodiscard]] QHash<int, QByteArray> roleNames() const override;

  private:
    struct Segment
    {
        QString id;
        bool codeBlock = false;
        QString text;
        QString language;
        QVariant semantic;

        bool operator==(const Segment&) const = default;
    };
    struct Result
    {
        quint64 generation = 0;
        QList<Segment> segments;
        QString error;
    };

    static Result parse(quint64 generation, const QString& source, MarkupDocumentModel::SourceFormat format);
    void schedule();
    void dispatch();
    void applyFinished();
    void reconcileSegments(QList<Segment> segments);

    QString source_;
    MarkupDocumentModel::SourceFormat format_ = MarkupDocumentModel::SourceFormat::PlainText;
    bool finalized_ = false;
    quint64 generation_ = 0;
    quint64 appliedGeneration_ = 0;
    QTimer timer_;
    QFutureWatcher<Result> watcher_;
    QList<Segment> segments_;
};
