// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QQuickTextDocument>
#include <QString>
#include <QSyntaxHighlighter>
#include <QtQmlIntegration/qqmlintegration.h>

#include <memory>

/// Applies renderer-independent syntax spans to one Qt Quick text document.
class SyntaxDocumentHighlighter : public QSyntaxHighlighter
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QQuickTextDocument* textDocument READ textDocument WRITE setTextDocument NOTIFY textDocumentChanged)
    Q_PROPERTY(QString language READ language WRITE setLanguage NOTIFY languageChanged)
    Q_PROPERTY(bool darkTheme READ darkTheme WRITE setDarkTheme NOTIFY darkThemeChanged)
    Q_PROPERTY(QString syntaxName READ syntaxName NOTIFY syntaxNameChanged)
    Q_PROPERTY(bool languageRecognized READ languageRecognized NOTIFY languageRecognizedChanged)

  public:
    explicit SyntaxDocumentHighlighter(QObject* parent = nullptr);
    ~SyntaxDocumentHighlighter() override;

    [[nodiscard]] QQuickTextDocument* textDocument() const;
    void setTextDocument(QQuickTextDocument* document);

    [[nodiscard]] QString language() const;
    void setLanguage(const QString& language);

    [[nodiscard]] bool darkTheme() const;
    void setDarkTheme(bool darkTheme);

    [[nodiscard]] QString syntaxName() const;
    [[nodiscard]] bool languageRecognized() const;

  signals:
    void textDocumentChanged();
    void languageChanged();
    void darkThemeChanged();
    void syntaxNameChanged();
    void languageRecognizedChanged();

  protected:
    void highlightBlock(const QString& text) override;

  private:
    struct Private;

    void attachDocument();
    void scheduleHighlight();
    void dispatchHighlight();
    void applyFinishedHighlight();
    void rehighlightDocument();
    void setSyntaxResolution(QString syntaxName, bool languageRecognized);

    std::unique_ptr<Private> d;
};
