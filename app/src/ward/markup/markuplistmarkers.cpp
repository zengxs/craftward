// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "markuplistmarkers.h"

#include "markuplistmetrics.h"

#include <QQuickWindow>
#include <QTextBlock>
#include <QTextDocument>
#include <QTextList>

namespace {
QString
markerText(const QTextBlock& block)
{
    switch (block.blockFormat().marker()) {
        case QTextBlockFormat::MarkerType::Checked:
            return QString(QChar(0x2612));
        case QTextBlockFormat::MarkerType::Unchecked:
            return QString(QChar(0x2610));
        case QTextBlockFormat::MarkerType::NoMarker:
            break;
    }
    switch (block.textList()->format().style()) {
        case QTextListFormat::ListDisc:
            return QString(QChar(0x2022));
        case QTextListFormat::ListCircle:
            return QString(QChar(0x25e6));
        case QTextListFormat::ListSquare:
            return QString(QChar(0x25aa));
        default:
            return block.textList()->itemText(block);
    }
}
}

MarkupListMarkers::MarkupListMarkers(QQuickItem* parent)
  : MarkupTextBackground(parent, { "color", "renderType" })
{
    setFlag(ItemHasContents);
}

void
MarkupListMarkers::updatePolish()
{
    markers_.clear();
    if (const auto geometry = documentGeometry()) {
        color_ = textEdit()->property("color").value<QColor>();
        renderType_ = QSGTextNode::RenderType(textEdit()->property("renderType").toInt());
        for (auto block = geometry->document->begin(); block.isValid(); block = block.next()) {
            const auto* body = block.layout();
            if (!block.isVisible() || !block.textList() || !body || body->lineCount() == 0)
                continue;
            const auto font = block.charFormat().font();
            auto layout = std::make_unique<QTextLayout>(markerText(block), font);
            layout->beginLayout();
            const auto marker = layout->createLine();
            layout->endLayout();
            const auto first = body->lineAt(0);
            const bool ordered = block.textList()->format().style() == QTextListFormat::ListDecimal;
            const qreal gap = markupListMarkerGap(font, ordered);
            const auto textRect = first.naturalTextRect();
            const qreal x = block.textDirection() == Qt::RightToLeft
                              ? textRect.right() + gap
                              : textRect.left() - gap - marker.horizontalAdvance();
            // Fallback glyphs can raise the body's ascent. Align baselines, not line tops.
            const qreal y = first.y() + first.ascent() - marker.ascent();
            markers_.push_back({ std::move(layout), geometry->origin + body->position() + QPointF(x, y) });
        }
    }
    update();
}

QSGNode*
MarkupListMarkers::updatePaintNode(QSGNode* node, UpdatePaintNodeData*)
{
    auto* text = static_cast<QSGTextNode*>(node);
    if (markers_.empty()) {
        delete text;
        return nullptr;
    }
    if (!text)
        text = window()->createTextNode();
    text->clear();
    text->setColor(color_);
    text->setRenderType(renderType_);
    for (const auto& marker : markers_)
        text->addTextLayout(marker.position, marker.layout.get());
    return text;
}
