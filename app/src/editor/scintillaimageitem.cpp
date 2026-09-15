// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "scintillaimageitem_p.h"

#include <QPainter>
#include <QQuickWindow>
#include <QSGSimpleTextureNode>
#include <QScreen>
#include <QtMath>

#include <cstring>
#include <memory>

namespace {
class ImageNode : public QSGSimpleTextureNode
{
  public:
    std::unique_ptr<QSGTexture> imageTexture;
    quint64 revision = 0;
};
}

ScintillaImageItem::ScintillaImageItem(QQuickItem* parent)
  : QQuickItem(parent)
{
    setFlag(ItemHasContents);
}

void
ScintillaImageItem::invalidateImage(const QRectF& rect)
{
    m_dirty += rect.toAlignedRect().intersected(boundingRect().toAlignedRect());
    polish();
}

void
ScintillaImageItem::invalidateImage()
{
    invalidateImage(boundingRect());
}

void
ScintillaImageItem::scrollImage(int dy)
{
    const qreal ratio = m_image.devicePixelRatio();
    const int pixels = qRound(dy * ratio);
    if (m_image.isNull() || !m_dirty.isEmpty() || !qFuzzyCompare(qreal(pixels), dy * ratio) ||
        qAbs(pixels) >= m_image.height()) {
        invalidateImage();
        return;
    }
    if (!pixels)
        return;
    const qsizetype stride = m_image.bytesPerLine();
    uchar* bits = m_image.bits();
    const qsizetype bytes = (m_image.height() - qAbs(pixels)) * stride;
    if (pixels > 0)
        std::memmove(bits + pixels * stride, bits, bytes);
    else
        std::memmove(bits, bits - pixels * stride, bytes);
    invalidateImage(dy > 0 ? QRectF(0, 0, width(), dy) : QRectF(0, height() + dy, width(), -dy));
}

void
ScintillaImageItem::updatePolish()
{
    if (!window() || width() <= 0 || height() <= 0)
        return;
    const qreal ratio = window()->effectiveDevicePixelRatio();
    const QSize size(qCeil(width() * ratio), qCeil(height() * ratio));
    if (m_image.size() != size || !qFuzzyCompare(m_image.devicePixelRatio(), ratio)) {
        m_image = QImage(size, QImage::Format_ARGB32_Premultiplied);
        if (m_image.isNull())
            return;
        m_image.setDevicePixelRatio(ratio);
        if (const auto* screen = window()->screen()) {
            m_image.setDotsPerMeterX(qRound(screen->logicalDotsPerInchX() / 0.0254));
            m_image.setDotsPerMeterY(qRound(screen->logicalDotsPerInchY() / 0.0254));
        }
        m_image.fill(Qt::transparent);
        m_dirty = boundingRect().toAlignedRect();
    }
    if (m_dirty.isEmpty())
        return;
    const QRect dirty = m_dirty.boundingRect();
    m_dirty = {};
    QPainter painter(&m_image);
    painter.setRenderHint(QPainter::TextAntialiasing);
    const auto repaint = [&](const QRect& rect) {
        painter.save();
        painter.setClipRect(rect);
        // Cached pixels must not accumulate antialiasing at fractional device coordinates.
        painter.setCompositionMode(QPainter::CompositionMode_Source);
        painter.fillRect(rect, Qt::transparent);
        painter.setCompositionMode(QPainter::CompositionMode_SourceOver);
        const bool complete = paintImage(painter, rect);
        painter.restore();
        return complete;
    };
    const bool complete = repaint(dirty);
    if (!complete) {
        // Wrapping or styling may abandon a partial paint. Repair before publishing it.
        m_dirty = {};
        if (!repaint(boundingRect().toAlignedRect()))
            invalidateImage();
    }
    painter.end();
    ++m_revision;
    update();
}

QSGNode*
ScintillaImageItem::updatePaintNode(QSGNode* oldNode, UpdatePaintNodeData*)
{
    if (m_image.isNull()) {
        delete oldNode;
        return nullptr;
    }
    auto* node = static_cast<ImageNode*>(oldNode);
    if (!node)
        node = new ImageNode;
    if (node->revision != m_revision || !node->imageTexture) {
        node->imageTexture.reset(window()->createTextureFromImage(m_image));
        if (!node->imageTexture) {
            delete node;
            return nullptr;
        }
        node->setTexture(node->imageTexture.get());
        node->setFiltering(QSGTexture::Nearest);
        node->revision = m_revision;
    }
    node->setRect(boundingRect());
    const qreal ratio = m_image.devicePixelRatio();
    node->setSourceRect(QRectF(0, 0, width() * ratio, height() * ratio));
    return node;
}

void
ScintillaImageItem::geometryChange(const QRectF& geometry, const QRectF& oldGeometry)
{
    QQuickItem::geometryChange(geometry, oldGeometry);
    if (geometry.size() != oldGeometry.size())
        invalidateImage();
}

void
ScintillaImageItem::itemChange(ItemChange change, const ItemChangeData& data)
{
    QQuickItem::itemChange(change, data);
    if (change == ItemDevicePixelRatioHasChanged || change == ItemSceneChange || change == ItemVisibleHasChanged)
        invalidateImage();
}
