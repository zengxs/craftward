// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QColor>
#include <QFont>
#include <QPointer>
#include <QQmlParserStatus>
#include <QQuickTextDocument>
#include <QVariant>
#include <QtQmlIntegration/qqmlintegration.h>

/// Projects one semantic segment into a materialized TextEdit's native document.
class MarkupTextDocument
  : public QObject
  , public QQmlParserStatus
{
    Q_OBJECT
    QML_ELEMENT
    Q_INTERFACES(QQmlParserStatus)
    Q_PROPERTY(QQuickTextDocument* textDocument READ textDocument WRITE setTextDocument NOTIFY textDocumentChanged)
    Q_PROPERTY(QVariant segment READ segment WRITE setSegment NOTIFY segmentChanged)
    Q_PROPERTY(QFont font MEMBER font_ NOTIFY styleChanged)
    Q_PROPERTY(QFont codeFont MEMBER codeFont_ NOTIFY styleChanged)
    Q_PROPERTY(QColor textColor MEMBER textColor_ NOTIFY styleChanged)
    Q_PROPERTY(QColor linkColor MEMBER linkColor_ NOTIFY styleChanged)
    Q_PROPERTY(QColor codeBackground MEMBER codeBackground_ NOTIFY styleChanged)

  public:
    explicit MarkupTextDocument(QObject* parent = nullptr);

    [[nodiscard]] QQuickTextDocument* textDocument() const;
    void setTextDocument(QQuickTextDocument* document);
    [[nodiscard]] QVariant segment() const;
    void setSegment(const QVariant& segment);
    void classBegin() override;
    void componentComplete() override;

  signals:
    void textDocumentChanged();
    void segmentChanged();
    void styleChanged();

  private:
    void render();

    QPointer<QQuickTextDocument> document_;
    QVariant segment_;
    QFont font_;
    QFont codeFont_;
    QColor textColor_ = Qt::black;
    QColor linkColor_ = Qt::blue;
    QColor codeBackground_ = Qt::lightGray;
    bool complete_ = true;
};
