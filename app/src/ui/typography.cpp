// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "typography.h"

#include "applicationfonts.h"

#include <QFontDatabase>

namespace {

constexpr qreal codeFontPointSize = 13.0;
constexpr qreal codeLineHeightScaleValue = 1.25;

} // namespace

Typography::Typography(QObject* parent)
  : QObject(parent)
  , m_monoFamily(loadApplicationMonoFont())
{
    if (m_monoFamily.isEmpty())
        m_monoFamily = QFontDatabase::systemFont(QFontDatabase::FixedFont).family();

    m_codeFont.setFamily(m_monoFamily);
    m_codeFont.setPointSizeF(codeFontPointSize);
    m_codeFont.setWeight(QFont::Medium);
}

QString
Typography::monoFamily() const
{
    return m_monoFamily;
}

QFont
Typography::codeFont() const
{
    return m_codeFont;
}

qreal
Typography::codeLineHeightScale() const
{
    return codeLineHeightScaleValue;
}
