// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QColor>
#include <QFont>
#include <QList>
#include <QPointer>
#include <QQmlParserStatus>
#include <QQuickTextDocument>
#include <QVariant>
#include <QtQmlIntegration/qqmlintegration.h>

/// Writes one projected text surface into a materialized TextEdit's native document.
class MarkupTextDocument
  : public QObject
  , public QQmlParserStatus
{
    Q_OBJECT
    QML_ELEMENT
    Q_INTERFACES(QQmlParserStatus)
    Q_PROPERTY(QQuickTextDocument* textDocument READ textDocument WRITE setTextDocument NOTIFY textDocumentChanged)
    Q_PROPERTY(QVariant surface READ surface WRITE setSurface NOTIFY surfaceChanged)
    Q_PROPERTY(QFont font MEMBER font_ NOTIFY styleChanged)
    Q_PROPERTY(QFont codeFont MEMBER codeFont_ NOTIFY styleChanged)
    Q_PROPERTY(QColor textColor MEMBER textColor_ NOTIFY styleChanged)
    Q_PROPERTY(QColor linkColor MEMBER linkColor_ NOTIFY styleChanged)
    Q_PROPERTY(QColor codeBackground MEMBER codeBackground_ NOTIFY styleChanged)

  public:
    explicit MarkupTextDocument(QObject* parent = nullptr);

    [[nodiscard]] QQuickTextDocument* textDocument() const;
    void setTextDocument(QQuickTextDocument* document);
    [[nodiscard]] QVariant surface() const;
    void setSurface(const QVariant& surface);
    Q_INVOKABLE [[nodiscard]] QVariantMap endpointAt(int position) const;
    Q_INVOKABLE [[nodiscard]] QVariantMap wordAt(int position) const;
    Q_INVOKABLE [[nodiscard]] int documentPosition(int surfacePosition) const;
    void classBegin() override;
    void componentComplete() override;

  signals:
    void textDocumentChanged();
    void surfaceChanged();
    void styleChanged();
    void rendered();

  private:
    struct CollapsedLineBreak
    {
        int surfacePosition;
        int documentPosition;
    };

    void render();
    void rebuildPositionMap();
    [[nodiscard]] int surfacePosition(int documentPosition) const;
    [[nodiscard]] QVariantMap endpointAtSurfacePosition(int position) const;

    QPointer<QQuickTextDocument> document_;
    QVariant surface_;
    QList<CollapsedLineBreak> collapsedLineBreaks_;
    QFont font_;
    QFont codeFont_;
    QColor textColor_ = Qt::black;
    QColor linkColor_ = Qt::blue;
    QColor codeBackground_ = Qt::lightGray;
    bool complete_ = true;
};
