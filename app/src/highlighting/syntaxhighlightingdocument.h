// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "syntaxhighlightingengine.h"
#include <QByteArray>
#include <QObject>

namespace craftward::highlighting {
struct DocumentStyles
{
    quint64 epoch = 0, revision = 0, configuration = 0, sequence = 0;
    qsizetype start = 0, end = 0;
    Result result;
    bool complete = false;
};

class SyntaxHighlightingDocumentWorker;

// GUI-facing interface; the mutable Rust session never leaves its worker thread.
class SyntaxHighlightingDocument final : public QObject
{
    Q_OBJECT
  public:
    explicit SyntaxHighlightingDocument(QObject* parent = nullptr);
    ~SyntaxHighlightingDocument() override;
    void reset(QByteArray source, QString language, QString fileName, Theme theme);
    void edit(qsizetype start, qsizetype deleted, QByteArray inserted);
    void configure(QString language, QString fileName, Theme theme);
    void setPaused(bool paused);
    void acknowledge(const DocumentStyles& styles, bool applied);

  signals:
    void stylesReady(const craftward::highlighting::DocumentStyles& styles);
    void failed(const QString& message);
    void resyncRequired();

  private:
    SyntaxHighlightingDocumentWorker* worker;
    quint64 epoch = 0, revision = 0, configuration = 0;
    bool paused = false;
};
}

Q_DECLARE_METATYPE(craftward::highlighting::DocumentStyles)
