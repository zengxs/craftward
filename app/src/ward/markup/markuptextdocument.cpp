// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/markup/markuptextdocument.h"

#include "markuplistmetrics.h"
#include "markuprenderplan.h"

#include <QFontInfo>
#include <QTextBlock>
#include <QTextBoundaryFinder>
#include <QTextCursor>
#include <QTextList>

#include <algorithm>
#include <cmath>

namespace {
QFont
scaledFont(QFont font, qreal scale)
{
    if (font.pixelSize() > 0)
        font.setPixelSize(qRound(font.pixelSize() * scale));
    else
        font.setPointSizeF(font.pointSizeF() * scale);
    return font;
}
}

MarkupTextDocument::MarkupTextDocument(QObject* parent)
  : QObject(parent)
{
    connect(this, &MarkupTextDocument::styleChanged, this, &MarkupTextDocument::render);
}

QQuickTextDocument*
MarkupTextDocument::textDocument() const
{
    return document_;
}

void
MarkupTextDocument::setTextDocument(QQuickTextDocument* document)
{
    if (document_ == document)
        return;
    if (document_)
        disconnect(document_, nullptr, this, nullptr);
    document_ = document;
    if (document_)
        connect(document_, &QQuickTextDocument::textDocumentChanged, this, &MarkupTextDocument::render);
    render();
    emit textDocumentChanged();
}

QVariant
MarkupTextDocument::surface() const
{
    return surface_;
}

void
MarkupTextDocument::setSurface(const QVariant& surface)
{
    if (surface_ == surface)
        return;
    surface_ = surface;
    rebuildPositionMap();
    render();
    emit surfaceChanged();
}

bool
MarkupTextDocument::hasInlineCode() const
{
    for (const auto& block : surface_.value<MarkupTextSurface>().blocks)
        for (const auto& run : block.runs)
            if (run.code && !run.text.isEmpty())
                return true;
    return false;
}

void
MarkupTextDocument::classBegin()
{
    complete_ = false;
}

void
MarkupTextDocument::componentComplete()
{
    complete_ = true;
    render();
}

void
MarkupTextDocument::render()
{
    if (!complete_ || !document_ || !document_->textDocument() || !surface_.canConvert<MarkupTextSurface>())
        return;
    auto* document = document_->textDocument();
    document->setUndoRedoEnabled(false);
    // TextEdit measures on contentsChanged, so publish only the completed layout.
    // Disabling layout still emits content changes and leaves its height stale.
    QTextCursor transaction(document);
    transaction.beginEditBlock();
    transaction.select(QTextCursor::Document);
    transaction.removeSelectedText();
    transaction.setBlockFormat(QTextBlockFormat());
    transaction.setCharFormat(QTextCharFormat());
    document->setDefaultFont(font_);
    document->setDocumentMargin(0);
    document->setIndentWidth(listIndentWidth_ > 0 ? listIndentWidth_ : markupListIndentWidth(surface_, font_));
    const auto surface = surface_.value<MarkupTextSurface>();
    QHash<QString, QTextList*> lists;
    bool first = true;
    for (const auto& block : surface.blocks) {
        qreal scale = 1.0;
        for (const auto& run : block.runs)
            scale = std::max(scale, run.scale);
        const int fontPixelSize = QFontInfo(scaledFont(font_, scale)).pixelSize();
        auto blockFormat = block.format;
        // Use the font size rather than multiplying the font's built-in leading.
        // Taller fallback glyphs may still expand the line to avoid clipping.
        blockFormat.setLineHeight(std::ceil(fontPixelSize * lineHeightScale_), QTextBlockFormat::MinimumHeight);
        if (first)
            transaction.setBlockFormat(blockFormat);
        else
            transaction.insertBlock(blockFormat, QTextCharFormat());
        first = false;
        QTextCharFormat markerFormat;
        // Keep native numbering and indentation; MarkupListMarkers draws the marker.
        if (!block.listKey.isEmpty())
            markerFormat.setForeground(Qt::transparent);
        if (!block.listKey.isEmpty() && block.list.style() == QTextListFormat::ListDisc &&
            blockFormat.marker() == QTextBlockFormat::MarkerType::NoMarker) {
            // Enlarge the bullet without shifting its baseline with a larger font.
            markerFormat.setFontWeight(QFont::Black);
        }
        transaction.setBlockCharFormat(markerFormat);
        if (!block.listKey.isEmpty()) {
            auto* list = lists.value(block.listKey);
            if (list)
                list->add(transaction.block());
            else
                lists.insert(block.listKey, transaction.createList(block.list));
        }
        for (const auto& run : block.runs) {
            QTextCharFormat format;
            format.setFont(scaledFont(font_, run.scale));
            format.setForeground(textColor_);
            format.merge(run.format);
            if (run.code) {
                format.setFontFamilies(codeFont_.families());
                format.setFontFixedPitch(true);
                // The background item decorates native glyph ranges without changing the text.
                format.setProperty(InlineCodeProperty, true);
            } else if (format.fontWeight() >= QFont::Bold) {
                // Prefer a real Chinese bold face within emphasized prose.
                auto families = format.fontFamilies().toStringList();
                const auto chineseFamily = QStringLiteral("PingFang SC");
                if (!families.contains(chineseFamily))
                    families.append(chineseFamily);
                format.setFontFamilies(families);
            }
            if (run.annotation || format.isAnchor())
                format.setForeground(linkColor_);
            if (run.annotation)
                format.setBackground(annotationBackground_);
            transaction.insertText(run.text, format);
        }
    }
    transaction.endEditBlock();
    emit rendered();
}

QVariantMap
MarkupTextDocument::endpointAt(int position) const
{
    return endpointAtSurfacePosition(surfacePosition(position));
}

void
MarkupTextDocument::rebuildPositionMap()
{
    collapsedLineBreaks_.clear();
    int start = 0;
    // Each insertText call collapses CRLF to one document separator. Keep raw
    // semantic offsets for copying and map only the removed UTF-16 positions.
    for (const auto& run : surface_.value<MarkupTextSurface>().selectionRuns()) {
        for (qsizetype i = 1; i < run.text.size(); ++i) {
            if (run.text.at(i - 1) == QLatin1Char('\r') && run.text.at(i) == QLatin1Char('\n')) {
                const int position = start + int(i);
                collapsedLineBreaks_.append({ position, position - int(collapsedLineBreaks_.size()) });
            }
        }
        start += run.text.size();
    }
}

int
MarkupTextDocument::documentPosition(int position) const
{
    const auto end = std::upper_bound(
      collapsedLineBreaks_.cbegin(),
      collapsedLineBreaks_.cend(),
      position,
      [](int offset, const CollapsedLineBreak& lineBreak) { return offset < lineBreak.surfacePosition; });
    return position - int(end - collapsedLineBreaks_.cbegin());
}

int
MarkupTextDocument::surfacePosition(int position) const
{
    const auto end = std::upper_bound(
      collapsedLineBreaks_.cbegin(),
      collapsedLineBreaks_.cend(),
      position,
      [](int offset, const CollapsedLineBreak& lineBreak) { return offset < lineBreak.documentPosition; });
    return position + int(end - collapsedLineBreaks_.cbegin());
}

QVariantMap
MarkupTextDocument::endpointAtSurfacePosition(int position) const
{
    const auto surface = surface_.value<MarkupTextSurface>();
    const auto text = surface.text();
    position = std::clamp(position, 0, int(text.size()));
    QTextBoundaryFinder boundary(QTextBoundaryFinder::Grapheme, text);
    boundary.setPosition(position);
    if (!boundary.isAtBoundary())
        position = boundary.toPreviousBoundary();
    int start = 0;
    const auto runs = surface.selectionRuns();
    for (qsizetype i = 0; i < runs.size(); ++i) {
        const auto& run = runs[i];
        const int end = start + run.text.size();
        if (position < end || i + 1 == runs.size())
            return { { QStringLiteral("key"), run.key },
                     { QStringLiteral("offset"), std::clamp(position - start, 0, int(run.text.size())) } };
        start = end;
    }
    return {};
}

QVariantMap
MarkupTextDocument::wordAt(int position) const
{
    const auto text = surface_.value<MarkupTextSurface>().text();
    position = std::clamp(surfacePosition(position), 0, int(text.size()));
    QTextBoundaryFinder boundary(QTextBoundaryFinder::Word, text);
    boundary.setPosition(position);
    int start = position;
    if (!(boundary.boundaryReasons() & QTextBoundaryFinder::StartOfItem))
        start = boundary.toPreviousBoundary();
    boundary.setPosition(position);
    int end = boundary.toNextBoundary();
    return { { QStringLiteral("start"), endpointAtSurfacePosition(std::max(0, start)) },
             { QStringLiteral("end"), endpointAtSurfacePosition(end < 0 ? text.size() : end) } };
}
