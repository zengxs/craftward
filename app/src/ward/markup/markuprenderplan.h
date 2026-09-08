// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "document.qpb.h"

#include <QTextFormat>
#include <QVariant>

struct MarkupTextRun
{
    QString key;
    QString text;
    QTextCharFormat format;
    bool code = false;
    bool annotation = false;
    qreal scale = 1;
    bool operator==(const MarkupTextRun&) const = default;
};

struct MarkupTextBlock
{
    QString key;
    QTextBlockFormat format;
    QTextListFormat list;
    QString listKey;
    QList<MarkupTextRun> runs;
    bool operator==(const MarkupTextBlock&) const = default;
};

/// Value-only text projection. No font measurement or document layout is retained.
struct MarkupTextSurface
{
    QString separator = QStringLiteral("\n");
    QList<MarkupTextBlock> blocks;
    [[nodiscard]] QList<MarkupTextRun> selectionRuns() const;
    [[nodiscard]] QString text() const;
    bool operator==(const MarkupTextSurface&) const = default;
};
Q_DECLARE_METATYPE(MarkupTextSurface)

[[nodiscard]] QVariantList
markupRenderParts(const ward::markup::v1::SemanticDocument& document);
[[nodiscard]] QVariantList
markupLiteralPart(const QString& key, const QString& text);
[[nodiscard]] QList<MarkupTextSurface>
markupSurfaces(const QVariantList& parts);
