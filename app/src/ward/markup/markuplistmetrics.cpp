// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "markuplistmetrics.h"

#include "markuprenderplan.h"

#include <QFontInfo>
#include <QFontMetricsF>

#include <algorithm>
#include <cmath>

qreal
markupListMarkerGap(const QFont& font, bool ordered)
{
    return QFontInfo(font).pixelSize() * (ordered ? 0.7 : 0.9);
}

qreal
markupListIndentWidth(const QVariant& content, const QFont& font)
{
    const auto surfaces = content.canConvert<MarkupTextSurface>() ? QList{ content.value<MarkupTextSurface>() }
                                                                  : markupSurfaces(content.toList());
    qreal width = QFontInfo(font).pixelSize() * 2;
    int digits = 0;
    for (const auto& surface : surfaces)
        digits = std::max(digits, surface.listNumberDigits);
    if (digits > 0) {
        const QFontMetricsF metrics(font);
        qreal digitWidth = 0;
        for (int digit = 0; digit <= 9; ++digit)
            digitWidth = std::max(digitWidth, metrics.horizontalAdvance(QString::number(digit)));
        // A complete list keeps its gutter when proportional digits change.
        width = std::max(
          width, digitWidth * digits + metrics.horizontalAdvance(QLatin1Char('.')) + markupListMarkerGap(font, true));
    }
    return std::ceil(width);
}
