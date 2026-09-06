// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/markup/markupdocumentmodel.h"
#include "ward/markup/markupsemanticmodel.h"
#include "ward/markup/markuptextdocument.h"

#include <QAbstractItemModelTester>
#include <QAbstractTextDocumentLayout>
#include <QFontMetricsF>
#include <QPersistentModelIndex>
#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QQuickItem>
#include <QSignalSpy>
#include <QTextBlock>
#include <QTextCursor>
#include <QTextList>
#include <QTextTable>
#include <QtTest/QTest>

namespace {
using namespace ward::markup::v1;
using Format = MarkupDocumentModel::SourceFormat;

QVariant
payload(QAbstractItemModel* model, int row)
{
    return model->data(model->index(row, 0), MarkupSemanticModel::SemanticSegmentRole);
}

QTextCharFormat
formatAt(QTextDocument* document, const QString& text)
{
    const int position = document->toPlainText().indexOf(text);
    if (position < 0)
        return {};
    QTextCursor cursor(document);
    cursor.setPosition(position);
    cursor.movePosition(QTextCursor::NextCharacter, QTextCursor::KeepAnchor);
    return cursor.charFormat();
}

class NativeText
{
  public:
    explicit NativeText(const QVariant& segment)
      : component(&engine)
    {
        engine.rootContext()->setContextProperty(QStringLiteral("semanticPayload"), segment);
        component.setData(R"(
            import QtQuick
            import Craftward.Markup
            TextEdit {
                id: nativeText
                width: 480
                font.family: "Helvetica Neue"
                font.pixelSize: 16
                readOnly: true
                selectByMouse: true
                wrapMode: TextEdit.Wrap
                textFormat: TextEdit.RichText
                MarkupTextDocument {
                    objectName: "adapter"
                    textDocument: nativeText.textDocument
                    segment: semanticPayload
                    font: nativeText.font
                    codeFont { family: "Menlo"; pixelSize: 16 }
                    codeBackground: "#eeeeee"
                }
                function targetAt(position) {
                    const rect = positionToRectangle(position);
                    return linkAt(rect.x + 2, rect.y + rect.height / 2);
                }
            }
        )",
                          QUrl(QStringLiteral("qrc:/MarkupSemanticTest.qml")));
        object.reset(component.create());
    }

    QTextDocument* document() const
    {
        return object ? object->property("textDocument").value<QQuickTextDocument*>()->textDocument() : nullptr;
    }

    QQmlEngine engine;
    QQmlComponent component;
    std::unique_ptr<QObject> object;
};
}

class MarkupSemanticTest : public QObject
{
    Q_OBJECT

  private slots:
    void reconcilesOnlyChangedSegments();
    void resolvesReferencesAcrossTheCompleteSnapshot();
    void rendersInlineFormatsAndNativeLinkHits();
    void preservesEmphasisAroundInlineCode();
    void productionSegmentConsumesSemanticPayload();
    void splitsListsAndTablesAtStableBoundaries();
    void placesNestedTablesBelowThePrecedingParagraph();
    void preservesCodeAndUnsupportedSource();
    void keepsUnchangedTextSelectionAndReleasesDocuments();
    void discardsObsoleteSnapshots();
    void preservesGeometryAcrossPaletteChanges();
    void productionSegmentsDoNotOverlapAfterPaletteChanges();
    void updatesGeometryWhenReplacingSegments();
};

void
MarkupSemanticTest::reconcilesOnlyChangedSegments()
{
    MarkupDocumentModel document;
    const QString initial = QStringLiteral("First **stable**.\n\n").repeated(8) + QStringLiteral("Tail");
    document.reconcileSource(initial, Format::Markdown, false);
    auto* model = document.semanticModel();
    QAbstractItemModelTester tester(model, QAbstractItemModelTester::FailureReportingMode::QtTest);
    QTRY_COMPARE(model->rowCount(), 2);
    const QPersistentModelIndex first(model->index(0, 0));
    const auto firstPayload = payload(model, 0);
    QSignalSpy changed(model, &QAbstractItemModel::dataChanged);
    QSignalSpy reset(model, &QAbstractItemModel::modelReset);
    document.reconcileSource(initial + QStringLiteral(" grows"), Format::Markdown, false);
    QTRY_COMPARE(model->data(model->index(1, 0), MarkupSemanticModel::SegmentTextRole).toString(),
                 QStringLiteral("Tail grows"));
    QCOMPARE(changed.size(), 1);
    QCOMPARE(changed.first().at(0).value<QModelIndex>().row(), 1);
    QVERIFY(first.isValid());
    QCOMPARE(payload(model, 0), firstPayload);
    QCOMPARE(reset.size(), 0);
}

void
MarkupSemanticTest::resolvesReferencesAcrossTheCompleteSnapshot()
{
    MarkupDocumentModel document;
    document.reconcileSource(
      QStringLiteral("[Reference][ref]\n\nA later paragraph.\n\n[ref]: https://example.com/target \"Hint\""),
      Format::Markdown);
    auto* model = document.semanticModel();
    QTRY_COMPARE(model->rowCount(), 1);
    NativeText text(payload(model, 0));
    QVERIFY2(text.object, qPrintable(text.component.errorString()));
    QCOMPARE(text.document()->toPlainText(), QStringLiteral("Reference\nA later paragraph."));
    QCOMPARE(formatAt(text.document(), QStringLiteral("Reference")).anchorHref(),
             QStringLiteral("https://example.com/target"));

    document.reconcileSource(
      QStringLiteral("[Reference][ref]\n\nA later paragraph.\n\n[ref]: https://example.com/changed"), Format::Markdown);
    QTRY_VERIFY(payload(model, 0) != text.object->findChild<MarkupTextDocument*>()->segment());
    text.object->findChild<MarkupTextDocument*>()->setSegment(payload(model, 0));
    QCOMPARE(formatAt(text.document(), QStringLiteral("Reference")).anchorHref(),
             QStringLiteral("https://example.com/changed"));
}

void
MarkupSemanticTest::rendersInlineFormatsAndNativeLinkHits()
{
    MarkupDocumentModel document;
    document.reconcileSource(
      QString::fromUtf8("**Bold** *em* ~~gone~~ `print \"hello world\"` [link](https://example.com \"Hint\") "
                        ":codex-annotation{index=\"4\"} عربي 😀 é &amp;  \nnext"),
      Format::Markdown);
    auto* model = document.semanticModel();
    QTRY_COMPARE(model->rowCount(), 1);
    NativeText text(payload(model, 0));
    QVERIFY2(text.object, qPrintable(text.component.errorString()));
    QCOMPARE(text.document()->toPlainText(),
             QString::fromUtf8("Bold em gone print \"hello world\" link [4] عربي 😀 é &\nnext"));
    QVERIFY(formatAt(text.document(), QStringLiteral("Bold")).fontWeight() >= QFont::Bold);
    QVERIFY(formatAt(text.document(), QStringLiteral("em")).fontItalic());
    QVERIFY(formatAt(text.document(), QStringLiteral("gone")).fontStrikeOut());
    QCOMPARE(formatAt(text.document(), QStringLiteral("print")).font().family(), QStringLiteral("Menlo"));
    QCOMPARE(formatAt(text.document(), QStringLiteral("link")).toolTip(), QStringLiteral("Hint"));
    const int linkPosition = text.document()->toPlainText().indexOf(QStringLiteral("link"));
    QVariant target;
    QVERIFY(QMetaObject::invokeMethod(
      text.object.get(), "targetAt", Q_RETURN_ARG(QVariant, target), Q_ARG(QVariant, linkPosition)));
    QCOMPARE(target.toString(), QStringLiteral("https://example.com"));
    QVERIFY(QMetaObject::invokeMethod(text.object.get(), "selectAll"));
    QCOMPARE(text.object->property("selectedText").toString().replace(QChar::LineSeparator, QLatin1Char('\n')),
             text.document()->toPlainText());
}

void
MarkupSemanticTest::preservesEmphasisAroundInlineCode()
{
    MarkupDocumentModel document;
    document.reconcileSource(QStringLiteral("**`bold code`** *`italic code`* ~~`deleted code`~~"), Format::Markdown);
    auto* model = document.semanticModel();
    QTRY_COMPARE(model->rowCount(), 1);
    NativeText text(payload(model, 0));
    QVERIFY(text.object);
    QVERIFY(formatAt(text.document(), QStringLiteral("bold")).fontWeight() >= QFont::Bold);
    QVERIFY(formatAt(text.document(), QStringLiteral("italic")).fontItalic());
    QVERIFY(formatAt(text.document(), QStringLiteral("deleted")).fontStrikeOut());
    QCOMPARE(formatAt(text.document(), QStringLiteral("bold")).font().family(), QStringLiteral("Menlo"));
}

void
MarkupSemanticTest::productionSegmentConsumesSemanticPayload()
{
    MarkupDocumentModel document;
    document.reconcileSource(QStringLiteral("Native **text** :codex-annotation{index=\"4\"}"), Format::Markdown);
    auto* model = document.semanticModel();
    QTRY_COMPARE(model->rowCount(), 1);
    QQmlEngine engine;
    engine.rootContext()->setContextProperty(QStringLiteral("semanticPayload"), payload(model, 0));
    QQmlComponent component(&engine);
    component.setData(R"(
        import QtQuick
        import Craftward.Components
        MarkupSegmentView {
            width: 480
            codeBlock: false
            segmentText: "Unparsed **source**"
            language: ""
            markdown: true
            semanticSegment: semanticPayload
        }
    )",
                      QUrl(QStringLiteral("qrc:/ProductionSemanticSegmentTest.qml")));
    const std::unique_ptr<QObject> view(component.create());
    QVERIFY2(view, qPrintable(component.errorString()));
    auto* adapter = view->findChild<MarkupTextDocument*>(QStringLiteral("markupNativeAdapter"));
    QVERIFY(adapter);
    QCOMPARE(adapter->textDocument()->textDocument()->toPlainText(), QStringLiteral("Native text [4]"));
    QVERIFY(view->property("implicitHeight").toReal() > 0);
}

void
MarkupSemanticTest::splitsListsAndTablesAtStableBoundaries()
{
    MarkupDocumentModel document;
    const QString source =
      QStringLiteral("7. First **item**\n8. Second item\n\n| A | B |\n| :-- | --: |\n") +
      QStringLiteral("| `code` | [link](https://example.com) :codex-annotation{index=\"2\"} |\n").repeated(15);
    document.reconcileSource(source, Format::Markdown);
    auto* model = document.semanticModel();
    QTRY_COMPARE(model->rowCount(), 2);
    NativeText list(payload(model, 0));
    QVERIFY2(list.object, qPrintable(list.component.errorString()));
    QVERIFY(list.document()->firstBlock().textList());
    QCOMPARE(list.document()->firstBlock().textList()->format().start(), 7);
    QCOMPARE(list.document()->firstBlock().textList()->count(), 2);
    NativeText table(payload(model, 1));
    QVERIFY2(table.object, qPrintable(table.component.errorString()));
    const auto frames = table.document()->rootFrame()->childFrames();
    QCOMPARE(frames.size(), 1);
    auto* nativeTable = qobject_cast<QTextTable*>(frames.first());
    QVERIFY(nativeTable);
    QCOMPARE(nativeTable->rows(), 16);
    QCOMPARE(nativeTable->columns(), 2);
    QCOMPARE(nativeTable->cellAt(0, 1).firstCursorPosition().blockFormat().alignment(), Qt::AlignRight);
    QCOMPARE(formatAt(table.document(), QStringLiteral("code")).font().family(), QStringLiteral("Menlo"));
    QCOMPARE(formatAt(table.document(), QStringLiteral("link")).anchorHref(), QStringLiteral("https://example.com"));
    QVERIFY(table.document()->toPlainText().contains(QStringLiteral("[2]")));
    const auto firstBody = payload(model, 1);
    QSignalSpy changed(model, &QAbstractItemModel::dataChanged);
    document.reconcileSource(source + QStringLiteral("| More | cells |\n").repeated(128), Format::Markdown);
    QTRY_COMPARE(model->rowCount(), 10);
    QCOMPARE(payload(model, 1), firstBody);
    QCOMPARE(changed.size(), 0);
}

void
MarkupSemanticTest::placesNestedTablesBelowThePrecedingParagraph()
{
    MarkupDocumentModel document;
    document.reconcileSource(QStringLiteral("> Introduction\n>\n> | A | B |\n> | --- | --- |\n> | one | two |\n"),
                             Format::Markdown);
    auto* model = document.semanticModel();
    QTRY_COMPARE(model->rowCount(), 1);
    NativeText text(payload(model, 0));
    QVERIFY(text.object);
    const auto first = text.document()->firstBlock();
    QCOMPARE(first.text(), QStringLiteral("Introduction"));
    QVERIFY(first.blockFormat().leftMargin() > 0);
    const auto frames = text.document()->rootFrame()->childFrames();
    QCOMPARE(frames.size(), 1);
    const auto* layout = text.document()->documentLayout();
    const QRectF paragraphRect = layout->blockBoundingRect(first);
    QVERIFY(paragraphRect.height() >= QFontMetricsF(text.document()->defaultFont()).height());
    QVERIFY(layout->frameBoundingRect(frames.first()).top() >= paragraphRect.bottom());
}

void
MarkupSemanticTest::preservesCodeAndUnsupportedSource()
{
    MarkupDocumentModel document;
    document.reconcileSource(
      QStringLiteral("```python\n  print(\"hello\")\n\n```\n\n![alt](image.png)\n\n<div>literal &amp;</div>\n"),
      Format::Markdown);
    auto* model = document.semanticModel();
    QTRY_COMPARE(model->rowCount(), 3);
    QCOMPARE(model->data(model->index(0, 0), MarkupSemanticModel::CodeBlockRole).toBool(), true);
    QCOMPARE(model->data(model->index(0, 0), MarkupSemanticModel::SegmentTextRole).toString(),
             QStringLiteral("  print(\"hello\")\n"));
    QVERIFY(!payload(model, 1).isValid());
    QCOMPARE(model->data(model->index(1, 0), MarkupSemanticModel::SegmentTextRole).toString().trimmed(),
             QStringLiteral("![alt](image.png)"));
    QVERIFY(!model->data(model->index(2, 0), MarkupSemanticModel::MarkdownRole).toBool());
    QVERIFY(model->data(model->index(2, 0), MarkupSemanticModel::SegmentTextRole)
              .toString()
              .contains(QStringLiteral("<div>literal &amp;</div>")));

    document.reconcileSource(QStringLiteral("**plain** :codex-annotation{index=\"4\"}"), Format::PlainText);
    QTRY_COMPARE(model->rowCount(), 1);
    NativeText plain(payload(model, 0));
    QVERIFY(plain.object);
    QCOMPARE(plain.document()->toPlainText(), QStringLiteral("**plain** :codex-annotation{index=\"4\"}"));

    document.reconcileSource(QStringLiteral("```\nunlabelled code\n```"), Format::Markdown);
    QTRY_VERIFY(model->data(model->index(0, 0), MarkupSemanticModel::CodeBlockRole).toBool());
    QCOMPARE(model->data(model->index(0, 0), MarkupSemanticModel::SegmentTextRole).toString(),
             QStringLiteral("unlabelled code"));
    QVERIFY(model->data(model->index(0, 0), MarkupSemanticModel::LanguageRole).toString().isEmpty());
}

void
MarkupSemanticTest::keepsUnchangedTextSelectionAndReleasesDocuments()
{
    MarkupDocumentModel document;
    document.reconcileSource(QStringLiteral("Select **this** text."), Format::Markdown);
    auto* model = document.semanticModel();
    QTRY_COMPARE(model->rowCount(), 1);
    NativeText text(payload(model, 0));
    QVERIFY(text.object);
    auto* adapter = text.object->findChild<MarkupTextDocument*>();
    QVERIFY(adapter);
    QMetaObject::invokeMethod(text.object.get(), "selectAll");
    const auto selected = text.object->property("selectedText");
    QSignalSpy changed(text.document(), &QTextDocument::contentsChanged);
    adapter->setSegment(payload(model, 0));
    QCOMPARE(changed.size(), 0);
    QCOMPARE(text.object->property("selectedText"), selected);
    QPointer<QTextDocument> layout(text.document());
    text.object.reset();
    QVERIFY(!layout);
    NativeText rematerialized(payload(model, 0));
    QVERIFY(rematerialized.object);
    QCOMPARE(rematerialized.document()->toPlainText(), QStringLiteral("Select this text."));
}

void
MarkupSemanticTest::preservesGeometryAcrossPaletteChanges()
{
    MarkupDocumentModel document;
    document.reconcileSource(QStringLiteral("One paragraph."), Format::Markdown);
    auto* model = document.semanticModel();
    QTRY_COMPARE(model->rowCount(), 1);
    NativeText text(payload(model, 0));
    QVERIFY(text.object);
    auto* adapter = text.object->findChild<MarkupTextDocument*>();
    QVERIFY(adapter);
    const qreal height = text.object->property("implicitHeight").toReal();
    QVERIFY(height > 0);
    const auto plainText = text.document()->toPlainText();
    for (int iteration = 0; iteration < 20; ++iteration) {
        QVERIFY(adapter->setProperty("textColor", iteration % 2 ? QColor(Qt::black) : QColor(Qt::darkGray)));
        QCoreApplication::processEvents();
        QCOMPARE(text.object->property("implicitHeight").toReal(), height);
        QCOMPARE(text.document()->toPlainText(), plainText);
    }
}

void
MarkupSemanticTest::productionSegmentsDoNotOverlapAfterPaletteChanges()
{
    MarkupDocumentModel document;
    document.reconcileSource(
      QStringLiteral("A **paragraph** with enough words to wrap across multiple lines. ").repeated(6) +
        QStringLiteral("\n\nA [link](https://example.com) and `inline code`.\n\n") +
        QString::fromUtf8("中文段落也应保持正确的行高和位置。\n\n") +
        QStringLiteral("| A | B |\n| --- | --- |\n| text | **bold** |"),
      Format::Markdown);
    auto* model = document.semanticModel();
    QTRY_COMPARE(model->rowCount(), 2);
    QQmlEngine engine;
    engine.rootContext()->setContextProperty(QStringLiteral("semanticPayloads"),
                                             QVariantList{ payload(model, 0), payload(model, 1) });
    QQmlComponent component(&engine);
    component.setData(R"(
        import QtQuick
        import Craftward.Components
        Column {
            id: fixture
            property bool inactive: false
            width: 480
            Repeater {
                model: 2
                MarkupSegmentView {
                    required property int index
                    objectName: "segment" + index
                    width: fixture.width
                    font { family: "Helvetica Neue"; pixelSize: 16 }
                    palette.text: fixture.inactive ? "#505050" : "#101010"
                    palette.link: fixture.inactive ? "#606080" : "#0000ff"
                    codeBlock: false
                    segmentText: ""
                    language: ""
                    markdown: true
                    semanticSegment: semanticPayloads[index]
                }
            }
            Text {
                objectName: "followingMessage"
                text: "The following message must remain below the semantic segments."
            }
        }
    )",
                      QUrl(QStringLiteral("qrc:/SemanticPaletteGeometryTest.qml")));
    QList<qreal> reportedHeights;
    const std::unique_ptr<QObject> view(component.create());
    QVERIFY2(view, qPrintable(component.errorString()));
    QQuickItem* first = nullptr;
    QQuickItem* second = nullptr;
    for (auto* item : qobject_cast<QQuickItem*>(view.get())->childItems()) {
        if (item->objectName() == QStringLiteral("segment0"))
            first = item;
        else if (item->objectName() == QStringLiteral("segment1"))
            second = item;
    }
    QVERIFY(first);
    QVERIFY(second);
    auto* following = view->findChild<QQuickItem*>(QStringLiteral("followingMessage"));
    QVERIFY(following);
    QVERIFY(QMetaObject::invokeMethod(view.get(), "forceLayout"));
    const qreal height = first->implicitHeight();
    const qreal secondHeight = second->implicitHeight();
    QVERIFY(height > 100);
    QVERIFY(secondHeight > 0);
    auto* adapter = first->findChild<MarkupTextDocument*>();
    QVERIFY(adapter);
    const auto plainText = adapter->textDocument()->textDocument()->toPlainText();
    connect(first, &QQuickItem::implicitHeightChanged, this, [first, &reportedHeights] {
        reportedHeights.append(first->implicitHeight());
    });
    for (int iteration = 0; iteration < 20; ++iteration) {
        const bool inactive = iteration % 2 == 0;
        QVERIFY(view->setProperty("inactive", inactive));
        QCoreApplication::processEvents();
        QVERIFY(QMetaObject::invokeMethod(view.get(), "forceLayout"));
        QCOMPARE(adapter->property("textColor").value<QColor>(), QColor(inactive ? "#505050" : "#101010"));
        QVERIFY2(following->y() >= first->y() + height + secondHeight,
                 "The following message overlaps semantic text after a palette change.");
        QCOMPARE(first->implicitHeight(), height);
        QCOMPARE(second->implicitHeight(), secondHeight);
        QVERIFY2(second->y() >= first->y() + height, "Adjacent semantic segments overlap after a palette change.");
        QCOMPARE(adapter->textDocument()->textDocument()->toPlainText(), plainText);
    }
    for (qreal reported : reportedHeights)
        QCOMPARE(reported, height);
}

void
MarkupSemanticTest::updatesGeometryWhenReplacingSegments()
{
    MarkupDocumentModel document;
    auto* model = document.semanticModel();
    const QStringList sources = {
        QStringLiteral("Initial paragraph."),
        QStringLiteral("A growing paragraph that wraps across several lines. ").repeated(12),
        QStringLiteral("| A | B |\n| --- | --- |\n| a | b |"),
        QStringLiteral("1. First item\n2. Second item"),
        QStringLiteral("A short replacement."),
    };
    document.reconcileSource(sources.first(), Format::Markdown);
    QTRY_COMPARE(model->rowCount(), 1);
    NativeText text(payload(model, 0));
    QVERIFY(text.object);
    auto* adapter = text.object->findChild<MarkupTextDocument*>();
    QVERIFY(adapter);
    for (const auto& source : sources.sliced(1)) {
        const auto previous = payload(model, 0);
        document.reconcileSource(source, Format::Markdown);
        QTRY_VERIFY(payload(model, 0) != previous);
        adapter->setSegment(payload(model, 0));
        NativeText fresh(payload(model, 0));
        QVERIFY(fresh.object);
        QCOMPARE(text.document()->toPlainText(), fresh.document()->toPlainText());
        QCOMPARE(text.object->property("implicitHeight"), fresh.object->property("implicitHeight"));
        QCOMPARE(text.document()->size(), fresh.document()->size());
    }
}

void
MarkupSemanticTest::discardsObsoleteSnapshots()
{
    MarkupDocumentModel document;
    auto* model = document.semanticModel();
    document.reconcileSource(QStringLiteral("A paragraph.\n\n").repeated(3000), Format::Markdown);
    QCoreApplication::processEvents();
    document.reconcileSource(QStringLiteral("Newest snapshot"), Format::PlainText);
    QTRY_COMPARE(model->rowCount(), 1);
    QCOMPARE(model->data(model->index(0, 0), MarkupSemanticModel::SegmentTextRole).toString(),
             QStringLiteral("Newest snapshot"));
    document.reconcileSource({}, Format::Markdown);
    QTRY_COMPARE(model->rowCount(), 0);
}

QTEST_MAIN(MarkupSemanticTest)

#include "markupsemantictest.moc"
