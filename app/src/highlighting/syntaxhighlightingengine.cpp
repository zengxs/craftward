// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "highlighting/syntaxhighlightingengine.h"

#include "highlight.qpb.h"
#include "ward/coreffierror.h"

#include <ward_core.h>

#include <QByteArray>
#include <QProtobufSerializer>

#include <limits>
#include <utility>

namespace {
struct SyntaxEngineDeleter
{
    void operator()(WardSyntaxHighlightingEngine* engine) const
    {
        ward_core_syntax_highlighting_engine_destroy(engine);
    }
};

struct WardOwnedBufferDeleter
{
    void operator()(WardOwnedBuffer* buffer) const { ward_core_owned_buffer_destroy(buffer); }
};

using OwnedSyntaxEngine = std::unique_ptr<WardSyntaxHighlightingEngine, SyntaxEngineDeleter>;
using OwnedWardBuffer = std::unique_ptr<WardOwnedBuffer, WardOwnedBufferDeleter>;

QColor
colorFromWire(const ward::highlighting::v1::Color& color)
{
    return QColor::fromRgb(static_cast<int>(color.red()),
                           static_cast<int>(color.green()),
                           static_cast<int>(color.blue()),
                           static_cast<int>(color.alpha()));
}

WardSyntaxHighlightingTheme
themeToWire(craftward::highlighting::Theme theme)
{
    return theme == craftward::highlighting::Theme::Dark ? WardSyntaxHighlightingThemeDark
                                                         : WardSyntaxHighlightingThemeLight;
}
}

namespace craftward::highlighting {
struct SyntaxHighlightingEngine::Private
{
    OwnedSyntaxEngine engine;
    QString errorMessage;
};

SyntaxHighlightingEngine::SyntaxHighlightingEngine()
  : d(std::make_unique<Private>())
{
    WardError* rawError = nullptr;
    d->engine.reset(ward_core_syntax_highlighting_engine_create(&rawError));
    if (!d->engine) {
        d->errorMessage = ward::coreffi::takeErrorMessage(rawError);
        if (d->errorMessage.isEmpty())
            d->errorMessage = QStringLiteral("Ward Core did not create a syntax-highlighting engine.");
    }
}

SyntaxHighlightingEngine::~SyntaxHighlightingEngine() = default;

std::shared_ptr<const SyntaxHighlightingEngine>
SyntaxHighlightingEngine::shared()
{
    static const auto engine = std::shared_ptr<const SyntaxHighlightingEngine>(new SyntaxHighlightingEngine);
    return engine;
}

Result
SyntaxHighlightingEngine::highlight(QByteArrayView source, QByteArrayView language, Theme theme) const
{
    if (!d->engine)
        return Result{ .errorMessage = d->errorMessage };

    WardError* rawError = nullptr;
    OwnedWardBuffer buffer(ward_core_syntax_highlight(d->engine.get(),
                                                      reinterpret_cast<const std::uint8_t*>(source.data()),
                                                      static_cast<std::size_t>(source.size()),
                                                      reinterpret_cast<const std::uint8_t*>(language.data()),
                                                      static_cast<std::size_t>(language.size()),
                                                      themeToWire(theme),
                                                      &rawError));
    if (!buffer) {
        QString message = ward::coreffi::takeErrorMessage(rawError);
        if (message.isEmpty())
            message = QStringLiteral("Ward Core returned no syntax-highlighting result.");
        return Result{ .errorMessage = std::move(message) };
    }

    const std::size_t bufferSize = ward_core_owned_buffer_size(buffer.get());
    if (bufferSize > static_cast<std::size_t>(std::numeric_limits<qsizetype>::max())) {
        return Result{ .errorMessage = QStringLiteral("The syntax-highlighting result is too large.") };
    }
    const QByteArrayView bytes(reinterpret_cast<const char*>(ward_core_owned_buffer_data(buffer.get())),
                               static_cast<qsizetype>(bufferSize));
    ward::highlighting::v1::HighlightedCode highlighted;
    QProtobufSerializer serializer;
    if (!highlighted.deserialize(&serializer, bytes)) {
        return Result{
            .errorMessage =
              QStringLiteral("Failed to decode the syntax-highlighting result: %1").arg(serializer.lastErrorString()),
        };
    }

    Result result{
        .syntaxName = highlighted.syntaxName(),
        .languageRecognized = highlighted.languageRecognized(),
    };
    result.spans.reserve(highlighted.spans().size());
    for (const ward::highlighting::v1::Span& span : highlighted.spans()) {
        if (!span.hasStyle() || !span.style().hasForeground() || !span.style().hasBackground() ||
            span.utf8Start() > span.utf8End() || span.utf8End() > static_cast<quint64>(source.size()) ||
            span.utf8End() > static_cast<quint64>(std::numeric_limits<qsizetype>::max())) {
            return Result{ .errorMessage = QStringLiteral("Ward Core returned an invalid highlighting span.") };
        }
        const auto& style = span.style();
        result.spans.append(Span{
          .utf8Start = static_cast<qsizetype>(span.utf8Start()),
          .utf8End = static_cast<qsizetype>(span.utf8End()),
          .style =
            Style{
              .foreground = colorFromWire(style.foreground()),
              .background = colorFromWire(style.background()),
              .bold = style.bold(),
              .italic = style.italic(),
              .underline = style.underline(),
            },
        });
    }
    return result;
}
}
