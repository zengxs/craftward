// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "markupselectionbackground.h"

#include <QAbstractTextDocumentLayout>
#include <QFontMetricsF>
#include <QGlyphRun>
#include <QMetaProperty>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQmlInfo>
#include <QQuickTextDocument>
#include <QSGSimpleRectNode>
#include <QTextBlock>
#include <QTextCursor>
#include <QTextDocument>
#include <QTextLayout>

#include <algorithm>

namespace {
void
disconnectAll(QList<QMetaObject::Connection>& connections)
{
    for (const auto& connection : connections)
        QObject::disconnect(connection);
    connections.clear();
}

using Decorations = QList<QPair<QRectF, QColor>>;

void
appendDecorations(Decorations& decorations,
                  const QTextBlock& block,
                  const QTextLine& line,
                  int from,
                  int to,
                  const QPointF& origin,
                  const QColor& defaultColor)
{
    QList<int> boundaries{ from, to };
    const auto appendBoundary = [&](int position) {
        if (position > from && position < to)
            boundaries.append(position);
    };
    for (auto it = block.begin(); !it.atEnd(); ++it) {
        const auto fragment = it.fragment();
        appendBoundary(fragment.position() - block.position());
        appendBoundary(fragment.position() + fragment.length() - block.position());
    }
    const auto formats = block.layout()->formats();
    for (const auto& range : formats) {
        appendBoundary(range.start);
        appendBoundary(range.start + range.length);
    }
    std::sort(boundaries.begin(), boundaries.end());
    boundaries.erase(std::unique(boundaries.begin(), boundaries.end()), boundaries.end());
    for (int i = 1; i < boundaries.size(); ++i) {
        const int start = boundaries.at(i - 1);
        QTextCursor cursor(block);
        cursor.setPosition(block.position() + start);
        cursor.movePosition(QTextCursor::NextCharacter, QTextCursor::KeepAnchor);
        auto format = cursor.charFormat();
        for (const auto& range : formats)
            if (range.start <= start && start < range.start + range.length)
                format.merge(range.format);
        const auto color = format.hasProperty(QTextFormat::TextUnderlineColor) ? format.underlineColor()
                           : format.hasProperty(QTextFormat::ForegroundBrush)  ? format.foreground().color()
                                                                               : defaultColor;
        for (const auto& run : line.glyphRuns(start, boundaries.at(i) - start, QTextLayout::RetrieveGlyphPositions)) {
            const auto font = run.rawFont();
            const auto append = [&](qreal offset) {
                auto rectangle = run.boundingRect().translated(block.layout()->position() + origin);
                rectangle.setY(qRound(block.layout()->position().y() + origin.y() + line.y() + line.ascent() + offset));
                rectangle.setHeight(font.lineThickness());
                decorations.append({ rectangle, color });
            };
            if (run.underline())
                append(font.underlinePosition());
            if (run.overline())
                append(-font.ascent());
            if (run.strikeOut())
                append(-font.ascent() / 3);
        }
    }
}

QList<QRectF>
selectionRectangles(QTextDocument* document,
                    int start,
                    int end,
                    const QPointF& origin,
                    const QColor& textColor,
                    Decorations& decorations)
{
    QList<QRectF> rectangles;
    start = std::clamp(start, 0, document->characterCount() - 1);
    end = std::clamp(end, start, document->characterCount() - 1);
    for (auto block = document->findBlock(start); block.isValid() && block.position() < end; block = block.next()) {
        const auto* layout = block.layout();
        if (!layout)
            continue;
        const auto text = block.text();
        const int localStart = std::max(0, start - block.position());
        const int localEnd = std::min(int(text.size()), end - block.position());
        for (int i = 0; i < layout->lineCount(); ++i) {
            const auto line = layout->lineAt(i);
            const int from = std::max(localStart, line.textStart());
            const int to = std::min(localEnd, line.textStart() + line.textLength());
            QList<QRectF> spans;
            if (from < to) {
                // Qt supplies visual ranges, including bidi runs and partial ligatures.
                bool decorated = false;
                for (const auto& run : line.glyphRuns(from, to - from, QTextLayout::RetrieveGlyphPositions)) {
                    const auto bounds = run.boundingRect();
                    spans.append(QRectF(bounds.x(), line.y(), bounds.width(), line.height()));
                    decorated |= run.underline() || run.overline() || run.strikeOut();
                }
                // Transparent native selection also suppresses its text decorations.
                if (decorated)
                    appendDecorations(decorations, block, line, from, to, origin, textColor);
                // Tabs have cursor advances but are omitted from glyphRuns().
                for (int position = text.indexOf(QLatin1Char('\t'), from); position >= 0 && position < to;
                     position = text.indexOf(QLatin1Char('\t'), position + 1)) {
                    const auto first = line.cursorToX(position, QTextLine::Leading);
                    const auto last = line.cursorToX(position, QTextLine::Trailing);
                    spans.append(QRectF(std::min(first, last), line.y(), qAbs(last - first), line.height()));
                }
            }
            // Give selected paragraph separators a visible cell, including empty lines.
            if (i + 1 == layout->lineCount() && block.next().isValid() && end > block.position() + text.size()) {
                const auto width = QFontMetricsF(document->defaultFont()).horizontalAdvance(QLatin1Char(' '));
                const auto x = line.cursorToX(text.size());
                spans.append(
                  QRectF(block.textDirection() == Qt::RightToLeft ? x - width : x, line.y(), width, line.height()));
            }
            std::sort(spans.begin(), spans.end(), [](const QRectF& a, const QRectF& b) { return a.left() < b.left(); });
            QList<QRectF> merged;
            for (const auto& span : spans) {
                if (span.width() <= 0)
                    continue;
                if (!merged.isEmpty() && span.left() <= merged.last().right() + 0.01)
                    merged.last() = merged.last().united(span);
                else
                    merged.append(span);
            }
            for (auto rectangle : merged)
                rectangles.append(rectangle.translated(layout->position() + origin));
        }
    }
    return rectangles;
}
}

MarkupSelectionBackground::MarkupSelectionBackground(QQuickItem* parent)
  : QQuickItem(parent)
{
    setFlag(ItemHasContents);
}

QQuickItem*
MarkupSelectionBackground::textEdit() const
{
    return textEdit_;
}

void
MarkupSelectionBackground::setTextEdit(QQuickItem* item)
{
    if (textEdit_ == item)
        return;
    disconnectAll(itemConnections_);
    textEdit_ = item;
    if (item) {
        const auto slot = metaObject()->method(metaObject()->indexOfSlot("scheduleLayout()"));
        for (const auto* name : { "selectionStart",
                                  "selectionEnd",
                                  "cursorRectangle",
                                  "contentWidth",
                                  "contentHeight",
                                  "textDocument",
                                  "color" }) {
            const auto property = item->metaObject()->property(item->metaObject()->indexOfProperty(name));
            if (property.hasNotifySignal())
                itemConnections_.append(connect(item, property.notifySignal(), this, slot));
        }
        itemConnections_.append(connect(item, &QObject::destroyed, this, [this] {
            scheduleLayout();
            emit textEditChanged();
        }));
    }
    scheduleLayout();
    emit textEditChanged();
}

QColor
MarkupSelectionBackground::color() const
{
    return color_;
}

void
MarkupSelectionBackground::setColor(const QColor& color)
{
    if (color_ == color)
        return;
    color_ = color;
    update();
    emit colorChanged();
}

void
MarkupSelectionBackground::scheduleLayout()
{
    polish();
}

void
MarkupSelectionBackground::observeDocument()
{
    auto* quick = textEdit_ ? textEdit_->property("textDocument").value<QQuickTextDocument*>() : nullptr;
    auto* document = quick ? quick->textDocument() : nullptr;
    auto* layout = document ? document->documentLayout() : nullptr;
    if (quickDocument_ == quick && document_ == document && layout_ == layout)
        return;
    disconnectAll(documentConnections_);
    quickDocument_ = quick;
    document_ = document;
    layout_ = layout;
    if (quick) {
        documentConnections_.append(
          connect(quick, &QQuickTextDocument::textDocumentChanged, this, &MarkupSelectionBackground::scheduleLayout));
        documentConnections_.append(
          connect(quick, &QObject::destroyed, this, &MarkupSelectionBackground::scheduleLayout));
    }
    if (document) {
        documentConnections_.append(
          connect(document, &QTextDocument::contentsChanged, this, &MarkupSelectionBackground::scheduleLayout));
        documentConnections_.append(
          connect(document, &QTextDocument::documentLayoutChanged, this, &MarkupSelectionBackground::scheduleLayout));
        documentConnections_.append(
          connect(document, &QObject::destroyed, this, &MarkupSelectionBackground::scheduleLayout));
    }
    if (layout) {
        documentConnections_.append(
          connect(layout, &QAbstractTextDocumentLayout::update, this, &MarkupSelectionBackground::scheduleLayout));
        documentConnections_.append(
          connect(layout, &QAbstractTextDocumentLayout::updateBlock, this, &MarkupSelectionBackground::scheduleLayout));
        documentConnections_.append(connect(
          layout, &QAbstractTextDocumentLayout::documentSizeChanged, this, &MarkupSelectionBackground::scheduleLayout));
    }
}

void
MarkupSelectionBackground::updatePolish()
{
    observeDocument();
    QList<QRectF> rectangles;
    Decorations decorations;
    if (textEdit_ && document_) {
        const int start = textEdit_->property("selectionStart").toInt();
        const int end = textEdit_->property("selectionEnd").toInt();
        if (start < end) {
            document_->documentLayout()->documentSize();
            const auto* first = document_->firstBlock().layout();
            if (first && first->lineCount() > 0) {
                const auto line = first->lineAt(0);
                QRectF cursor;
                QMetaObject::invokeMethod(
                  textEdit_, "positionToRectangle", Q_RETURN_ARG(QRectF, cursor), Q_ARG(int, 0));
                const auto origin = textEdit_->mapToItem(this, cursor.topLeft()) - first->position() -
                                    QPointF(line.cursorToX(0), line.y());
                rectangles = selectionRectangles(
                  document_, start, end, origin, textEdit_->property("color").value<QColor>(), decorations);
            }
        }
    }
    if (rectangles_ != rectangles || decorations_ != decorations) {
        rectangles_ = std::move(rectangles);
        decorations_ = std::move(decorations);
        updateDecorations();
        update();
    }
}

void
MarkupSelectionBackground::updateDecorations()
{
    if (!decorations_.isEmpty() && !decorationComponent_) {
        // Qt's text decorations use curve strokes; retain the same antialiasing.
        decorationComponent_ = new QQmlComponent(qmlEngine(this), this);
        decorationComponent_->loadFromModule("Craftward.Markup", "MarkupSelectionDecoration");
    }
    for (int i = 0; i < decorations_.size(); ++i) {
        if (i == decorationItems_.size()) {
            auto* item = qobject_cast<QQuickItem*>(decorationComponent_->create());
            if (!item) {
                qmlWarning(this) << decorationComponent_->errorString();
                break;
            }
            item->setParent(this);
            item->setParentItem(this);
            decorationItems_.append(item);
        }
        auto* item = decorationItems_.at(i);
        const auto& [rectangle, color] = decorations_.at(i);
        item->setPosition(rectangle.topLeft());
        item->setSize(rectangle.size());
        item->setProperty("color", color);
    }
    while (decorationItems_.size() > decorations_.size())
        delete decorationItems_.takeLast();
}

QSGNode*
MarkupSelectionBackground::updatePaintNode(QSGNode* node, UpdatePaintNodeData*)
{
    if (!node)
        node = new QSGNode;
    auto* child = node->firstChild();
    const auto append = [&](const QRectF& rectangle, const QColor& color) {
        auto* background = static_cast<QSGSimpleRectNode*>(child);
        if (child)
            child = child->nextSibling();
        else {
            background = new QSGSimpleRectNode;
            node->appendChildNode(background);
        }
        background->setRect(rectangle);
        background->setColor(color);
    };
    for (const auto& rectangle : rectangles_)
        append(rectangle, color_);
    while (child) {
        auto* next = child->nextSibling();
        node->removeChildNode(child);
        delete child;
        child = next;
    }
    return node;
}

void
MarkupSelectionBackground::geometryChange(const QRectF& geometry, const QRectF& previous)
{
    QQuickItem::geometryChange(geometry, previous);
    scheduleLayout();
}
