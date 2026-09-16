// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "highlighting/syntaxhighlightingdocument.h"
#include <functional>

class ScintillaQuickAdapter;

class ScintillaHighlighter final : public QObject
{
  public:
    ScintillaHighlighter(QObject* parent, ScintillaQuickAdapter& editor);
    void reset(QByteArray source);
    void edit(qsizetype start, qsizetype deleted, QByteArray inserted);
    void configure(QString language, QString fileName, bool darkTheme);
    void setComposing(bool composing);
    void styleNeeded(qsizetype position);
    void restoreStyles();
    void resetFromEditor();
    void updatePresentation();
    bool presentationReady() const { return ready; }
    QString syntaxName = QStringLiteral("Plain Text");
    bool languageRecognized = false;
    std::function<void()> metadataChanged;
    std::function<void()> presentationChanged;

  private:
    ScintillaQuickAdapter& editor;
    craftward::highlighting::SyntaxHighlightingDocument document;
    QList<craftward::highlighting::Style> styles;
    QString language, fileName;
    bool darkTheme = false, failed = false, capacityReported = false;
    bool ready = true, receivedStyles = false;
    bool synchronizeQueued = false, needsReset = false, needsConfigure = false;
    qsizetype styledThrough = 0;
    QByteArray resetSource;
    int styleId(const craftward::highlighting::Style& style);
    void defineStyle(int index);
    void apply(const craftward::highlighting::DocumentStyles& batch);
    void setMetadata(const QString& name, bool recognized);
    void setReady(bool value);
    void queueSynchronization();
};
