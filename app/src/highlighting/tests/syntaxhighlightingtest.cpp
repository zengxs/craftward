// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "highlighting/syntaxdocumenthighlighter.h"
#include "highlighting/syntaxhighlightingengine.h"

#include <QQmlComponent>
#include <QQmlEngine>
#include <QQuickTextDocument>
#include <QTextBlock>
#include <QTextDocument>
#include <QTextLayout>
#include <QtTest/QTest>

#include <algorithm>
#include <memory>

namespace {
const craftward::highlighting::Span*
spanAtUtf8Range(const craftward::highlighting::Result& highlighted, qsizetype start, qsizetype end)
{
    const auto span = std::find_if(highlighted.spans.cbegin(),
                                   highlighted.spans.cend(),
                                   [start, end](const craftward::highlighting::Span& candidate) {
                                       return candidate.utf8Start == start && candidate.utf8End == end;
                                   });
    return span == highlighted.spans.cend() ? nullptr : std::addressof(*span);
}

bool
hasFormat(const QTextDocument& document, int start, int length, const QColor& foreground)
{
    const QTextBlock block = document.firstBlock();
    if (!block.isValid() || !block.layout())
        return false;
    const QList<QTextLayout::FormatRange> formats = block.layout()->formats();
    return std::any_of(
      formats.cbegin(), formats.cend(), [start, length, &foreground](const QTextLayout::FormatRange& range) {
          return range.start == start && range.length == length && range.format.foreground().color() == foreground;
      });
}
}

class SyntaxHighlightingTest : public QObject
{
    Q_OBJECT

  private slots:
    void loadsMaintainedResources();
    void appliesUtf8RangesToAQTextDocument();
};

void
SyntaxHighlightingTest::loadsMaintainedResources()
{
    const auto engine = craftward::highlighting::SyntaxHighlightingEngine::shared();
    const QByteArray source = QString::fromUtf8("😀 let answer = 42;\n").toUtf8();

    const craftward::highlighting::Result highlighted =
      engine->highlight(source, QByteArrayLiteral("rust"), craftward::highlighting::Theme::Light);

    QVERIFY2(highlighted.succeeded(), qPrintable(highlighted.errorMessage));
    QCOMPARE(highlighted.syntaxName, QStringLiteral("Rust"));
    QVERIFY(highlighted.languageRecognized);
    const craftward::highlighting::Span* keyword = spanAtUtf8Range(highlighted, 5, 8);
    QVERIFY(keyword);
    QVERIFY(keyword->style.foreground.isValid());

    const craftward::highlighting::Result plain = engine->highlight(source, {}, craftward::highlighting::Theme::Light);
    QVERIFY2(plain.succeeded(), qPrintable(plain.errorMessage));
    QCOMPARE(plain.syntaxName, QStringLiteral("Plain Text"));
    QVERIFY(!plain.languageRecognized);

    const craftward::highlighting::Result regex = engine->highlight(
      QByteArrayLiteral("^(?<word>\\w+)$"), QByteArrayLiteral("regex"), craftward::highlighting::Theme::Light);
    QVERIFY2(regex.succeeded(), qPrintable(regex.errorMessage));
    QCOMPARE(regex.syntaxName, QStringLiteral("Regular Expression"));
    QVERIFY(regex.languageRecognized);
    QVERIFY(!regex.spans.isEmpty());
}

void
SyntaxHighlightingTest::appliesUtf8RangesToAQTextDocument()
{
    const QByteArray source = QString::fromUtf8("😀 let answer = 42;\n").toUtf8();
    const auto engine = craftward::highlighting::SyntaxHighlightingEngine::shared();
    const craftward::highlighting::Result dark =
      engine->highlight(source, QByteArrayLiteral("rust"), craftward::highlighting::Theme::Dark);
    const craftward::highlighting::Result light =
      engine->highlight(source, QByteArrayLiteral("rust"), craftward::highlighting::Theme::Light);
    QVERIFY2(dark.succeeded(), qPrintable(dark.errorMessage));
    QVERIFY2(light.succeeded(), qPrintable(light.errorMessage));
    const craftward::highlighting::Span* darkKeyword = spanAtUtf8Range(dark, 5, 8);
    const craftward::highlighting::Span* lightKeyword = spanAtUtf8Range(light, 5, 8);
    QVERIFY(darkKeyword);
    QVERIFY(lightKeyword);
    QVERIFY(darkKeyword->style.foreground != lightKeyword->style.foreground);

    QQmlEngine qmlEngine;
    QQmlComponent component(&qmlEngine);
    component.setData("import QtQuick\nTextEdit {}", QUrl(QStringLiteral("qrc:/SyntaxHighlightingTest.qml")));
    QVERIFY2(component.isReady(), qPrintable(component.errorString()));
    const std::unique_ptr<QObject> textEdit(component.create());
    QVERIFY(textEdit);
    QVERIFY(textEdit->setProperty("text", QString::fromUtf8(source)));
    QQuickTextDocument* quickDocument = textEdit->property("textDocument").value<QQuickTextDocument*>();
    QVERIFY(quickDocument);
    QTextDocument* document = quickDocument->textDocument();
    QVERIFY(document);

    SyntaxDocumentHighlighter highlighter;
    highlighter.setLanguage(QStringLiteral("rust"));
    highlighter.setDarkTheme(true);
    highlighter.setTextDocument(quickDocument);

    QTRY_VERIFY_WITH_TIMEOUT(hasFormat(*document, 3, 3, darkKeyword->style.foreground), 5000);
    QTRY_COMPARE_WITH_TIMEOUT(highlighter.syntaxName(), QStringLiteral("Rust"), 5000);
    QVERIFY(highlighter.languageRecognized());

    highlighter.setDarkTheme(false);
    QTRY_VERIFY_WITH_TIMEOUT(hasFormat(*document, 3, 3, lightKeyword->style.foreground), 5000);
}

QTEST_MAIN(SyntaxHighlightingTest)

#include "syntaxhighlightingtest.moc"
