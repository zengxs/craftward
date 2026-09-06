// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/markup/markupsemanticmodel.h"

#include "ward/coreffierror.h"

#include <ward_core.h>

#include <QDebug>
#include <QtConcurrent/QtConcurrentRun>
#include <QtProtobuf/QProtobufSerializer>
#include <QtProtobuf/qprotobufregistration.h>

#include <algorithm>
#include <limits>
#include <memory>

namespace {
using namespace ward::markup::v1;
using TextKind = TextKindGadget::TextKind;
constexpr qsizetype MAX_GROUP_BLOCKS = 8;
constexpr qsizetype MAX_GROUP_CHILDREN = 16;
constexpr quint64 TARGET_GROUP_BYTES = 8 * 1024;

bool
supported(const SemanticBlock& block)
{
    for (const auto& node : block.nodes()) {
        if (node.hasImage() || node.hasFootnoteDefinition() || node.hasFootnoteReference() || node.hasAdmonitionKind())
            return false;
        if (node.hasText() && node.text().kind() == TextKind::TEXT_KIND_UNSUPPORTED)
            return false;
        if (node.hasList() && node.list().hasStart() && node.list().start() > std::numeric_limits<int>::max())
            return false;
    }
    return true;
}

QString
decodedText(const SemanticBlock& block)
{
    QString text;
    for (const auto& node : block.nodes()) {
        if (node.hasText())
            text += node.text().value().text();
        else if (node.hasAnnotation())
            text += node.annotation().label().text();
    }
    return text;
}

// Split at structural boundaries. The copied outer container describes only this
// segment, so an appended sibling does not invalidate earlier segment payloads.
QList<SemanticBlock>
splitBlock(const SemanticBlock& block)
{
    const auto& nodes = block.nodes();
    if (nodes.isEmpty() || (!nodes.first().hasTable() && !nodes.first().hasList()))
        return { block };

    QList<qsizetype> children;
    for (qsizetype index = 1; index < nodes.size(); ++index) {
        if (nodes[index].hasParentIndex() && nodes[index].parentIndex() == 0)
            children.append(index);
    }
    QList<SemanticBlock> segments;
    for (qsizetype child = 0; child < children.size();) {
        qsizetype next = child + 1;
        while (next < children.size() && next - child < MAX_GROUP_CHILDREN &&
               nodes[children[next]].source().end() - nodes[children[child]].source().start() <= TARGET_GROUP_BYTES)
            ++next;
        const qsizetype first = children[child];
        const qsizetype end = next < children.size() ? children[next] : nodes.size();
        SemanticBlock segment = block;
        auto range = nodes[first].source();
        range.setEnd(nodes[children[next - 1]].source().end());
        segment.setSource(range);
        auto outer = nodes.first();
        outer.setSource(segment.source());
        if (outer.hasList() && outer.list().hasStart()) {
            auto list = outer.list();
            list.setStart(list.start() + child);
            outer.setList(list);
        }
        QList<SemanticNode> selected{ outer };
        for (qsizetype index = first; index < end; ++index) {
            auto node = nodes[index];
            node.setParentIndex(node.parentIndex() == 0 ? 0 : node.parentIndex() - first + 1);
            selected.append(std::move(node));
        }
        segment.setNodes(selected);
        segments.append(std::move(segment));
        child = next;
    }
    return segments;
}
}

MarkupSemanticModel::MarkupSemanticModel(QObject* parent)
  : QAbstractListModel(parent)
{
    // Generated enum registrars may run after the static message registration.
    // Drain them before workers deserialize repeated enum fields such as columns.
    qRegisterProtobufTypes();
    timer_.setSingleShot(true);
    connect(&timer_, &QTimer::timeout, this, &MarkupSemanticModel::dispatch);
    connect(&watcher_, &QFutureWatcher<Result>::finished, this, &MarkupSemanticModel::applyFinished);
}

int
MarkupSemanticModel::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : segments_.size();
}

QVariant
MarkupSemanticModel::data(const QModelIndex& index, int role) const
{
    if (!index.isValid() || index.parent().isValid() || index.row() < 0 || index.row() >= segments_.size())
        return {};
    const auto& segment = segments_[index.row()];
    switch (role) {
        case SegmentIdRole:
            return segment.id;
        case CodeBlockRole:
            return segment.codeBlock;
        case SegmentTextRole:
        case PlainTextRole:
            return segment.text;
        case LanguageRole:
            return segment.language;
        case MarkdownRole:
            return false;
        case SemanticSegmentRole:
            return segment.semantic;
        default:
            return {};
    }
}

QHash<int, QByteArray>
MarkupSemanticModel::roleNames() const
{
    return { { SegmentIdRole, "segmentId" },
             { CodeBlockRole, "codeBlock" },
             { SegmentTextRole, "segmentText" },
             { PlainTextRole, "plainText" },
             { LanguageRole, "language" },
             { MarkdownRole, "markdown" },
             { SemanticSegmentRole, "semanticSegment" } };
}

void
MarkupSemanticModel::reconcileSource(const QString& source, MarkupDocumentModel::SourceFormat format, bool finalized)
{
    if (source_ == source && format_ == format && finalized_ == finalized)
        return;
    source_ = source;
    format_ = format;
    finalized_ = finalized;
    ++generation_;
    schedule();
}

void
MarkupSemanticModel::schedule()
{
    if (!timer_.isActive() && !watcher_.isRunning() && appliedGeneration_ != generation_)
        timer_.start(finalized_ ? 0 : 32);
}

void
MarkupSemanticModel::dispatch()
{
    // Workers own value snapshots only; document teardown never waits for them.
    watcher_.setFuture(QtConcurrent::run(
      [generation = generation_, source = source_, format = format_] { return parse(generation, source, format); }));
}

MarkupSemanticModel::Result
MarkupSemanticModel::parse(quint64 generation, const QString& source, MarkupDocumentModel::SourceFormat format)
{
    Result result{ .generation = generation };
    const QByteArray encoded = source.toUtf8();
    WardError* error = nullptr;
    const auto wireFormat = format == MarkupDocumentModel::SourceFormat::Markdown ? WardMarkupSourceFormatMarkdown
                                                                                  : WardMarkupSourceFormatPlainText;
    const std::unique_ptr<WardOwnedBuffer, decltype(&ward_core_owned_buffer_destroy)> buffer(
      ward_core_markup_parse_semantic(
        wireFormat, reinterpret_cast<const uint8_t*>(encoded.constData()), encoded.size(), &error),
      &ward_core_owned_buffer_destroy);
    SemanticDocument document;
    QProtobufSerializer serializer;
    if (!buffer) {
        result.error = ward::coreffi::takeErrorMessage(error);
        if (result.error.isEmpty())
            result.error = QStringLiteral("Ward Core returned no semantic document.");
    } else if (ward_core_owned_buffer_size(buffer.get()) > std::numeric_limits<qsizetype>::max()) {
        result.error = QStringLiteral("The semantic document is too large.");
    } else if (!document.deserialize(
                 &serializer,
                 QByteArrayView(reinterpret_cast<const char*>(ward_core_owned_buffer_data(buffer.get())),
                                ward_core_owned_buffer_size(buffer.get())))) {
        result.error = serializer.lastErrorString();
    }
    if (!result.error.isEmpty()) {
        if (!source.isEmpty())
            result.segments.append(Segment{ .id = QStringLiteral("fallback:0"), .text = source });
        return result;
    }

    QList<SemanticBlock> group;
    quint64 groupStart = 0;
    const auto flushGroup = [&] {
        if (group.isEmpty())
            return;
        SemanticDocument payload;
        payload.setSourceFormat(document.sourceFormat());
        payload.setBlocks(group);
        QString text;
        for (const auto& part : group) {
            if (!text.isEmpty())
                text += QStringLiteral("\n\n");
            text += decodedText(part);
        }
        QString id = group.first().blockId();
        const auto nodes = group.first().nodes();
        if (nodes.first().hasList() || nodes.first().hasTable())
            id += QLatin1Char('/') + nodes.at(1).nodeId();
        result.segments.append(Segment{ .id = id, .text = text, .semantic = QVariant::fromValue(payload) });
        group.clear();
    };
    for (const auto& block : document.blocks()) {
        const auto parts = splitBlock(block);
        for (const auto& part : parts) {
            if (part.nodes().isEmpty())
                continue;
            const auto root = part.nodes().first();
            Segment segment{ .id = block.blockId(), .codeBlock = root.hasCodeBlock() };
            if (segment.codeBlock) {
                flushGroup();
                segment.text = decodedText(part);
                if (segment.text.endsWith(QLatin1Char('\n')))
                    segment.text.chop(1);
                segment.language = root.codeBlock().hasLanguage() ? root.codeBlock().language() : QString();
            } else if (supported(part)) {
                const bool structuredGroup = root.hasTable() || root.hasList();
                if (structuredGroup || group.size() >= MAX_GROUP_BLOCKS ||
                    (!group.isEmpty() && part.source().end() - groupStart > TARGET_GROUP_BYTES))
                    flushGroup();
                if (group.isEmpty())
                    groupStart = part.source().start();
                group.append(part);
                if (structuredGroup)
                    flushGroup();
                continue;
            } else {
                flushGroup();
                if (root.hasTable() || root.hasList())
                    segment.id += QLatin1Char('/') + part.nodes().at(1).nodeId();
                segment.text =
                  QString::fromUtf8(encoded.sliced(part.source().start(), part.source().end() - part.source().start()));
            }
            result.segments.append(std::move(segment));
        }
    }
    flushGroup();
    return result;
}

void
MarkupSemanticModel::applyFinished()
{
    auto result = watcher_.result();
    if (result.generation == generation_ && result.generation > appliedGeneration_) {
        if (!result.error.isEmpty())
            qWarning().noquote() << "Failed to parse a semantic markup document:" << result.error;
        appliedGeneration_ = result.generation;
        reconcileSegments(std::move(result.segments));
    }
    schedule();
}

void
MarkupSemanticModel::reconcileSegments(QList<Segment> segments)
{
    qsizetype prefix = 0;
    while (prefix < std::min(segments_.size(), segments.size()) && segments_[prefix].id == segments[prefix].id) {
        if (segments_[prefix] != segments[prefix]) {
            segments_[prefix] = std::move(segments[prefix]);
            emit dataChanged(index(prefix), index(prefix));
        }
        ++prefix;
    }
    qsizetype suffix = 0;
    while (suffix < std::min(segments_.size(), segments.size()) - prefix &&
           segments_[segments_.size() - suffix - 1] == segments[segments.size() - suffix - 1])
        ++suffix;
    const qsizetype removed = segments_.size() - prefix - suffix;
    if (removed > 0) {
        beginRemoveRows({}, prefix, prefix + removed - 1);
        segments_.remove(prefix, removed);
        endRemoveRows();
    }
    const qsizetype inserted = segments.size() - prefix - suffix;
    if (inserted > 0) {
        beginInsertRows({}, prefix, prefix + inserted - 1);
        for (qsizetype offset = 0; offset < inserted; ++offset)
            segments_.insert(prefix + offset, std::move(segments[prefix + offset]));
        endInsertRows();
    }
}
