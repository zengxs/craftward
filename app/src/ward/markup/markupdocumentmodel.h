// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QAbstractListModel>
#include <QFutureWatcher>
#include <QTimer>

/// Retains semantic data, never text layouts, for one complete message.
class MarkupDocumentModel : public QAbstractListModel
{
    Q_OBJECT

  public:
    enum class SourceFormat
    {
        PlainText,
        Markdown,
    };

    enum Role
    {
        SegmentIdRole = Qt::UserRole + 1,
        CodeBlockRole,
        SegmentTextRole,
        PlainTextRole,
        LanguageRole,
        SemanticSegmentRole,
    };

    explicit MarkupDocumentModel(QObject* parent = nullptr);
    bool reconcileSource(const QString& source, SourceFormat format, bool finalized = true);

    [[nodiscard]] int rowCount(const QModelIndex& parent = {}) const override;
    [[nodiscard]] QVariant data(const QModelIndex& index, int role) const override;
    [[nodiscard]] QHash<int, QByteArray> roleNames() const override;

  signals:
    void documentReconciled();

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

    static Result parse(quint64 generation, const QString& source, SourceFormat format);
    void schedule();
    void dispatch();
    void applyFinished();
    void reconcileSegments(QList<Segment> segments);

    QString source_;
    SourceFormat format_ = SourceFormat::PlainText;
    bool finalized_ = false;
    quint64 generation_ = 0;
    quint64 appliedGeneration_ = 0;
    QTimer timer_;
    QFutureWatcher<Result> watcher_;
    QList<Segment> segments_;
};
