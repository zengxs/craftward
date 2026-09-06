// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "MessageSelection.h"

#include <QClipboard>
#include <QGuiApplication>
#include <QTextBoundaryFinder>

#include <algorithm>

void
MessageSelection::setSegments(const QVariantList& segments)
{
    if (segments_ == segments)
        return;
    segments_ = segments;
    nodes_.clear();
    index_.clear();
    messages_.clear();
    for (const QVariant& value : segments_) {
        const QVariantMap segment = value.toMap();
        const QString message = segment.value("message").toString();
        const int columns = std::max(1, segment.value("columns").toInt());
        const QVariantList parts = segment.value("parts").toList();
        for (int part = 0; part < parts.size(); ++part) {
            QString separator =
              part == 0 ? (messages_.contains(message) ? "\n\n" : "") : (part % columns == 0 ? "\n" : "\t");
            for (const QVariant& item : parts.at(part).toMap().value("nodes").toList()) {
                const QVariantMap node = item.toMap();
                const QString id = node.value("id").toString();
                Q_ASSERT(!index_.contains(id));
                const int ordinal = nodes_.size();
                index_.insert(id, ordinal);
                if (!messages_.contains(message))
                    messages_.insert(message, { ordinal, ordinal });
                else
                    messages_[message].second = ordinal;
                nodes_.append(
                  { id, message, node.value("text").toString(), separator, node.value("kind") == "control" });
                separator.clear();
            }
        }
    }
    clear();
    emit contentChanged();
}

MessageSelection::Endpoint
MessageSelection::resolve(const QVariantMap& endpoint) const
{
    const int ordinal = index_.value(endpoint.value("nodeId").toString(), -1);
    if (ordinal < 0)
        return {};
    const Node& node = nodes_.at(ordinal);
    int offset = std::clamp(endpoint.value("offset").toInt(), 0, node.length());
    if (!node.control) {
        QTextBoundaryFinder boundaries(QTextBoundaryFinder::Grapheme, node.text);
        boundaries.setPosition(offset);
        if (!boundaries.isAtBoundary())
            offset = std::max(0, int(boundaries.toPreviousBoundary()));
    }
    return { ordinal, offset };
}

bool
MessageSelection::before(Endpoint first, Endpoint second)
{
    return first.node < second.node || (first.node == second.node && first.offset < second.offset);
}

QPair<MessageSelection::Endpoint, MessageSelection::Endpoint>
MessageSelection::ordered() const
{
    return before(focus_, anchor_) ? qMakePair(focus_, anchor_) : qMakePair(anchor_, focus_);
}

bool
MessageSelection::hasSelection() const
{
    return anchor_.node >= 0 && anchor_ != focus_;
}

void
MessageSelection::begin(const QVariantMap& endpoint)
{
    const Endpoint next = resolve(endpoint);
    if (next.node < 0)
        return;
    anchor_ = focus_ = next;
    clamped_ = false;
    emit changed();
}

void
MessageSelection::extend(const QVariantMap& endpoint)
{
    if (anchor_.node < 0) {
        begin(endpoint);
        return;
    }
    Endpoint next = resolve(endpoint);
    if (next.node < 0)
        return;
    const QString& message = nodes_.at(anchor_.node).message;
    const bool clamped = nodes_.at(next.node).message != message;
    if (clamped) {
        const auto bounds = messages_.value(message);
        next = next.node < anchor_.node ? Endpoint{ bounds.first, 0 }
                                        : Endpoint{ bounds.second, nodes_.at(bounds.second).length() };
    }
    if (focus_ == next && clamped_ == clamped)
        return;
    focus_ = next;
    clamped_ = clamped;
    emit changed();
}

void
MessageSelection::clear()
{
    anchor_ = focus_ = {};
    clamped_ = false;
    emit changed();
}

void
MessageSelection::selectMessage()
{
    if (anchor_.node < 0)
        return;
    const auto bounds = messages_.value(nodes_.at(anchor_.node).message);
    anchor_ = { bounds.first, 0 };
    focus_ = { bounds.second, nodes_.at(bounds.second).length() };
    clamped_ = false;
    emit changed();
}

QVariantMap
MessageSelection::range(const QVariantList& nodes) const
{
    int first = -1;
    int last = 0;
    int position = 0;
    const auto [low, high] = ordered();
    for (const QVariant& item : nodes) {
        const int ordinal = index_.value(item.toMap().value("id").toString(), -1);
        if (ordinal < 0)
            continue;
        const int length = nodes_.at(ordinal).length();
        if (hasSelection() && ordinal >= low.node && ordinal <= high.node) {
            const int start = ordinal == low.node ? low.offset : 0;
            const int end = ordinal == high.node ? high.offset : length;
            if (end > start) {
                if (first < 0)
                    first = position + start;
                last = position + end;
            }
        }
        position += length;
    }
    return { { "start", std::max(0, first) }, { "end", last } };
}

QString
MessageSelection::text(int limit) const
{
    if (!hasSelection())
        return {};
    const auto [low, high] = ordered();
    QString result;
    const auto append = [&result, limit](const QString& piece) {
        result += limit < 0 ? piece : piece.left(std::max(0, limit - int(result.size())));
    };
    for (int i = low.node; i <= high.node; ++i) {
        const Node& node = nodes_.at(i);
        if (i != low.node)
            append(node.separator);
        const int start = i == low.node ? low.offset : 0;
        const int end = i == high.node ? high.offset : node.length();
        if (end > start) {
            const int count = limit < 0 ? end - start : std::min(end - start, limit - int(result.size()));
            append(node.control ? "[" + node.text + "]" : node.text.mid(start, std::max(0, count)));
        }
        if (limit >= 0 && result.size() >= limit)
            break;
    }
    if (!result.isEmpty() && result.back().isHighSurrogate())
        result.chop(1);
    return result;
}

QString
MessageSelection::copy() const
{
    const QString result = text();
    if (!result.isEmpty())
        QGuiApplication::clipboard()->setText(result);
    return result;
}

QVariantMap
MessageSelection::describe(Endpoint endpoint) const
{
    if (endpoint.node < 0)
        return {};
    const Node& node = nodes_.at(endpoint.node);
    return { { "messageId", node.message }, { "nodeId", node.id }, { "offset", endpoint.offset } };
}

QVariantMap
MessageSelection::state() const
{
    return { { "anchor", describe(anchor_) },
             { "focus", describe(focus_) },
             { "backward", before(focus_, anchor_) },
             { "clampedToMessage", clamped_ },
             { "offsetUnit", "rendered UTF-16, snapped to a grapheme boundary" } };
}
