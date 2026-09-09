// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "markupcodebackground.h"

#include "markuptextdocument.h"

#include <QGlyphRun>
#include <QTextBlock>
#include <QTextDocument>
#include <QTextLayout>

#include <algorithm>

namespace {
QVariantList
codeRectangles(QTextDocument* document, const QPointF& origin, qreal padding)
{
    QVariantList rectangles;
    for (auto block = document->begin(); block.isValid(); block = block.next()) {
        const auto* layout = block.layout();
        if (!layout || !block.isVisible())
            continue;
        const auto text = block.text();
        for (int i = 0; i < layout->lineCount(); ++i) {
            const auto line = layout->lineAt(i);
            QList<QRectF> spans;
            for (auto it = block.begin(); !it.atEnd(); ++it) {
                const auto fragment = it.fragment();
                const auto format = fragment.charFormat();
                if (!format.boolProperty(MarkupTextDocument::InlineCodeProperty))
                    continue;
                const int from = std::max(fragment.position() - block.position(), line.textStart());
                const int to = std::min(fragment.position() + fragment.length() - block.position(),
                                        line.textStart() + line.textLength());
                if (from >= to)
                    continue;
                // Native visual ranges preserve bidi ordering, font fallback, and partial ligatures.
                for (const auto& run : line.glyphRuns(from, to - from, QTextLayout::RetrieveGlyphPositions)) {
                    const auto bounds = run.boundingRect();
                    // Emoji font bounds can exceed the line box; use the native line height.
                    spans.append(QRectF(bounds.x(), line.y(), bounds.width(), line.height()));
                }
                // Tabs reserve width but do not appear in glyphRuns().
                for (int position = text.indexOf(QLatin1Char('\t'), from); position >= 0 && position < to;
                     position = text.indexOf(QLatin1Char('\t'), position + 1)) {
                    const qreal first = line.cursorToX(position, QTextLine::Leading);
                    const qreal last = line.cursorToX(position, QTextLine::Trailing);
                    spans.append(QRectF(std::min(first, last), line.y(), qAbs(last - first), line.height()));
                }
            }
            std::sort(spans.begin(), spans.end(), [](const QRectF& a, const QRectF& b) { return a.left() < b.left(); });
            QList<QRectF> merged;
            for (const auto& span : spans) {
                if (span.isEmpty())
                    continue;
                if (!merged.isEmpty() && span.left() <= merged.last().right() + 0.01)
                    merged.last() = merged.last().united(span);
                else
                    merged.append(span);
            }
            for (const auto& rectangle : merged)
                rectangles.append(rectangle.translated(layout->position() + origin).adjusted(0, -padding, 0, padding));
        }
    }
    return rectangles;
}
}

MarkupCodeBackground::MarkupCodeBackground(QQuickItem* parent)
  : MarkupTextBackground(parent)
{
}

qreal
MarkupCodeBackground::verticalPadding() const
{
    return verticalPadding_;
}

void
MarkupCodeBackground::setVerticalPadding(qreal padding)
{
    padding = std::max(qreal(0), padding);
    if (verticalPadding_ == padding)
        return;
    verticalPadding_ = padding;
    scheduleLayout();
    emit verticalPaddingChanged();
}

QVariantList
MarkupCodeBackground::rectangles() const
{
    return rectangles_;
}

void
MarkupCodeBackground::updatePolish()
{
    QVariantList rectangles;
    if (const auto geometry = documentGeometry())
        rectangles = codeRectangles(geometry->document, geometry->origin, verticalPadding_);
    if (rectangles_ != rectangles) {
        rectangles_ = std::move(rectangles);
        emit rectanglesChanged();
    }
}
