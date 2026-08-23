// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/markup/markupdocumentmodel.h"

#include "document.qpb.h"
#include "ward/coreffierror.h"

#include <ward_core.h>

#include <QByteArray>
#include <QByteArrayView>
#include <QDebug>
#include <QtConcurrent/QtConcurrentRun>
#include <QtProtobuf/QProtobufSerializer>

#include <algorithm>
#include <cstdint>
#include <limits>
#include <memory>
#include <utility>

namespace {
struct WardOwnedBufferDeleter
{
    void operator()(WardOwnedBuffer* buffer) const { ward_core_owned_buffer_destroy(buffer); }
};

using OwnedWardBuffer = std::unique_ptr<WardOwnedBuffer, WardOwnedBufferDeleter>;
using WireBlock = ward::markup::v1::Block;
constexpr int STREAM_PARSE_INTERVAL_MILLISECONDS = 32;

WardMarkupSourceFormat
toWireFormat(MarkupDocumentModel::SourceFormat format)
{
    switch (format) {
        case MarkupDocumentModel::SourceFormat::PlainText:
            return WardMarkupSourceFormatPlainText;
        case MarkupDocumentModel::SourceFormat::Markdown:
            return WardMarkupSourceFormatMarkdown;
    }
    Q_UNREACHABLE_RETURN(WardMarkupSourceFormatPlainText);
}

}

MarkupDocumentModel::MarkupDocumentModel(QObject* parent)
  : QAbstractListModel(parent)
{
    parseTimer_.setSingleShot(true);
    parseTimer_.setInterval(STREAM_PARSE_INTERVAL_MILLISECONDS);
    connect(&parseTimer_, &QTimer::timeout, this, &MarkupDocumentModel::dispatchParse);
    connect(&parseWatcher_, &QFutureWatcher<ParseResult>::finished, this, &MarkupDocumentModel::applyFinishedParse);
}

int
MarkupDocumentModel::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : rows_.size();
}

QVariant
MarkupDocumentModel::data(const QModelIndex& index, int role) const
{
    if (!checkIndex(index,
                    QAbstractItemModel::CheckIndexOption::IndexIsValid |
                      QAbstractItemModel::CheckIndexOption::ParentIsInvalid))
        return {};

    const BlockRow& row = rows_.at(index.row());
    switch (role) {
        case BlockIdRole:
            return row.blockId;
        case CodeBlockRole:
            return row.kind == BlockKind::Code;
        case SourceStartRole:
            return row.sourceStart;
        case SourceEndRole:
            return row.sourceEnd;
        case BlockTextRole:
            return row.text;
        case PlainTextRole:
            return row.plainText;
        case LanguageRole:
            return row.language;
        case MarkdownRole:
            return row.markdown;
        default:
            return {};
    }
}

QHash<int, QByteArray>
MarkupDocumentModel::roleNames() const
{
    return {
        { BlockIdRole, "blockId" },     { CodeBlockRole, "codeBlock" }, { SourceStartRole, "sourceStart" },
        { SourceEndRole, "sourceEnd" }, { BlockTextRole, "blockText" }, { PlainTextRole, "plainText" },
        { LanguageRole, "language" },   { MarkdownRole, "markdown" },
    };
}

bool
MarkupDocumentModel::reconcileSource(const QString& source, SourceFormat format, bool finalized)
{
    if (requestedSource_ == source && requestedFormat_ == format && requestedFinalized_ == finalized)
        return true;

    requestedSource_ = source;
    requestedFormat_ = format;
    requestedFinalized_ = finalized;
    ++requestedGeneration_;

    if (format == SourceFormat::PlainText) {
        parseTimer_.stop();
        appliedSource_ = source;
        appliedFormat_ = format;
        appliedFinalized_ = finalized;
        hasAppliedSource_ = true;
        reconcileRows(fallbackRows(source, format));
        emit documentReconciled();
        return true;
    }

    scheduleParse();
    return true;
}

QList<MarkupDocumentModel::BlockRow>
MarkupDocumentModel::fallbackRows(const QString& source, SourceFormat format, qulonglong sourceOffset)
{
    if (source.isEmpty())
        return {};
    const QByteArray encoded = source.toUtf8();
    return {
        BlockRow{
          .blockId = QStringLiteral("prose:%1").arg(sourceOffset),
          .sourceStart = sourceOffset,
          .sourceEnd = sourceOffset + static_cast<qulonglong>(encoded.size()),
          .text = source,
          .plainText = source,
          .markdown = format == SourceFormat::Markdown,
        },
    };
}

bool
MarkupDocumentModel::parseRows(const QString& source,
                               SourceFormat format,
                               qulonglong sourceOffset,
                               QList<BlockRow>* rows,
                               QString* errorMessage)
{
    const QByteArray encoded = source.toUtf8();
    WardError* rawError = nullptr;
    OwnedWardBuffer buffer(ward_core_markup_parse(toWireFormat(format),
                                                  reinterpret_cast<const std::uint8_t*>(encoded.constData()),
                                                  static_cast<std::size_t>(encoded.size()),
                                                  &rawError));
    if (!buffer) {
        *errorMessage = ward::coreffi::takeErrorMessage(rawError);
        if (errorMessage->isEmpty())
            *errorMessage = QStringLiteral("Ward Core returned no markup document.");
        return false;
    }

    const std::size_t bufferSize = ward_core_owned_buffer_size(buffer.get());
    if (bufferSize > static_cast<std::size_t>(std::numeric_limits<qsizetype>::max())) {
        *errorMessage = QStringLiteral("The serialized markup document is too large.");
        return false;
    }
    const QByteArrayView bytes(reinterpret_cast<const char*>(ward_core_owned_buffer_data(buffer.get())),
                               static_cast<qsizetype>(bufferSize));
    ward::markup::v1::Document document;
    QProtobufSerializer serializer;
    if (!document.deserialize(&serializer, bytes)) {
        *errorMessage = QStringLiteral("Failed to decode the markup document: %1").arg(serializer.lastErrorString());
        return false;
    }

    const bool markdown = format == SourceFormat::Markdown;
    rows->reserve(document.blocks().size());
    for (const WireBlock& block : document.blocks()) {
        const qulonglong sourceStart = sourceOffset + block.sourceStart();
        const qulonglong sourceEnd = sourceOffset + block.sourceEnd();
        if (block.hasProse()) {
            const auto& prose = block.prose();
            rows->append(BlockRow{
              .blockId = QStringLiteral("prose:%1").arg(sourceStart),
              .kind = BlockKind::Prose,
              .sourceStart = sourceStart,
              .sourceEnd = sourceEnd,
              .text = prose.source(),
              .plainText = prose.plainText(),
              .markdown = markdown,
            });
        } else if (block.hasCodeBlock()) {
            const auto& code = block.codeBlock();
            rows->append(BlockRow{
              .blockId = QStringLiteral("code:%1").arg(sourceStart),
              .kind = BlockKind::Code,
              .sourceStart = sourceStart,
              .sourceEnd = sourceEnd,
              .text = code.code(),
              .plainText = code.code(),
              .language = code.hasLanguage() ? code.language() : QString(),
              .markdown = false,
            });
        }
    }
    return true;
}

MarkupDocumentModel::ParseResult
MarkupDocumentModel::parseRequest(ParseRequest request)
{
    ParseResult result{
        .generation = request.generation,
        .source = std::move(request.source),
        .format = request.format,
        .finalized = request.finalized,
        .rows = std::move(request.retainedRows),
    };

    const QByteArray encoded = result.source.toUtf8();
    if (request.sourceOffset > static_cast<qulonglong>(encoded.size())) {
        result.errorMessage = QStringLiteral("The incremental markup source offset is invalid.");
        return result;
    }
    const QByteArray suffixBytes = encoded.sliced(static_cast<qsizetype>(request.sourceOffset));
    const QString suffix = QString::fromUtf8(suffixBytes);
    QList<BlockRow> suffixRows;
    result.parsed = parseRows(suffix, result.format, request.sourceOffset, &suffixRows, &result.errorMessage);
    if (!result.parsed)
        suffixRows = fallbackRows(suffix, result.format, request.sourceOffset);
    result.rows.reserve(result.rows.size() + suffixRows.size());
    for (BlockRow& row : suffixRows)
        result.rows.append(std::move(row));
    return result;
}

MarkupDocumentModel::ParseRequest
MarkupDocumentModel::makeParseRequest() const
{
    ParseRequest request{
        .generation = requestedGeneration_,
        .source = requestedSource_,
        .format = requestedFormat_,
        .finalized = requestedFinalized_,
    };
    if (requestedFinalized_ || !hasAppliedSource_ || appliedFormat_ != requestedFormat_ || rows_.isEmpty())
        return request;

    const QByteArray appliedBytes = appliedSource_.toUtf8();
    const QByteArray requestedBytes = requestedSource_.toUtf8();
    const qsizetype sharedSize = std::min(appliedBytes.size(), requestedBytes.size());
    qsizetype sharedPrefix = 0;
    while (sharedPrefix < sharedSize && appliedBytes.at(sharedPrefix) == requestedBytes.at(sharedPrefix))
        ++sharedPrefix;

    qsizetype firstAffectedRow = 0;
    while (firstAffectedRow < rows_.size() &&
           rows_.at(firstAffectedRow).sourceEnd <= static_cast<qulonglong>(sharedPrefix)) {
        ++firstAffectedRow;
    }
    const qsizetype restartRow =
      firstAffectedRow >= rows_.size() ? rows_.size() - 1 : std::max<qsizetype>(0, firstAffectedRow - 1);
    const qulonglong sourceOffset = rows_.at(restartRow).sourceStart;
    if (sourceOffset > static_cast<qulonglong>(sharedPrefix) ||
        sourceOffset > static_cast<qulonglong>(requestedBytes.size())) {
        return request;
    }

    request.sourceOffset = sourceOffset;
    request.retainedRows = rows_.mid(0, restartRow);
    return request;
}

void
MarkupDocumentModel::scheduleParse()
{
    if (requestedFormat_ != SourceFormat::Markdown || parseWatcher_.isRunning() || parseTimer_.isActive())
        return;
    parseTimer_.start(requestedFinalized_ ? 0 : STREAM_PARSE_INTERVAL_MILLISECONDS);
}

void
MarkupDocumentModel::dispatchParse()
{
    if (requestedFormat_ != SourceFormat::Markdown || parseWatcher_.isRunning())
        return;
    ParseRequest request = makeParseRequest();
    parseWatcher_.setFuture(
      QtConcurrent::run([request = std::move(request)]() mutable { return parseRequest(std::move(request)); }));
}

void
MarkupDocumentModel::applyFinishedParse()
{
    ParseResult result = parseWatcher_.result();
    if (result.generation == requestedGeneration_) {
        if (!result.parsed)
            qWarning().noquote() << "Failed to parse a markup document:" << result.errorMessage;
        appliedSource_ = result.source;
        appliedFormat_ = result.format;
        appliedFinalized_ = result.finalized;
        hasAppliedSource_ = true;
        reconcileRows(std::move(result.rows));
        emit documentReconciled();
    }

    const bool requestIsApplied = hasAppliedSource_ && appliedSource_ == requestedSource_ &&
                                  appliedFormat_ == requestedFormat_ && appliedFinalized_ == requestedFinalized_;
    if (!requestIsApplied)
        scheduleParse();
}

void
MarkupDocumentModel::reconcileRows(QList<BlockRow> rows)
{
    const qsizetype sharedSize = std::min(rows_.size(), rows.size());
    qsizetype commonPrefix = 0;
    while (commonPrefix < sharedSize && rows_.at(commonPrefix).blockId == rows.at(commonPrefix).blockId)
        ++commonPrefix;

    qsizetype firstChanged = -1;
    qsizetype lastChanged = -1;
    for (qsizetype index = 0; index < commonPrefix; ++index) {
        if (rows_.at(index) == rows.at(index))
            continue;
        rows_[index] = std::move(rows[index]);
        if (firstChanged < 0)
            firstChanged = index;
        lastChanged = index;
    }
    if (firstChanged >= 0)
        emit dataChanged(this->index(firstChanged), this->index(lastChanged));

    if (commonPrefix < rows_.size()) {
        beginRemoveRows({}, commonPrefix, rows_.size() - 1);
        rows_.remove(commonPrefix, rows_.size() - commonPrefix);
        endRemoveRows();
    }
    if (commonPrefix < rows.size()) {
        beginInsertRows({}, commonPrefix, rows.size() - 1);
        for (qsizetype index = commonPrefix; index < rows.size(); ++index)
            rows_.append(std::move(rows[index]));
        endInsertRows();
    }
}
