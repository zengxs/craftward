// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "markuptextbackground.h"

#include <QColor>

class QQmlComponent;

/// Paints selection backgrounds and preserves syntax decorations beneath a TextEdit.
class MarkupSelectionBackground : public MarkupTextBackground
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QColor color READ color WRITE setColor NOTIFY colorChanged)
    Q_PROPERTY(bool nativeSelection READ nativeSelection WRITE setNativeSelection NOTIFY nativeSelectionChanged)
    Q_PROPERTY(bool joinParagraphs READ joinParagraphs WRITE setJoinParagraphs NOTIFY joinParagraphsChanged)

  public:
    explicit MarkupSelectionBackground(QQuickItem* parent = nullptr);
    [[nodiscard]] QColor color() const;
    void setColor(const QColor& color);
    [[nodiscard]] bool nativeSelection() const;
    void setNativeSelection(bool nativeSelection);
    [[nodiscard]] bool joinParagraphs() const;
    void setJoinParagraphs(bool joinParagraphs);

  signals:
    void colorChanged();
    void nativeSelectionChanged();
    void joinParagraphsChanged();

  protected:
    void updatePolish() override;
    QSGNode* updatePaintNode(QSGNode* node, UpdatePaintNodeData*) override;

  private:
    void updateDecorations();

    QList<QRectF> rectangles_;
    QList<QPair<QRectF, QColor>> decorations_;
    QList<QQuickItem*> decorationItems_;
    QQmlComponent* decorationComponent_ = nullptr;
    QColor color_ = Qt::lightGray;
    bool nativeSelection_ = false;
    bool joinParagraphs_ = false;
};
