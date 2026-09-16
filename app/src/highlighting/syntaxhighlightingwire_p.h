// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "highlight.qpb.h"
#include "syntaxhighlightingengine.h"

namespace craftward::highlighting {
inline QColor
colorFromWire(const ward::highlighting::v1::Color& color)
{
    return QColor::fromRgb(static_cast<int>(color.red()),
                           static_cast<int>(color.green()),
                           static_cast<int>(color.blue()),
                           static_cast<int>(color.alpha()));
}

inline Style
styleFromWire(const ward::highlighting::v1::Style& style)
{
    return Style{ .foreground = colorFromWire(style.foreground()),
                  .background = colorFromWire(style.background()),
                  .bold = style.bold(),
                  .italic = style.italic(),
                  .underline = style.underline() };
}
}
