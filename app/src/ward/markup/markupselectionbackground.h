// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QColor>
#include <QPointer>
#include <QQuickItem>
#include <QtQmlIntegration/qqmlintegration.h>

class QAbstractTextDocumentLayout;
class QQmlComponent;
class QQuickTextDocument;
class QTextDocument;

/// Paints selection backgrounds and preserves syntax decorations beneath a TextEdit.
class MarkupSelectionBackground : public QQuickItem
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QQuickItem* textEdit READ textEdit WRITE setTextEdit NOTIFY textEditChanged)
    Q_PROPERTY(QColor color READ color WRITE setColor NOTIFY colorChanged)

  public:
    explicit MarkupSelectionBackground(QQuickItem* parent = nullptr);
    [[nodiscard]] QQuickItem* textEdit() const;
    void setTextEdit(QQuickItem* item);
    [[nodiscard]] QColor color() const;
    void setColor(const QColor& color);

  signals:
    void textEditChanged();
    void colorChanged();

  protected:
    void updatePolish() override;
    QSGNode* updatePaintNode(QSGNode* node, UpdatePaintNodeData*) override;
    void geometryChange(const QRectF& geometry, const QRectF& previous) override;

  private slots:
    void scheduleLayout();

  private:
    void observeDocument();
    void updateDecorations();

    QPointer<QQuickItem> textEdit_;
    QPointer<QQuickTextDocument> quickDocument_;
    QPointer<QTextDocument> document_;
    QPointer<QAbstractTextDocumentLayout> layout_;
    QList<QMetaObject::Connection> itemConnections_;
    QList<QMetaObject::Connection> documentConnections_;
    QList<QRectF> rectangles_;
    QList<QPair<QRectF, QColor>> decorations_;
    QList<QQuickItem*> decorationItems_;
    QQmlComponent* decorationComponent_ = nullptr;
    QColor color_ = Qt::lightGray;
};
