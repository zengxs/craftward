// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "markupselection.h"

#include <QClipboard>
#include <QGuiApplication>
#include <QTextBoundaryFinder>

#include <algorithm>

MarkupSelection::MarkupSelection(QObject* parent)
  : QObject(parent)
{
}

void
MarkupSelection::reconcile(const QList<MarkupTextSurface>& surfaces)
{
    runs_.clear();
    indices_.clear();
    text_.clear();
    bool first = true;
    for (const auto& surface : surfaces) {
        if (!first)
            text_ += surface.separator;
        first = false;
        for (const auto& run : surface.selectionRuns()) {
            indices_.insert(run.key, runs_.size());
            runs_.append({ run.key, int(text_.size()), int(run.text.size()) });
            text_ += run.text;
        }
    }
    text_.replace(QChar::LineSeparator, QLatin1Char('\n'));
    boundaries_ = QTextBoundaryFinder(QTextBoundaryFinder::Grapheme, text_);
    if (position(anchor_) < 0 || position(focus_) < 0) {
        anchor_.clear();
        focus_.clear();
    } else {
        anchor_ = normalize(anchor_);
        focus_ = normalize(focus_);
    }
    emit changed();
}

int
MarkupSelection::position(const QVariantMap& value) const
{
    const auto found = indices_.constFind(value.value(QStringLiteral("key")).toString());
    if (found == indices_.cend())
        return -1;
    const auto& run = runs_[found.value()];
    return run.start + std::clamp(value.value(QStringLiteral("offset")).toInt(), 0, run.length);
}

QVariantMap
MarkupSelection::endpoint(int position) const
{
    if (runs_.isEmpty() || position < 0)
        return {};
    position = std::clamp(position, 0, int(text_.size()));
    boundaries_.setPosition(position);
    if (!boundaries_.isAtBoundary())
        position = boundaries_.toPreviousBoundary();
    const auto found = std::upper_bound(
      runs_.cbegin(), runs_.cend(), position, [](int offset, const Run& run) { return offset < run.start; });
    const auto& run = found == runs_.cbegin() ? runs_.first() : *std::prev(found);
    return { { QStringLiteral("key"), run.key },
             { QStringLiteral("offset"), std::clamp(position - run.start, 0, run.length) } };
}

QVariantMap
MarkupSelection::normalize(const QVariantMap& value) const
{
    int offset = position(value);
    if (offset < 0)
        return {};
    boundaries_.setPosition(offset);
    if (!boundaries_.isAtBoundary())
        offset = boundaries_.toPreviousBoundary();
    const auto& run = runs_[indices_.value(value.value(QStringLiteral("key")).toString())];
    // Keep the surviving identity at a shared boundary when a new sibling appears.
    if (offset >= run.start && offset <= run.start + run.length)
        return { { QStringLiteral("key"), run.key }, { QStringLiteral("offset"), offset - run.start } };
    return endpoint(offset);
}

bool
MarkupSelection::hasSelection() const
{
    return position(anchor_) >= 0 && position(focus_) >= 0 && position(anchor_) != position(focus_);
}

void
MarkupSelection::begin(const QVariantMap& value)
{
    const auto next = normalize(value);
    if (next.isEmpty())
        return;
    anchor_ = next;
    focus_ = next;
    emit changed();
}

void
MarkupSelection::extend(const QVariantMap& value)
{
    const auto next = normalize(value);
    if (next.isEmpty())
        return;
    if (anchor_.isEmpty())
        anchor_ = next;
    if (focus_ == next)
        return;
    focus_ = next;
    emit changed();
}

void
MarkupSelection::clear()
{
    if (anchor_.isEmpty() && focus_.isEmpty())
        return;
    anchor_.clear();
    focus_.clear();
    emit changed();
}

void
MarkupSelection::selectAll()
{
    anchor_ = endpoint(0);
    focus_ = endpoint(text_.size());
    emit changed();
}

QVariantMap
MarkupSelection::range(const QVariant& value) const
{
    const auto surface = value.value<MarkupTextSurface>();
    const auto runs = surface.selectionRuns();
    const auto found = runs.isEmpty() ? indices_.cend() : indices_.constFind(runs.first().key);
    if (!hasSelection() || found == indices_.cend())
        return { { QStringLiteral("start"), 0 }, { QStringLiteral("end"), 0 } };
    const int start = runs_[found.value()].start;
    const int length = surface.text().size();
    const int anchor = position(anchor_);
    const int focus = position(focus_);
    return { { QStringLiteral("start"), std::clamp(std::min(anchor, focus) - start, 0, length) },
             { QStringLiteral("end"), std::clamp(std::max(anchor, focus) - start, 0, length) } };
}

QString
MarkupSelection::text() const
{
    if (!hasSelection())
        return {};
    const int anchor = position(anchor_);
    const int focus = position(focus_);
    return text_.mid(std::min(anchor, focus), qAbs(focus - anchor));
}

void
MarkupSelection::copy() const
{
    if (hasSelection())
        QGuiApplication::clipboard()->setText(text());
}
