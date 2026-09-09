// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "markuptextbackground.h"

#include <QVariantList>

/// Projects native inline code ranges into background rectangles in item coordinates.
class MarkupCodeBackground : public MarkupTextBackground
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(qreal verticalPadding READ verticalPadding WRITE setVerticalPadding NOTIFY verticalPaddingChanged)
    Q_PROPERTY(QVariantList rectangles READ rectangles NOTIFY rectanglesChanged)

  public:
    explicit MarkupCodeBackground(QQuickItem* parent = nullptr);
    [[nodiscard]] qreal verticalPadding() const;
    void setVerticalPadding(qreal padding);
    [[nodiscard]] QVariantList rectangles() const;

  signals:
    void verticalPaddingChanged();
    void rectanglesChanged();

  protected:
    void updatePolish() override;

  private:
    QVariantList rectangles_;
    qreal verticalPadding_ = 0;
};
