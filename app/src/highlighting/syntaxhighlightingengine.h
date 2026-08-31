// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QByteArrayView>
#include <QColor>
#include <QList>
#include <QString>

#include <memory>

namespace craftward::highlighting {
enum class Theme
{
    Light,
    Dark,
};

struct Style
{
    QColor foreground;
    QColor background;
    bool bold = false;
    bool italic = false;
    bool underline = false;

    bool operator==(const Style&) const = default;
};

/// One styled range using byte offsets in the supplied UTF-8 source.
struct Span
{
    qsizetype utf8Start = 0;
    qsizetype utf8End = 0;
    Style style;

    bool operator==(const Span&) const = default;
};

/// One complete renderer-independent highlighting result.
struct Result
{
    QString syntaxName;
    bool languageRecognized = false;
    QList<Span> spans;
    QString errorMessage;

    [[nodiscard]] bool succeeded() const { return errorMessage.isEmpty(); }
};

/// Loads application-maintained embedded packs and produces UTF-8 style ranges.
///
/// The immutable engine is shared by the QTextDocument adapter today and can
/// be consumed directly by a future Scintilla adapter.
class SyntaxHighlightingEngine final
{
  public:
    ~SyntaxHighlightingEngine();

    SyntaxHighlightingEngine(const SyntaxHighlightingEngine&) = delete;
    SyntaxHighlightingEngine& operator=(const SyntaxHighlightingEngine&) = delete;

    [[nodiscard]] static std::shared_ptr<const SyntaxHighlightingEngine> shared();
    [[nodiscard]] Result highlight(QByteArrayView source, QByteArrayView language, Theme theme) const;

  private:
    struct Private;

    SyntaxHighlightingEngine();

    std::unique_ptr<Private> d;
};
}
