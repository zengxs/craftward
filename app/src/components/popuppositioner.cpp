// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "popuppositioner.h"

#include <QCursor>
#include <QGuiApplication>
#include <QQuickWindow>
#include <QScreen>

#include <algorithm>

QRectF
PopupPositioner::place(QQuickItem* anchor,
                       const QRectF& anchorRect,
                       QQuickItem* relativeTo,
                       const QSizeF& preferredSize) const
{
    if (!anchor || !anchor->window() || !relativeTo || !relativeTo->window())
        return {};
    auto* content = anchor->window()->contentItem();
    const auto mappedAnchor = anchor->mapRectToItem(content, anchorRect);
    const QRectF globalAnchor(content->mapToGlobal(mappedAnchor.topLeft()), mappedAnchor.size());
    auto* screen = QGuiApplication::screenAt(globalAnchor.center().toPoint());
    if (!screen)
        screen = anchor->window()->screen();
    if (!screen)
        return {};

    constexpr qreal gap = 8;
    constexpr qreal margin = 8;
    const QRectF bounds = QRectF(screen->availableGeometry()).adjusted(margin, margin, -margin, -margin);
    if (bounds.isEmpty())
        return {};
    const qreal width = std::clamp(preferredSize.width(), qreal(1), bounds.width());
    const qreal below = std::max(qreal(0), bounds.bottom() - globalAnchor.bottom() - gap);
    const qreal above = std::max(qreal(0), globalAnchor.top() - gap - bounds.top());
    const bool useBelow = below >= preferredSize.height() || below >= above;
    // Reduce the scrollable viewport when neither side can fit the preferred height.
    const qreal height = std::clamp(
      preferredSize.height(), qreal(1), std::max(qreal(1), std::min(bounds.height(), useBelow ? below : above)));
    const qreal x = std::clamp(globalAnchor.left(), bounds.left(), bounds.right() - width);
    const qreal preferredY = useBelow ? globalAnchor.bottom() + gap : globalAnchor.top() - gap - height;
    const qreal y = std::clamp(preferredY, bounds.top(), bounds.bottom() - height);
    return QRectF(relativeTo->mapFromGlobal(QPointF(x, y)), QSizeF(width, height));
}

bool
PopupPositioner::containsCursor(QQuickItem* item, const QRectF& rect) const
{
    return item && item->window() && item->isVisible() && rect.contains(item->mapFromGlobal(QCursor::pos()));
}
