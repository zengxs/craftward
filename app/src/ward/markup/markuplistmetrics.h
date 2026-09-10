// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QFont>
#include <QObject>
#include <QVariant>
#include <QtQmlIntegration/qqmlintegration.h>

[[nodiscard]] qreal
markupListIndentWidth(const QVariant& content, const QFont& font);

[[nodiscard]] qreal
markupListMarkerGap(const QFont& font, bool ordered);

/// Shares list indentation across native text and nested table surfaces.
class MarkupListMetrics : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

  public:
    explicit MarkupListMetrics(QObject* parent = nullptr)
      : QObject(parent)
    {
    }

    Q_INVOKABLE qreal indentWidth(const QVariant& content, const QFont& font) const
    {
        return markupListIndentWidth(content, font);
    }
};
