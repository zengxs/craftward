// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#ifndef CRAFTWARD_SCINTILLAIMAGEITEM_P_H
#define CRAFTWARD_SCINTILLAIMAGEITEM_P_H

#include <QImage>
#include <QQuickItem>
#include <QRegion>
#include <QtQml/qqmlregistration.h>

class QPainter;

// Rasterization stays on the GUI thread. The scene graph only receives image snapshots.
class ScintillaImageItem : public QQuickItem
{
    Q_OBJECT
    QML_ANONYMOUS

  public:
    explicit ScintillaImageItem(QQuickItem* parent = nullptr);
    void invalidateImage(const QRectF& rect);
    void invalidateImage();
    void scrollImage(int dy);

  protected:
    virtual bool paintImage(QPainter& painter, const QRect& rect) = 0;
    void updatePolish() override;
    QSGNode* updatePaintNode(QSGNode* node, UpdatePaintNodeData*) override;
    void geometryChange(const QRectF& geometry, const QRectF& oldGeometry) override;
    void itemChange(ItemChange change, const ItemChangeData& data) override;

  private:
    QImage m_image;
    QRegion m_dirty;
    quint64 m_revision = 0;
};

#endif
