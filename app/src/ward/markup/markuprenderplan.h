// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "document.qpb.h"

#include <QHash>
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
    int listNumberDigits = 0;
    QList<MarkupTextBlock> blocks;
    [[nodiscard]] QList<MarkupTextRun> selectionRuns() const;
    [[nodiscard]] QString text() const;
    bool operator==(const MarkupTextSurface&) const = default;
};
Q_DECLARE_METATYPE(MarkupTextSurface)

/// Complete-block list values retained when a top-level list is split.
struct MarkupListContext
{
    int numberDigits = 0;
    qreal rootItemSpacing = 0;
};

[[nodiscard]] MarkupListContext
markupListContext(const ward::markup::v1::SemanticBlock& block);
[[nodiscard]] QVariantList
markupRenderParts(const ward::markup::v1::SemanticDocument& document,
                  const QHash<QString, MarkupListContext>& listContexts = {});
[[nodiscard]] int
markupListNumberDigits(const ward::markup::v1::SemanticBlock& block);
[[nodiscard]] qreal
markupListItemSpacing(const QList<ward::markup::v1::SemanticNode>& nodes, qsizetype listIndex = 0);
[[nodiscard]] QVariantList
markupLiteralPart(const QString& key, const QString& text);
[[nodiscard]] QList<MarkupTextSurface>
markupSurfaces(const QVariantList& parts);
