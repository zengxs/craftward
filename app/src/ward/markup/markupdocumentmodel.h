// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QAbstractListModel>
#include <QFutureWatcher>
#include <QList>
#include <QString>
#include <QTimer>

#include <memory>

class MarkupRenderModel;

class MarkupDocumentModel : public QAbstractListModel
{
    Q_OBJECT
    Q_PROPERTY(QAbstractItemModel* renderModel READ renderModel CONSTANT)

  public:
    enum class SourceFormat
    {
        PlainText,
        Markdown,
    };

    enum Role
    {
        BlockIdRole = Qt::UserRole + 1,
        CodeBlockRole,
        SourceStartRole,
        SourceEndRole,
        BlockTextRole,
        PlainTextRole,
        LanguageRole,
        MarkdownRole,
    };

    explicit MarkupDocumentModel(QObject* parent = nullptr);
    ~MarkupDocumentModel() override;

    [[nodiscard]] int rowCount(const QModelIndex& parent = {}) const override;
    [[nodiscard]] QVariant data(const QModelIndex& index, int role) const override;
    [[nodiscard]] QHash<int, QByteArray> roleNames() const override;
    [[nodiscard]] QAbstractItemModel* renderModel() const;

    bool reconcileSource(const QString& source, SourceFormat format, bool finalized = true);
    Q_INVOKABLE void prepareForLayout();

  signals:
    void documentReconciled();

  private:
    enum class BlockKind
    {
        Prose,
        Code,
    };

    struct BlockRow
    {
        QString blockId;
        BlockKind kind = BlockKind::Prose;
        qulonglong sourceStart = 0;
        qulonglong sourceEnd = 0;
        QString text;
        QString plainText;
        QString language;
        bool markdown = false;

        bool operator==(const BlockRow&) const = default;
    };

    struct ParseRequest
    {
        quint64 generation = 0;
        QString source;
        SourceFormat format = SourceFormat::PlainText;
        bool finalized = false;
        qulonglong sourceOffset = 0;
        QList<BlockRow> retainedRows;
    };

    struct ParseResult
    {
        quint64 generation = 0;
        QString source;
        SourceFormat format = SourceFormat::PlainText;
        bool finalized = false;
        bool parsed = false;
        QString errorMessage;
        QList<BlockRow> rows;
    };

    [[nodiscard]] static QList<BlockRow> fallbackRows(const QString& source,
                                                      SourceFormat format,
                                                      qulonglong sourceOffset = 0);
    [[nodiscard]] static bool parseRows(const QString& source,
                                        SourceFormat format,
                                        qulonglong sourceOffset,
                                        QList<BlockRow>* rows,
                                        QString* errorMessage);
    [[nodiscard]] static ParseResult parseRequest(ParseRequest request);
    [[nodiscard]] ParseRequest makeParseRequest() const;
    void scheduleParse();
    void dispatchParse();
    void applyFinishedParse();
    void applyParseResult(ParseResult result);
    void reconcileRows(QList<BlockRow> rows);

    QString requestedSource_;
    SourceFormat requestedFormat_ = SourceFormat::PlainText;
    bool requestedFinalized_ = false;
    quint64 requestedGeneration_ = 0;
    quint64 appliedGeneration_ = 0;
    QString appliedSource_;
    SourceFormat appliedFormat_ = SourceFormat::PlainText;
    bool hasAppliedSource_ = false;
    QList<BlockRow> rows_;
    QTimer parseTimer_;
    QFutureWatcher<ParseResult> parseWatcher_;
    mutable std::unique_ptr<MarkupRenderModel> renderModel_;
};
