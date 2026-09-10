// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "markuptextbackground.h"

#include <QColor>
#include <QSGTextNode>

#include <memory>
#include <vector>

/// Draws list markers at the first text line's baseline with an independent gap.
class MarkupListMarkers : public MarkupTextBackground
{
    Q_OBJECT
    QML_ELEMENT

  public:
    explicit MarkupListMarkers(QQuickItem* parent = nullptr);

  protected:
    void updatePolish() override;
    QSGNode* updatePaintNode(QSGNode* node, UpdatePaintNodeData*) override;

  private:
    struct Marker
    {
        std::unique_ptr<QTextLayout> layout;
        QPointF position;
    };

    std::vector<Marker> markers_;
    QColor color_;
    QSGTextNode::RenderType renderType_ = QSGTextNode::QtRendering;
};
