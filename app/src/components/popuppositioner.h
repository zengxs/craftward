// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QObject>
#include <QQuickItem>
#include <QRectF>
#include <QSizeF>
#include <QtQml/qqmlregistration.h>

class PopupPositioner : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

  public:
    using QObject::QObject;

    /// Fits an anchored popup to its screen and returns geometry relative to its QML parent.
    Q_INVOKABLE QRectF place(QQuickItem* anchor,
                             const QRectF& anchorRect,
                             QQuickItem* relativeTo,
                             const QSizeF& preferredSize) const;

    /// Uses the system cursor because native popups can clear an anchor's hover state.
    Q_INVOKABLE bool containsCursor(QQuickItem* item, const QRectF& rect) const;
};
