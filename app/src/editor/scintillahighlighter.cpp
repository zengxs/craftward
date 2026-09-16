// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "scintillahighlighter_p.h"
#include "scintillaquickadapter_p.h"
#include <QDebug>
#include <utility>

using namespace Scintilla;
using namespace craftward::highlighting;

namespace {
int
styleNumber(int index)
{
    return index < 31 ? index + 1 : index + 9;
}
}

ScintillaHighlighter::ScintillaHighlighter(QObject* parent, ScintillaQuickAdapter& adapter)
  : QObject(parent)
  , editor(adapter)
  , document(this)
{
    editor.WndProc(Message::SetILexer, 0, 0);
    connect(&document, &SyntaxHighlightingDocument::stylesReady, this, [this](const DocumentStyles& batch) {
        if (synchronizeQueued)
            document.acknowledge(batch, false);
        else
            apply(batch);
    });
    connect(&document, &SyntaxHighlightingDocument::resyncRequired, this, [this] { resetFromEditor(); });
    connect(&document, &SyntaxHighlightingDocument::failed, this, [this](const QString& message) {
        // Only a pending snapshot is guaranteed to replace the failed session.
        if (needsReset)
            return;
        failed = true;
        editor.WndProc(Message::StartStyling, 0);
        editor.WndProc(Message::SetStyling, editor.WndProc(Message::GetLength), 0);
        setMetadata(QStringLiteral("Plain Text"), false);
        setReady(true);
        qWarning().noquote() << "Syntax highlighting:" << message;
        if (needsConfigure)
            resetFromEditor();
    });
    reset({});
}
void
ScintillaHighlighter::reset(QByteArray source)
{
    failed = false;
    receivedStyles = false;
    styledThrough = 0;
    setReady(source.isEmpty() || (language.trimmed().isEmpty() && fileName.isEmpty()));
    resetSource = std::move(source);
    needsReset = true;
    queueSynchronization();
}
void
ScintillaHighlighter::queueSynchronization()
{
    if (synchronizeQueued)
        return;
    synchronizeQueued = true;
    QTimer::singleShot(0, this, [this] {
        synchronizeQueued = false;
        if (std::exchange(needsReset, false)) {
            needsConfigure = false;
            document.reset(std::exchange(resetSource, {}), language, fileName, darkTheme ? Theme::Dark : Theme::Light);
        } else if (std::exchange(needsConfigure, false)) {
            document.configure(language, fileName, darkTheme ? Theme::Dark : Theme::Light);
        }
    });
}
void
ScintillaHighlighter::setReady(bool value)
{
    if (ready == value)
        return;
    ready = value;
    if (presentationChanged)
        presentationChanged();
}
void
ScintillaHighlighter::updatePresentation()
{
    if (!ready && !synchronizeQueued && receivedStyles && !editor.composing() &&
        styledThrough >= editor.visibleTextEnd())
        setReady(true);
}
void
ScintillaHighlighter::resetFromEditor()
{
    const qsizetype length = editor.WndProc(Message::GetLength);
    QByteArray bytes(length + 1, '\0');
    editor.WndProc(Message::GetText, bytes.size(), reinterpret_cast<sptr_t>(bytes.data()));
    bytes.resize(length);
    reset(std::move(bytes));
}
void
ScintillaHighlighter::edit(qsizetype start, qsizetype deleted, QByteArray inserted)
{
    if (failed || needsReset || !ready) {
        resetFromEditor();
        return;
    }
    document.edit(start, deleted, std::move(inserted));
}
void
ScintillaHighlighter::configure(QString newLanguage, QString newFileName, bool dark)
{
    const bool opening = needsReset || fileName != newFileName;
    receivedStyles = false;
    styledThrough = 0;
    language = std::move(newLanguage);
    fileName = std::move(newFileName);
    darkTheme = dark;
    if (opening)
        setReady(editor.WndProc(Message::GetLength) == 0 || (language.trimmed().isEmpty() && fileName.isEmpty()));
    if (failed)
        resetFromEditor();
    else {
        needsConfigure = true;
        queueSynchronization();
    }
}
void
ScintillaHighlighter::setComposing(bool composing)
{
    document.setPaused(composing);
}
void
ScintillaHighlighter::styleNeeded(qsizetype position)
{
    // Preserve shifted cached style bytes while the worker owns syntax validity.
    // StartStyling advances the core's request cursor without recoloring text.
    editor.WndProc(Message::StartStyling,
                   qBound(qsizetype(0), position, qsizetype(editor.WndProc(Message::GetLength))));
}
void
ScintillaHighlighter::setMetadata(const QString& name, bool recognized)
{
    if (syntaxName == name && languageRecognized == recognized)
        return;
    syntaxName = name;
    languageRecognized = recognized;
    if (metadataChanged)
        metadataChanged();
}
int
ScintillaHighlighter::styleId(const Style& source)
{
    Style effective = source;
    effective.background = {};
    const qsizetype existing = styles.indexOf(effective);
    if (existing >= 0)
        return styleNumber(existing);
    if (styles.size() >= 247) {
        if (!capacityReported) {
            capacityReported = true;
            qWarning("Syntax highlighting exceeded Scintilla's text style capacity.");
        }
        return 0;
    }
    styles.append(effective);
    defineStyle(styles.size() - 1);
    return styleNumber(styles.size() - 1);
}
void
ScintillaHighlighter::defineStyle(int index)
{
    const auto& style = styles[index];
    const int id = styleNumber(index);
    const int background = editor.WndProc(Message::StyleGetBack, STYLE_DEFAULT);
    const auto channel = [&](int foreground, int shift) {
        return (foreground * style.foreground.alpha() +
                ((background >> shift) & 255) * (255 - style.foreground.alpha()) + 127) /
               255;
    };
    const int foreground = channel(style.foreground.red(), 0) | (channel(style.foreground.green(), 8) << 8) |
                           (channel(style.foreground.blue(), 16) << 16);
    editor.WndProc(Message::StyleSetFore, id, foreground);
    editor.WndProc(Message::StyleSetWeight,
                   id,
                   style.bold ? SC_WEIGHT_BOLD : editor.WndProc(Message::StyleGetWeight, STYLE_DEFAULT));
    editor.WndProc(Message::StyleSetItalic, id, style.italic);
    editor.WndProc(Message::StyleSetUnderline, id, style.underline);
}
void
ScintillaHighlighter::restoreStyles()
{
    for (qsizetype index = 0; index < styles.size(); ++index)
        defineStyle(index);
}
void
ScintillaHighlighter::apply(const DocumentStyles& batch)
{
    const qsizetype length = editor.WndProc(Message::GetLength);
    if (editor.composing() || batch.start < 0 || batch.end > length || batch.start > batch.end) {
        document.acknowledge(batch, false);
        return;
    }
    QByteArray bytes(batch.end - batch.start, '\0');
    for (const auto& span : batch.result.spans) {
        const char id = static_cast<char>(styleId(span.style));
        std::fill(bytes.begin() + span.utf8Start - batch.start, bytes.begin() + span.utf8End - batch.start, id);
    }
    if (!bytes.isEmpty()) {
        const auto endStyled = editor.WndProc(Message::GetEndStyled);
        editor.WndProc(Message::StartStyling, batch.start);
        editor.WndProc(Message::SetStylingEx, bytes.size(), reinterpret_cast<sptr_t>(bytes.constData()));
        editor.WndProc(Message::StartStyling, qMax<qsizetype>(endStyled, batch.end));
    }
    setMetadata(batch.result.syntaxName, batch.result.languageRecognized);
    receivedStyles = true;
    if (batch.start <= styledThrough)
        styledThrough = qMax(styledThrough, batch.end);
    updatePresentation();
    document.acknowledge(batch, true);
}
