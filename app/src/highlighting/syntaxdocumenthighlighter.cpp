// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "highlighting/syntaxdocumenthighlighter.h"

#include "highlighting/syntaxhighlightingengine.h"

#include <QFutureWatcher>
#include <QPointer>
#include <QQuickTextDocument>
#include <QTextBlock>
#include <QTextCharFormat>
#include <QTextDocument>
#include <QTimer>
#include <QVector>
#include <QtConcurrent/QtConcurrentRun>

#include <algorithm>
#include <limits>
#include <utility>

namespace {
constexpr int HIGHLIGHT_DEBOUNCE_MILLISECONDS = 32;

struct FormatSpan
{
    int start = 0;
    int end = 0;
    QTextCharFormat format;
};

struct HighlightRequest
{
    quint64 generation = 0;
    QString source;
    QString language;
    craftward::highlighting::Theme theme = craftward::highlighting::Theme::Light;
};

struct HighlightResult
{
    quint64 generation = 0;
    QString source;
    craftward::highlighting::Result highlighted;
};

int
utf8Length(char32_t character)
{
    if (character <= 0x7f)
        return 1;
    if (character <= 0x7ff)
        return 2;
    if (character <= 0xffff)
        return 3;
    return 4;
}

QVector<int>
utf8ToUtf16Boundaries(const QString& source)
{
    const QByteArray encoded = source.toUtf8();
    QVector<int> boundaries(encoded.size() + 1, -1);
    qsizetype utf8Offset = 0;
    qsizetype utf16Offset = 0;
    while (utf16Offset < source.size()) {
        boundaries[utf8Offset] = static_cast<int>(utf16Offset);
        const QChar first = source.at(utf16Offset);
        char32_t character = first.unicode();
        qsizetype utf16Length = 1;
        if (first.isHighSurrogate() && utf16Offset + 1 < source.size() && source.at(utf16Offset + 1).isLowSurrogate()) {
            character = QChar::surrogateToUcs4(first, source.at(utf16Offset + 1));
            utf16Length = 2;
        }
        utf8Offset += utf8Length(character);
        utf16Offset += utf16Length;
    }
    boundaries[utf8Offset] = static_cast<int>(utf16Offset);
    return boundaries;
}

QList<FormatSpan>
documentSpans(const QString& source, const craftward::highlighting::Result& highlighted, QString* errorMessage)
{
    const QByteArray encoded = source.toUtf8();
    if (encoded.size() > std::numeric_limits<int>::max()) {
        *errorMessage = QStringLiteral("The highlighted document is too large for QTextDocument.");
        return {};
    }
    const QVector<int> boundaries = utf8ToUtf16Boundaries(source);
    QList<FormatSpan> spans;
    spans.reserve(highlighted.spans.size());
    for (const craftward::highlighting::Span& span : highlighted.spans) {
        if (span.utf8Start < 0 || span.utf8End < span.utf8Start || span.utf8End >= boundaries.size() ||
            boundaries.at(span.utf8Start) < 0 || boundaries.at(span.utf8End) < 0) {
            *errorMessage = QStringLiteral("The highlighter returned a range outside a UTF-8 character boundary.");
            return {};
        }
        if (span.utf8Start == span.utf8End)
            continue;

        QTextCharFormat format;
        format.setForeground(span.style.foreground);
        if (span.style.bold)
            format.setFontWeight(QFont::Bold);
        if (span.style.italic)
            format.setFontItalic(true);
        if (span.style.underline)
            format.setFontUnderline(true);
        spans.append(FormatSpan{
          .start = boundaries.at(span.utf8Start),
          .end = boundaries.at(span.utf8End),
          .format = std::move(format),
        });
    }
    return spans;
}
}

struct SyntaxDocumentHighlighter::Private
{
    QPointer<QQuickTextDocument> quickDocument;
    QMetaObject::Connection documentChangedConnection;
    QMetaObject::Connection documentDestroyedConnection;
    QMetaObject::Connection contentsChangedConnection;
    QString language;
    QString syntaxName;
    QList<FormatSpan> spans;
    quint64 requestedGeneration = 0;
    bool languageRecognized = false;
    bool darkTheme = false;
    bool applyingFormats = false;
    QTimer timer;
    QFutureWatcher<HighlightResult> watcher;
};

SyntaxDocumentHighlighter::SyntaxDocumentHighlighter(QObject* parent)
  : QSyntaxHighlighter(parent)
  , d(std::make_unique<Private>())
{
    d->timer.setSingleShot(true);
    d->timer.setInterval(HIGHLIGHT_DEBOUNCE_MILLISECONDS);
    connect(&d->timer, &QTimer::timeout, this, &SyntaxDocumentHighlighter::dispatchHighlight);
    connect(&d->watcher,
            &QFutureWatcher<HighlightResult>::finished,
            this,
            &SyntaxDocumentHighlighter::applyFinishedHighlight);
}

SyntaxDocumentHighlighter::~SyntaxDocumentHighlighter()
{
    d->timer.stop();
    disconnect(d->documentChangedConnection);
    disconnect(d->documentDestroyedConnection);
    disconnect(d->contentsChangedConnection);
    disconnect(&d->timer, nullptr, this, nullptr);
    disconnect(&d->watcher, nullptr, this, nullptr);
    setDocument(nullptr);
}

QQuickTextDocument*
SyntaxDocumentHighlighter::textDocument() const
{
    return d->quickDocument.data();
}

void
SyntaxDocumentHighlighter::setTextDocument(QQuickTextDocument* document)
{
    if (d->quickDocument == document)
        return;

    disconnect(d->documentChangedConnection);
    disconnect(d->documentDestroyedConnection);
    disconnect(d->contentsChangedConnection);
    setDocument(nullptr);
    d->quickDocument = document;
    d->spans.clear();
    setSyntaxResolution({}, false);

    if (document) {
        d->documentChangedConnection =
          connect(document, &QQuickTextDocument::textDocumentChanged, this, [this] { attachDocument(); });
        d->documentDestroyedConnection = connect(document, &QObject::destroyed, this, [this] {
            disconnect(d->contentsChangedConnection);
            setDocument(nullptr);
            d->quickDocument = nullptr;
            d->spans.clear();
            setSyntaxResolution({}, false);
            ++d->requestedGeneration;
            emit textDocumentChanged();
        });
    }

    attachDocument();
    emit textDocumentChanged();
}

QString
SyntaxDocumentHighlighter::language() const
{
    return d->language;
}

void
SyntaxDocumentHighlighter::setLanguage(const QString& language)
{
    if (d->language == language)
        return;
    d->language = language;
    setSyntaxResolution({}, false);
    scheduleHighlight();
    emit languageChanged();
}

bool
SyntaxDocumentHighlighter::darkTheme() const
{
    return d->darkTheme;
}

void
SyntaxDocumentHighlighter::setDarkTheme(bool darkTheme)
{
    if (d->darkTheme == darkTheme)
        return;
    d->darkTheme = darkTheme;
    scheduleHighlight();
    emit darkThemeChanged();
}

QString
SyntaxDocumentHighlighter::syntaxName() const
{
    return d->syntaxName;
}

bool
SyntaxDocumentHighlighter::languageRecognized() const
{
    return d->languageRecognized;
}

void
SyntaxDocumentHighlighter::attachDocument()
{
    disconnect(d->contentsChangedConnection);
    QTextDocument* nativeDocument = d->quickDocument ? d->quickDocument->textDocument() : nullptr;
    setDocument(nativeDocument);
    d->spans.clear();
    if (nativeDocument) {
        d->contentsChangedConnection = connect(nativeDocument, &QTextDocument::contentsChanged, this, [this] {
            if (!d->applyingFormats)
                scheduleHighlight();
        });
    }
    scheduleHighlight();
}

void
SyntaxDocumentHighlighter::scheduleHighlight()
{
    ++d->requestedGeneration;
    d->spans.clear();
    rehighlightDocument();
    if (document())
        d->timer.start();
    else
        d->timer.stop();
}

void
SyntaxDocumentHighlighter::dispatchHighlight()
{
    if (!document() || d->watcher.isRunning())
        return;

    HighlightRequest request{
        .generation = d->requestedGeneration,
        .source = document()->toPlainText(),
        .language = d->language,
        .theme = d->darkTheme ? craftward::highlighting::Theme::Dark : craftward::highlighting::Theme::Light,
    };
    const auto engine = craftward::highlighting::SyntaxHighlightingEngine::shared();
    d->watcher.setFuture(QtConcurrent::run([engine, request = std::move(request)]() mutable {
        const QByteArray source = request.source.toUtf8();
        const QByteArray language = request.language.toUtf8();
        return HighlightResult{
            .generation = request.generation,
            .source = std::move(request.source),
            .highlighted = engine->highlight(source, language, request.theme),
        };
    }));
}

void
SyntaxDocumentHighlighter::applyFinishedHighlight()
{
    HighlightResult result = d->watcher.result();
    if (document() && result.generation == d->requestedGeneration && result.source == document()->toPlainText()) {
        QString errorMessage = result.highlighted.errorMessage;
        QList<FormatSpan> spans;
        if (errorMessage.isEmpty())
            spans = documentSpans(result.source, result.highlighted, &errorMessage);
        if (!errorMessage.isEmpty())
            qWarning().noquote() << "Syntax highlighting failed:" << errorMessage;
        setSyntaxResolution(errorMessage.isEmpty() ? result.highlighted.syntaxName : QString(),
                            errorMessage.isEmpty() && result.highlighted.languageRecognized);
        d->spans = std::move(spans);
        rehighlightDocument();
    }

    if (document() && result.generation != d->requestedGeneration)
        d->timer.start(0);
}

void
SyntaxDocumentHighlighter::rehighlightDocument()
{
    if (!document())
        return;
    d->applyingFormats = true;
    rehighlight();
    d->applyingFormats = false;
}

void
SyntaxDocumentHighlighter::setSyntaxResolution(QString syntaxName, bool languageRecognized)
{
    const bool nameChanged = d->syntaxName != syntaxName;
    const bool recognitionChanged = d->languageRecognized != languageRecognized;
    if (!nameChanged && !recognitionChanged)
        return;

    d->syntaxName = std::move(syntaxName);
    d->languageRecognized = languageRecognized;
    if (nameChanged)
        emit syntaxNameChanged();
    if (recognitionChanged)
        emit languageRecognizedChanged();
}

void
SyntaxDocumentHighlighter::highlightBlock(const QString& text)
{
    const int blockStart = currentBlock().position();
    const int blockEnd = blockStart + text.size();
    auto span =
      std::lower_bound(d->spans.cbegin(), d->spans.cend(), blockStart, [](const FormatSpan& candidate, int position) {
          return candidate.end <= position;
      });
    for (; span != d->spans.cend() && span->start < blockEnd; ++span) {
        const int start = std::max(span->start, blockStart);
        const int end = std::min(span->end, blockEnd);
        if (start < end)
            setFormat(start - blockStart, end - start, span->format);
    }
}
