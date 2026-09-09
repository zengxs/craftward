// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QByteArray>
#include <QPointer>
#include <QQuickItem>
#include <QtQmlIntegration/qqmlintegration.h>

#include <initializer_list>
#include <optional>

class QAbstractTextDocumentLayout;
class QQuickTextDocument;
class QTextDocument;

/// Tracks the native document and coordinate origin for text background items.
class MarkupTextBackground : public QQuickItem
{
    Q_OBJECT
    QML_ANONYMOUS
    Q_PROPERTY(QQuickItem* textEdit READ textEdit WRITE setTextEdit NOTIFY textEditChanged)

  public:
    [[nodiscard]] QQuickItem* textEdit() const;
    void setTextEdit(QQuickItem* item);

  signals:
    void textEditChanged();

  protected:
    struct DocumentGeometry
    {
        QTextDocument* document;
        QPointF origin;
    };

    explicit MarkupTextBackground(QQuickItem* parent, std::initializer_list<const char*> additionalProperties = {});
    [[nodiscard]] std::optional<DocumentGeometry> documentGeometry();
    void geometryChange(const QRectF& geometry, const QRectF& previous) override;

  protected slots:
    void scheduleLayout();

  private:
    void observeDocument();

    QPointer<QQuickItem> textEdit_;
    QPointer<QQuickTextDocument> quickDocument_;
    QPointer<QTextDocument> document_;
    QPointer<QAbstractTextDocumentLayout> layout_;
    QList<QByteArray> observedProperties_;
    QList<QMetaObject::Connection> itemConnections_;
    QList<QMetaObject::Connection> documentConnections_;
};
