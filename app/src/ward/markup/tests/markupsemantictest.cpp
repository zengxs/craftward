// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/markup/markupdocumentmodel.h"
#include "ward/markup/markupselection.h"
#include "ward/markup/markuptextdocument.h"

#include "document.qpb.h"

#include <ward_core.h>

#include <QAbstractItemModelTester>
#include <QAbstractTextDocumentLayout>
#include <QByteArrayView>
#include <QClipboard>
#include <QFontMetricsF>
#include <QGuiApplication>
#include <QPainterPath>
#include <QPersistentModelIndex>
#include <QQmlComponent>
#include <QQmlContext>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickWindow>
#include <QRawFont>
#include <QScopeGuard>
#include <QSignalSpy>
#include <QTextBlock>
#include <QTextCursor>
#include <QTextFrame>
#include <QTextLayout>
#include <QTextList>
#include <QtProtobuf/QProtobufSerializer>
#include <QtTest/QTest>

namespace {
using namespace ward::markup::v1;
using Format = MarkupDocumentModel::SourceFormat;

QVariant
payload(QAbstractItemModel* model, int row)
{
    return model->data(model->index(row, 0), MarkupDocumentModel::RenderPartsRole);
}

QVariant
textPayload(const QVariant& value)
{
    if (value.canConvert<MarkupTextSurface>())
        return value;
    const auto surfaces = markupSurfaces(value.toList());
    return surfaces.isEmpty() ? QVariant() : QVariant::fromValue(surfaces.first());
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

QList<QQuickItem*>
visualItems(QObject* root, const QString& name)
{
    QList<QQuickItem*> result;
    auto* item = qobject_cast<QQuickItem*>(root);
    if (!item)
        return result;
    if (item->objectName() == name)
        result.append(item);
    for (auto* child : item->childItems())
        result.append(visualItems(child, name));
    return result;
}

MarkupTextDocument*
nativeAdapter(QObject* root)
{
    for (auto* editor : visualItems(root, QStringLiteral("markupProseText"))) {
        if (auto* adapter = editor->findChild<MarkupTextDocument*>())
            return adapter;
    }
    return root->findChild<MarkupTextDocument*>();
}

class NativeText
{
  public:
    explicit NativeText(const QVariant& segment)
      : component(&engine)
    {
        engine.rootContext()->setContextProperty(QStringLiteral("semanticPayload"), textPayload(segment));
        component.setData(R"(
            import QtQuick
            import Craftward.Components
            MarkupSelectableText {
                id: nativeText
                width: 480
                font.family: "Helvetica Neue"
                font.pixelSize: 16
                readOnly: true
                selectByMouse: true
                wrapMode: TextEdit.Wrap
                textFormat: TextEdit.RichText
                surface: semanticPayload
                codeFont { family: "Menlo"; pixelSize: 16 }
                codeBackground: "#eeeeee"
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
class SelectionScene
{
  public:
    SelectionScene(MarkupDocumentModel* model, bool virtualized = false, MarkupDocumentModel* other = nullptr)
      : component(&engine)
    {
        engine.rootContext()->setContextProperty(QStringLiteral("messageModel"), model);
        engine.rootContext()->setContextProperty(QStringLiteral("useVirtualization"), virtualized);
        engine.rootContext()->setContextProperty(QStringLiteral("otherMessage"), other);
        engine.rootContext()->setContextProperty(QStringLiteral("otherParts"), other ? payload(other, 0) : QVariant());
        component.setData(R"(
            import QtQuick
            import Craftward.Components
            Item {
                id: scene
                width: 520
                height: 520
                ListView {
                    id: timeline
                    objectName: "selectionTimeline"
                    anchors.fill: parent
                    clip: true
                    currentIndex: -1
                    cacheBuffer: useVirtualization ? 0 : 10000
                    model: messageModel
                    delegate: MarkupSegmentView {
                        required property var model
                        width: timeline.width
                        codeBlock: model.codeBlock
                        segmentText: model.segmentText
                        language: model.language
                        renderParts: model.renderParts
                        selectionCoordinator: messageModel.selection
                        selectionHost: host
                        font { family: "Helvetica Neue"; pixelSize: 16 }
                        codeFont { family: "Menlo"; pixelSize: 16 }
                    }
                }
                Loader {
                    active: otherMessage !== null
                    y: timeline.contentHeight + 20
                    width: parent.width
                    sourceComponent: MarkupSegmentView {
                        codeBlock: false
                        segmentText: ""
                        language: ""
                        renderParts: otherParts
                        selectionCoordinator: otherMessage ? otherMessage.selection : null
                        selectionHost: host
                        font { family: "Helvetica Neue"; pixelSize: 16 }
                    }
                }
                MarkupSelectionHost {
                    id: host
                    objectName: "selectionHost"
                    anchors.fill: parent
                    viewport: timeline
                }
            }
        )",
                          QUrl(QStringLiteral("qrc:/MessageSelectionScene.qml")));
        view.reset(qobject_cast<QQuickItem*>(component.create()));
        window.setColor(Qt::white);
        window.resize(520, 520);
        if (view)
            view->setParentItem(window.contentItem());
    }

    QList<QQuickItem*> editors() const
    {
        return visualItems(view.get(), QStringLiteral("markupProseText")) +
               visualItems(view.get(), QStringLiteral("markupCodeText"));
    }

    QQuickItem* editorContaining(const QString& text) const
    {
        for (auto* editor : editors()) {
            const auto* document = editor->property("textDocument").value<QQuickTextDocument*>()->textDocument();
            if (document->toPlainText().contains(text))
                return editor;
        }
        return nullptr;
    }

    static QPoint pointAt(QQuickItem* editor, int position)
    {
        QRectF rect;
        QMetaObject::invokeMethod(editor, "positionToRectangle", Q_RETURN_ARG(QRectF, rect), Q_ARG(int, position));
        return editor->mapToScene(QPointF(rect.x() + 0.5, rect.center().y())).toPoint();
    }

    QQmlEngine engine;
    QQmlComponent component;
    QQuickWindow window;
    std::unique_ptr<QQuickItem> view;
};

}

class MarkupSemanticTest : public QObject
{
    Q_OBJECT

  private slots:
    void decodesSemanticSnapshotFromRust();
    void reconcilesOnlyChangedSegments();
    void resolvesReferencesAcrossTheCompleteSnapshot();
    void rendersInlineFormatsAndNativeLinkHits();
    void preservesEmphasisAroundInlineCode();
    void resolvesChineseBoldWithoutChangingRegularOrCodeFonts();
    void confinesInlineCodeBackgroundToItsText_data();
    void confinesInlineCodeBackgroundToItsText();
    void decoratesInlineCodeWithoutChangingTextLayout_data();
    void decoratesInlineCodeWithoutChangingTextLayout();
    void productionSegmentConsumesSemanticPayload();
    void splitsListsAndTablesAtStableBoundaries();
    void distinguishesListSpacing_data();
    void distinguishesListSpacing();
    void sizesListIndentationAndNestedMarkers();
    void enlargesDiscMarkersWithoutMovingText();
    void alignsListMarkersWithInlineText_data();
    void alignsListMarkersWithInlineText();
    void preservesListLayoutAcrossSegments_data();
    void preservesListLayoutAcrossSegments();
    void preservesListSpacingAcrossEmptySegments_data();
    void preservesListSpacingAcrossEmptySegments();
    void alignsNestedListTablesWithText();
    void preservesBlockSpacingBeforeLists_data();
    void preservesBlockSpacingBeforeLists();
    void placesNestedTablesBelowThePrecedingParagraph();
    void preservesCodeAndUnsupportedSource();
    void keepsUnchangedTextSelectionAndReleasesDocuments();
    void discardsObsoleteSnapshots();
    void preservesGeometryAcrossPaletteChanges();
    void productionSegmentsDoNotOverlapAfterPaletteChanges();
    void updatesGeometryWhenReplacingSegments();
    void tableSelectionPreservesGlyphRendering_data();
    void tableSelectionPreservesGlyphRendering();
    void copiesAcrossSegmentsAndPreservesStreamingSelection();
    void mouseSelectionSpansProseCodeAndCells();
    void selectionSurvivesVirtualizedDelegateDestruction();
    void confinesMouseSelectionToOneMessage();
    void copiesEmptyCellsInReadingOrder();
    void selectsTextAfterNormalizedLineBreaks_data();
    void selectsTextAfterNormalizedLineBreaks();
    void hydratesCodeAfterDelegateCreation_data();
    void hydratesCodeAfterDelegateCreation();
    void preservesCodeHighlightingDuringSelection_data();
    void preservesCodeHighlightingDuringSelection();
    void codeSelectionBackgroundMatchesNativeGeometry_data();
    void codeSelectionBackgroundMatchesNativeGeometry();
    void codeSelectionBackgroundFollowsLayoutAndScrolling();
    void joinsSelectionWithinParagraphs_data();
    void joinsSelectionWithinParagraphs();
    void selectsEmptyLinesWithContinuousBackground_data();
    void selectsEmptyLinesWithContinuousBackground();
    void preservesSelectedCodeDecorations_data();
    void preservesSelectedCodeDecorations();
};

void
MarkupSemanticTest::decodesSemanticSnapshotFromRust()
{
    const QByteArray source =
      QStringLiteral("你好 👩‍💻 &amp; **bold** :codex-annotation{index=\"4\"}\n\n"
                     "0. [ ] task\n\n| A | B |\n|---|---:|\n| `a()` | [Ready][r] |\n\n[r]: /ready \"Status\"")
        .toUtf8();
    WardError* error = nullptr;
    const auto releaseError = qScopeGuard([&] {
        if (error)
            ward_core_error_destroy(error);
    });
    using Buffer = std::unique_ptr<WardOwnedBuffer, decltype(&ward_core_owned_buffer_destroy)>;
    Buffer buffer(
      ward_core_markup_parse_semantic(
        WardMarkupSourceFormatMarkdown, reinterpret_cast<const uint8_t*>(source.constData()), source.size(), &error),
      &ward_core_owned_buffer_destroy);
    QVERIFY(buffer);
    QVERIFY(!error);
    const QByteArrayView bytes(reinterpret_cast<const char*>(ward_core_owned_buffer_data(buffer.get())),
                               ward_core_owned_buffer_size(buffer.get()));
    ward::markup::v1::SemanticDocument document;
    QProtobufSerializer serializer;
    QVERIFY2(document.deserialize(&serializer, bytes), qPrintable(serializer.lastErrorString()));
    buffer.reset();
    QCOMPARE(document.blocks().size(), 3);
    const auto& intro = document.blocks().first().nodes();
    QVERIFY(!intro.first().hasParentIndex());
    QVERIFY(intro.at(1).hasParentIndex());
    QCOMPARE(intro.at(1).parentIndex(), 0u);
    const auto text = intro.at(1).text().value();
    QCOMPARE(text.text(), QStringLiteral("你好 👩‍💻 "));
    QCOMPARE(text.mappings().first().utf16End(), quint64(text.text().size()));
    QCOMPARE(text.mappings().first().source().end(), quint64(text.text().toUtf8().size()));
    QVERIFY(text.mappings().first().verbatim());
    bool annotation = false;
    bool link = false;
    bool uncheckedTask = false;
    bool bodyRow = false;
    bool zeroStart = false;
    bool entity = false;
    for (const auto& block : document.blocks()) {
        for (const auto& node : block.nodes()) {
            if (node.hasAnnotation()) {
                annotation = true;
                QCOMPARE(node.annotation().index(), 4u);
                QCOMPARE(node.annotation().label().text(), QStringLiteral("[4]"));
                QVERIFY(!node.annotation().label().mappings().first().verbatim());
            }
            if (node.hasLink()) {
                link = true;
                QCOMPARE(node.link().target(), QStringLiteral("/ready"));
                QCOMPARE(node.link().title(), QStringLiteral("Status"));
            }
            uncheckedTask |= node.hasTaskChecked() && !node.taskChecked();
            bodyRow |= node.hasTableRowHeader() && !node.tableRowHeader();
            zeroStart |= node.hasList() && node.list().hasStart() && node.list().start() == 0;
            if (node.hasText() && node.text().value().text() == QStringLiteral("&")) {
                entity = true;
                const auto mapping = node.text().value().mappings().first();
                QVERIFY(!mapping.verbatim());
                QCOMPARE(mapping.utf16End(), 1u);
                QCOMPARE(source.mid(mapping.source().start(), mapping.source().end() - mapping.source().start()),
                         QByteArray("&amp;"));
            }
        }
    }
    QVERIFY(annotation && link && uncheckedTask && bodyRow && zeroStart && entity);
}

void
MarkupSemanticTest::reconcilesOnlyChangedSegments()
{
    MarkupDocumentModel document;
    const QString initial = QStringLiteral("First **stable**.\n\n").repeated(8) + QStringLiteral("Tail");
    document.reconcileSource(initial, Format::Markdown, false);
    auto* model = &document;
    QAbstractItemModelTester tester(model, QAbstractItemModelTester::FailureReportingMode::QtTest);
    QTRY_COMPARE(model->rowCount(), 2);
    const QPersistentModelIndex first(model->index(0, 0));
    const auto firstPayload = payload(model, 0);
    QSignalSpy changed(model, &QAbstractItemModel::dataChanged);
    QSignalSpy reset(model, &QAbstractItemModel::modelReset);
    document.reconcileSource(initial + QStringLiteral(" grows"), Format::Markdown, false);
    QTRY_COMPARE(model->data(model->index(1, 0), MarkupDocumentModel::SegmentTextRole).toString(),
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
    auto* model = &document;
    QTRY_COMPARE(model->rowCount(), 1);
    NativeText text(payload(model, 0));
    QVERIFY2(text.object, qPrintable(text.component.errorString()));
    QCOMPARE(text.document()->toPlainText(), QStringLiteral("Reference\nA later paragraph."));
    QCOMPARE(formatAt(text.document(), QStringLiteral("Reference")).anchorHref(),
             QStringLiteral("https://example.com/target"));

    document.reconcileSource(
      QStringLiteral("[Reference][ref]\n\nA later paragraph.\n\n[ref]: https://example.com/changed"), Format::Markdown);
    QTRY_VERIFY(textPayload(payload(model, 0)) != text.object->findChild<MarkupTextDocument*>()->surface());
    text.object->findChild<MarkupTextDocument*>()->setSurface(textPayload(payload(model, 0)));
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
    auto* model = &document;
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
    auto* model = &document;
    QTRY_COMPARE(model->rowCount(), 1);
    NativeText text(payload(model, 0));
    QVERIFY(text.object);
    QVERIFY(formatAt(text.document(), QStringLiteral("bold")).fontWeight() >= QFont::Bold);
    QVERIFY(formatAt(text.document(), QStringLiteral("italic")).fontItalic());
    QVERIFY(formatAt(text.document(), QStringLiteral("deleted")).fontStrikeOut());
    QCOMPARE(formatAt(text.document(), QStringLiteral("bold")).font().family(), QStringLiteral("Menlo"));
}

void
MarkupSemanticTest::resolvesChineseBoldWithoutChangingRegularOrCodeFonts()
{
    MarkupDocumentModel model;
    model.reconcileSource(QStringLiteral("常规 Regular **粗体 Bold** **`代码 Code`**"), Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    QQuickWindow window;
    NativeText text(payload(&model, 0));
    QVERIFY2(text.object, qPrintable(text.component.errorString()));
    QFont bodyFont = QGuiApplication::font();
    bodyFont.setPointSizeF(14.0);
    QVERIFY(text.object->setProperty("font", bodyFont));
    QVERIFY(text.object->setProperty("renderType", 1));
    text.document()->documentLayout()->documentSize();

    const auto artifactDirectory = qEnvironmentVariable("CRAFTWARD_TEST_ARTIFACT_DIR");
    if (!artifactDirectory.isEmpty()) {
        auto* editor = qobject_cast<QQuickItem*>(text.object.get());
        editor->setParentItem(window.contentItem());
        window.setColor(Qt::white);
        window.resize(480, 100);
        window.show();
        QVERIFY(QTest::qWaitForWindowExposed(&window));
        QVERIFY(window.grabWindow().save(artifactDirectory + QStringLiteral("/native-bold-fallback.png")));
    }

    const auto renderedFont = [&](const QString& token) {
        const int position = text.document()->toPlainText().indexOf(token);
        if (position < 0)
            return QRawFont();
        const auto block = text.document()->findBlock(position);
        const auto runs = block.layout()->glyphRuns(position - block.position(), 1);
        return runs.size() == 1 ? runs.first().rawFont() : QRawFont();
    };
    const auto referenceFont = [](const QString& glyph, const QFont& font) {
        QTextLayout layout(glyph, font);
        layout.beginLayout();
        layout.createLine().setLineWidth(480);
        layout.endLayout();
        const auto runs = layout.glyphRuns();
        return runs.size() == 1 ? runs.first().rawFont() : QRawFont();
    };
    const auto checkFont = [](const QRawFont& actual, const QRawFont& expected, const QString& glyph) {
        QVERIFY(actual.isValid());
        QVERIFY(expected.isValid());
        QCOMPARE(actual.familyName(), expected.familyName());
        QCOMPARE(actual.styleName(), expected.styleName());
        const auto actualGlyphs = actual.glyphIndexesForString(glyph);
        const auto expectedGlyphs = expected.glyphIndexesForString(glyph);
        QCOMPARE(actualGlyphs.size(), 1);
        QCOMPARE(expectedGlyphs.size(), 1);
        QCOMPARE(actual.alphaMapForGlyph(actualGlyphs.first(), QRawFont::PixelAntialiasing),
                 expected.alphaMapForGlyph(expectedGlyphs.first(), QRawFont::PixelAntialiasing));
    };

    checkFont(renderedFont(QStringLiteral("常")), referenceFont(QStringLiteral("常"), bodyFont), QStringLiteral("常"));
    checkFont(
      renderedFont(QStringLiteral("Regular")), referenceFont(QStringLiteral("R"), bodyFont), QStringLiteral("R"));
    QFont boldFont = bodyFont;
    boldFont.setWeight(QFont::Bold);
    checkFont(renderedFont(QStringLiteral("Bold")), referenceFont(QStringLiteral("B"), boldFont), QStringLiteral("B"));
    boldFont.setFamily(QStringLiteral("PingFang SC"));
    checkFont(renderedFont(QStringLiteral("粗")), referenceFont(QStringLiteral("粗"), boldFont), QStringLiteral("粗"));
    QFont codeFont = bodyFont;
    codeFont.setFamilies(text.object->property("codeFont").value<QFont>().families());
    codeFont.setFixedPitch(true);
    codeFont.setWeight(QFont::Bold);
    checkFont(renderedFont(QStringLiteral("代")), referenceFont(QStringLiteral("代"), codeFont), QStringLiteral("代"));
    checkFont(renderedFont(QStringLiteral("Code")), referenceFont(QStringLiteral("C"), codeFont), QStringLiteral("C"));
}

void
MarkupSemanticTest::confinesInlineCodeBackgroundToItsText_data()
{
    QTest::addColumn<QString>("source");
    QTest::addColumn<int>("renderType");
    const auto paragraphs = QStringLiteral("`first`\n\nPlain `second` text");
    const auto list = QStringLiteral("- `first`\n- Plain `second` text");
    QTest::newRow("paragraphs-qt") << paragraphs << 0;
    QTest::newRow("paragraphs-native") << paragraphs << 1;
    QTest::newRow("list-qt") << list << 0;
    QTest::newRow("list-native") << list << 1;
}

void
MarkupSemanticTest::confinesInlineCodeBackgroundToItsText()
{
    QFETCH(QString, source);
    QFETCH(int, renderType);
    MarkupDocumentModel model;
    model.reconcileSource(source, Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    QQuickWindow window;
    window.setColor(Qt::white);
    window.resize(480, 160);
    NativeText text(payload(&model, 0));
    QVERIFY2(text.object, qPrintable(text.component.errorString()));
    auto* editor = qobject_cast<QQuickItem*>(text.object.get());
    QVERIFY(editor);
    QVERIFY(editor->setProperty("renderType", renderType));
    editor->setParentItem(window.contentItem());
    QCOMPARE(text.document()->blockCount(), 2);
    QCOMPARE(text.document()->lastBlock().text(), QStringLiteral("Plain second text"));

    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QCoreApplication::processEvents();
    const QImage rendered = window.grabWindow();
    QVERIFY(!rendered.isNull());
    const auto artifactDirectory = qEnvironmentVariable("CRAFTWARD_TEST_ARTIFACT_DIR");
    if (!artifactDirectory.isEmpty())
        QVERIFY(
          rendered.save(artifactDirectory +
                        QStringLiteral("/inline-background-%1.png").arg(QString::fromLatin1(QTest::currentDataTag()))));

    const auto block = text.document()->lastBlock();
    const auto bounds = text.document()->documentLayout()->blockBoundingRect(block);
    // Sample empty space within the paragraph, beyond the last glyph.
    const auto sample = editor->mapToScene(bounds.center());
    const qreal pixelRatio = rendered.width() / qreal(window.width());
    QCOMPARE(rendered.pixelColor(qRound(sample.x() * pixelRatio), qRound(sample.y() * pixelRatio)), QColor(Qt::white));
    QCOMPARE(formatAt(text.document(), QStringLiteral("Plain")).background().style(), Qt::NoBrush);
    QCOMPARE(formatAt(text.document(), QStringLiteral("text")).background().style(), Qt::NoBrush);
    QCOMPARE(formatAt(text.document(), QStringLiteral("first")).background().style(), Qt::NoBrush);
    QCOMPARE(formatAt(text.document(), QStringLiteral("second")).background().style(), Qt::NoBrush);
    QCOMPARE(visualItems(editor, QStringLiteral("markupCodeBackground")).size(), 2);
}

void
MarkupSemanticTest::decoratesInlineCodeWithoutChangingTextLayout_data()
{
    QTest::addColumn<QString>("source");
    QTest::addColumn<int>("renderType");
    QTest::addColumn<bool>("dark");
    for (const int renderType : { 0, 1 }) {
        const auto suffix = renderType == 0 ? "-qt" : "-native";
        QTest::newRow(qPrintable(QStringLiteral("short") + suffix))
          << QStringLiteral("Before `hello` after.") << renderType << false;
        QTest::newRow(qPrintable(QStringLiteral("wrapped") + suffix))
          << QStringLiteral("`some_very_long_identifier_abcdefghijklmnopqrstuvwxyz_0123456789` tail.") << renderType
          << false;
        QTest::newRow(qPrintable(QStringLiteral("unicode") + suffix))
          << QString::fromUtf8("文字 `你好👩‍💻é` and `مرحبا بالعالم`.") << renderType << false;
        QTest::newRow(qPrintable(QStringLiteral("dark") + suffix))
          << QStringLiteral("Before `hello` after.") << renderType << true;
    }
}

void
MarkupSemanticTest::decoratesInlineCodeWithoutChangingTextLayout()
{
    QFETCH(QString, source);
    QFETCH(int, renderType);
    QFETCH(bool, dark);
    MarkupDocumentModel model;
    model.reconcileSource(source, Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    NativeText decorated(payload(&model, 0));
    NativeText reference(payload(&model, 0));
    QVERIFY2(decorated.object, qPrintable(decorated.component.errorString()));
    QVERIFY2(reference.object, qPrintable(reference.component.errorString()));
    auto* editor = qobject_cast<QQuickItem*>(decorated.object.get());
    auto* plain = qobject_cast<QQuickItem*>(reference.object.get());
    QVERIFY(editor && plain);
    QQuickWindow window;
    const QColor pageColor(dark ? "#18181b" : "#ffffff");
    const QColor backgroundColor(dark ? "#27272a" : "#eeeeee");
    window.setColor(pageColor);
    window.resize(460, 240);
    for (auto* item : { editor, plain }) {
        item->setParentItem(window.contentItem());
        item->setWidth(200);
        item->setY(16);
        QVERIFY(item->setProperty("renderType", renderType));
        QVERIFY(item->setProperty("color", QColor(dark ? "#fafafa" : "#000000")));
        QVERIFY(item->setProperty("codeBackground", backgroundColor));
    }
    editor->setX(16);
    plain->setX(244);
    QVERIFY(plain->setProperty("codeVerticalPadding", 0));
    QVERIFY(plain->setProperty("codeRadius", 0));
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QTRY_VERIFY(!visualItems(editor, QStringLiteral("markupCodeBackground")).isEmpty());
    QTRY_COMPARE(visualItems(editor, QStringLiteral("markupCodeBackground")).size(),
                 visualItems(plain, QStringLiteral("markupCodeBackground")).size());

    const auto verifyLayout = [&] {
        QCOMPARE(decorated.document()->toPlainText(), reference.document()->toPlainText());
        for (auto block = decorated.document()->begin(), other = reference.document()->begin(); block.isValid();
             block = block.next(), other = other.next()) {
            QVERIFY(other.isValid());
            QCOMPARE(block.layout()->lineCount(), other.layout()->lineCount());
            for (int i = 0; i < block.layout()->lineCount(); ++i) {
                const auto line = block.layout()->lineAt(i);
                const auto original = other.layout()->lineAt(i);
                QCOMPARE(line.textStart(), original.textStart());
                QCOMPARE(line.textLength(), original.textLength());
                QCOMPARE(line.height(), original.height());
                QCOMPARE(line.ascent(), original.ascent());
                QCOMPARE(line.glyphRuns(), original.glyphRuns());
            }
        }
        const auto backgrounds = visualItems(editor, QStringLiteral("markupCodeBackground"));
        const auto originals = visualItems(plain, QStringLiteral("markupCodeBackground"));
        QCOMPARE(backgrounds.size(), originals.size());
        for (int i = 0; i < backgrounds.size(); ++i) {
            QCOMPARE(backgrounds.at(i)->x(), originals.at(i)->x());
            QCOMPARE(backgrounds.at(i)->width(), originals.at(i)->width());
            QCOMPARE(backgrounds.at(i)->height(), originals.at(i)->height() + 2);
            QVERIFY(backgrounds.at(i)->y() >= 0);
            QVERIFY(backgrounds.at(i)->y() + backgrounds.at(i)->height() <= editor->height() + 0.01);
            // Top padding shifts the text down; the background extends back up by one pixel.
            QCOMPARE(backgrounds.at(i)->y(), originals.at(i)->y());
        }
    };
    verifyLayout();

    const QImage rendered = window.grabWindow();
    QVERIFY(!rendered.isNull());
    const auto* first = visualItems(editor, QStringLiteral("markupCodeBackground")).first();
    const auto corner = first->mapToScene(QPointF(0.25, 0.25));
    const auto top = first->mapToScene(QPointF(first->width() / 2, 0.5));
    const qreal ratio = rendered.width() / qreal(window.width());
    const auto pixelAt = [&](const QPointF& point) {
        return rendered.pixelColor(qFloor(point.x() * ratio), qFloor(point.y() * ratio));
    };
    QCOMPARE(pixelAt(corner), pageColor);
    QCOMPARE(pixelAt(top), backgroundColor);
    const auto artifactDirectory = qEnvironmentVariable("CRAFTWARD_TEST_ARTIFACT_DIR");
    if (!artifactDirectory.isEmpty())
        QVERIFY(
          rendered.save(artifactDirectory +
                        QStringLiteral("/inline-style-%1.png").arg(QString::fromLatin1(QTest::currentDataTag()))));

    QVERIFY(QMetaObject::invokeMethod(editor, "selectAll"));
    QCOMPARE(editor->property("selectedText").toString(), decorated.document()->toPlainText());
    if (!artifactDirectory.isEmpty())
        QVERIFY(window.grabWindow().save(
          artifactDirectory +
          QStringLiteral("/inline-style-selected-%1.png").arg(QString::fromLatin1(QTest::currentDataTag()))));

    editor->setWidth(120);
    plain->setWidth(120);
    QTest::qWait(30);
    verifyLayout();
    QVERIFY(QMetaObject::invokeMethod(editor, "deselect"));
    const auto replacement = markupLiteralPart(QStringLiteral("plain"), QStringLiteral("Plain replacement"));
    QVERIFY(editor->setProperty("surface", textPayload(replacement)));
    QTRY_VERIFY(visualItems(editor, QStringLiteral("markupCodeBackground")).isEmpty());
    QCOMPARE(editor->property("topPadding").toReal(), qreal(0));
    QCOMPARE(editor->property("bottomPadding").toReal(), qreal(0));
}

void
MarkupSemanticTest::productionSegmentConsumesSemanticPayload()
{
    MarkupDocumentModel document;
    document.reconcileSource(QStringLiteral("Native **text** :codex-annotation{index=\"4\"}"), Format::Markdown);
    auto* model = &document;
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

            renderParts: semanticPayload
        }
    )",
                      QUrl(QStringLiteral("qrc:/ProductionSemanticSegmentTest.qml")));
    const std::unique_ptr<QObject> view(component.create());
    QVERIFY2(view, qPrintable(component.errorString()));
    auto* adapter = nativeAdapter(view.get());
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
    auto* model = &document;
    QTRY_COMPARE(model->rowCount(), 2);
    NativeText list(payload(model, 0));
    QVERIFY2(list.object, qPrintable(list.component.errorString()));
    QVERIFY(list.document()->firstBlock().textList());
    QCOMPARE(list.document()->firstBlock().textList()->format().start(), 7);
    QCOMPARE(list.document()->firstBlock().textList()->count(), 2);
    const auto table = payload(model, 1).toList().first().toMap();
    const auto rows = table.value(QStringLiteral("rows")).toList();
    QCOMPARE(rows.size(), 16);
    QCOMPARE(table.value(QStringLiteral("columns")).toInt(), 2);
    const auto header = rows.first().toMap().value(QStringLiteral("cells")).toList();
    NativeText heading(header.at(1));
    QVERIFY(heading.object);
    QCOMPARE(heading.document()->firstBlock().blockFormat().alignment(), Qt::AlignRight);
    QVERIFY(formatAt(heading.document(), QStringLiteral("B")).fontWeight() >= QFont::Bold);
    const auto cells = rows.at(1).toMap().value(QStringLiteral("cells")).toList();
    NativeText code(cells.first());
    NativeText link(cells.at(1));
    QVERIFY(code.object && link.object);
    QCOMPARE(formatAt(code.document(), QStringLiteral("code")).font().family(), QStringLiteral("Menlo"));
    QCOMPARE(formatAt(link.document(), QStringLiteral("link")).anchorHref(), QStringLiteral("https://example.com"));
    QVERIFY(link.document()->toPlainText().contains(QStringLiteral("[2]")));
    const auto firstBody = payload(model, 1);
    QSignalSpy changed(model, &QAbstractItemModel::dataChanged);
    document.reconcileSource(source + QStringLiteral("| More | cells |\n").repeated(128), Format::Markdown);
    QTRY_COMPARE(model->rowCount(), 10);
    QCOMPARE(payload(model, 1), firstBody);
    QCOMPARE(changed.size(), 0);
}

void
MarkupSemanticTest::distinguishesListSpacing_data()
{
    QTest::addColumn<QString>("source");
    QTest::addColumn<bool>("loose");
    QTest::newRow("tight-bullets") << QStringLiteral("- Alpha\n- Beta") << false;
    QTest::newRow("loose-bullets") << QStringLiteral("- Alpha\n\n- Beta") << true;
    QTest::newRow("tight-numbers") << QStringLiteral("1. Alpha\n2. Beta") << false;
    QTest::newRow("loose-numbers") << QStringLiteral("1. Alpha\n\n2. Beta") << true;
    QTest::newRow("tight-tasks") << QStringLiteral("- [ ] Alpha\n- [x] Beta") << false;
    QTest::newRow("loose-tasks") << QStringLiteral("- [ ] Alpha\n\n- [x] Beta") << true;
}

void
MarkupSemanticTest::distinguishesListSpacing()
{
    QFETCH(QString, source);
    QFETCH(bool, loose);
    MarkupDocumentModel model;
    model.reconcileSource(source, Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    const auto semantic =
      model.data(model.index(0, 0), MarkupDocumentModel::SemanticSegmentRole).value<SemanticDocument>();
    const auto nodes = semantic.blocks().first().nodes();
    const bool hasItemParagraph = std::any_of(nodes.cbegin(), nodes.cend(), [&](const SemanticNode& node) {
        return node.hasContainer() &&
               node.container() == ContainerKindGadget::ContainerKind::CONTAINER_KIND_PARAGRAPH &&
               node.hasParentIndex() && nodes.at(node.parentIndex()).hasContainer() &&
               nodes.at(node.parentIndex()).container() == ContainerKindGadget::ContainerKind::CONTAINER_KIND_LIST_ITEM;
    });
    QCOMPARE(hasItemParagraph, loose);
    NativeText fixture(payload(&model, 0));
    QVERIFY2(fixture.object, qPrintable(fixture.component.errorString()));
    auto* document = fixture.document();
    QCOMPARE(document->blockCount(), 2);
    const auto first = document->firstBlock();
    const auto second = first.next();
    QCOMPARE(first.text(), QStringLiteral("Alpha"));
    QCOMPARE(second.text(), QStringLiteral("Beta"));
    QCOMPARE(first.blockFormat().topMargin(), 0);
    QCOMPARE(second.blockFormat().topMargin(), loose ? 10 : 0);
    if (source.contains(QStringLiteral("[ ]"))) {
        QCOMPARE(first.blockFormat().marker(), QTextBlockFormat::MarkerType::Unchecked);
        QCOMPARE(second.blockFormat().marker(), QTextBlockFormat::MarkerType::Checked);
    }
    model.selection()->selectAll();
    QCOMPARE(model.selection()->text(), QStringLiteral("Alpha\nBeta"));
}

void
MarkupSemanticTest::sizesListIndentationAndNestedMarkers()
{
    MarkupDocumentModel model;
    model.reconcileSource(QStringLiteral("- Alpha\n  - Beta\n    - Gamma\n- Delta"), Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    NativeText fixture(payload(&model, 0));
    QVERIFY2(fixture.object, qPrintable(fixture.component.errorString()));
    auto* document = fixture.document();
    QCOMPARE(document->blockCount(), 4);
    QCOMPARE(document->indentWidth(), 32);
    const auto first = document->firstBlock();
    QCOMPARE(first.textList()->format().style(), QTextListFormat::ListDisc);
    QCOMPARE(first.next().textList()->format().style(), QTextListFormat::ListCircle);
    QCOMPARE(first.next().next().textList()->format().style(), QTextListFormat::ListSquare);
    QCOMPARE(first.next().next().next().textList()->format().style(), QTextListFormat::ListDisc);
    auto font = fixture.object->property("font").value<QFont>();
    font.setPixelSize(24);
    QVERIFY(fixture.object->setProperty("font", font));
    QTRY_COMPARE(document->indentWidth(), 48);
    document->documentLayout()->documentSize();
    QCOMPARE(document->firstBlock().layout()->lineAt(0).x(), 48);
    QCOMPARE(document->firstBlock().next().layout()->lineAt(0).x(), 96);
}

void
MarkupSemanticTest::enlargesDiscMarkersWithoutMovingText()
{
    MarkupDocumentModel model;
    model.reconcileSource(QStringLiteral("- Alpha 中文列表：长文本用于检查换行位置与选区是否稳定。\n"
                                         "  - Beta\n    - Gamma\n  1. Delta\n"
                                         "- [ ] Task\n- [x] Done\n- Omega"),
                          Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    NativeText actual(payload(&model, 0));
    NativeText reference(payload(&model, 0));
    QVERIFY(actual.object && reference.object);
    const auto glyphBounds = [](const QFont& font) {
        QTextLayout layout(QString(QChar(0x2022)), font);
        layout.beginLayout();
        const auto line = layout.createLine();
        layout.endLayout();
        const auto run = layout.glyphRuns().first();
        return run.rawFont().pathForGlyph(run.glyphIndexes().first()).boundingRect().translated(0, line.ascent());
    };
    for (const int size : { 14, 24 }) {
        auto font = QGuiApplication::font();
        font.setPixelSize(size);
        for (const auto* fixture : { &actual, &reference }) {
            QVERIFY(fixture->object->setProperty("width", 260));
            QVERIFY(fixture->object->setProperty("font", font));
        }
        auto* document = actual.document();
        auto* baseline = reference.document();
        for (auto block = baseline->begin(); block.isValid(); block = block.next()) {
            QTextCursor cursor(block);
            cursor.setBlockCharFormat(QTextCharFormat());
        }
        QCOMPARE(document->blockCount(), 7);
        QCOMPARE(document->toPlainText(), baseline->toPlainText());
        QCOMPARE(document->documentLayout()->documentSize(), baseline->documentLayout()->documentSize());
        auto expected = baseline->begin();
        for (auto block = document->begin(); block.isValid(); block = block.next(), expected = expected.next()) {
            const auto markerFont = block.charFormat().font();
            const auto originalFont = expected.charFormat().font();
            const bool disc = block.textList()->format().style() == QTextListFormat::ListDisc &&
                              block.blockFormat().marker() == QTextBlockFormat::MarkerType::NoMarker;
            if (disc) {
                const auto bounds = glyphBounds(markerFont);
                const auto original = glyphBounds(originalFont);
                QVERIFY(bounds.width() >= size * 0.28 && bounds.width() <= size * 0.34);
                QVERIFY(bounds.width() > original.width() * 1.4);
                QVERIFY(qAbs(bounds.center().y() - original.center().y()) < 0.5);
            } else {
                QCOMPARE(markerFont, originalFont);
            }
            QCOMPARE(block.layout()->position(), expected.layout()->position());
            QCOMPARE(block.layout()->lineCount(), expected.layout()->lineCount());
            for (int i = 0; i < block.layout()->lineCount(); ++i) {
                QCOMPARE(block.layout()->lineAt(i).rect(), expected.layout()->lineAt(i).rect());
                QCOMPARE(block.layout()->lineAt(i).ascent(), expected.layout()->lineAt(i).ascent());
            }
            auto originalRun = expected.begin();
            for (auto run = block.begin(); !run.atEnd(); ++run, ++originalRun)
                QCOMPARE(run.fragment().charFormat(), originalRun.fragment().charFormat());
        }
        QVERIFY(QMetaObject::invokeMethod(actual.object.get(), "selectAll"));
        QCOMPARE(actual.object->property("selectedText").toString().replace(QChar::ParagraphSeparator, QChar::LineFeed),
                 document->toPlainText());
    }
    model.selection()->selectAll();
    QCOMPARE(model.selection()->text(), actual.document()->toPlainText());
}

void
MarkupSemanticTest::alignsListMarkersWithInlineText_data()
{
    QTest::addColumn<int>("renderType");
    QTest::newRow("qt") << 0;
    QTest::newRow("native") << 1;
}

void
MarkupSemanticTest::alignsListMarkersWithInlineText()
{
    QFETCH(int, renderType);
    MarkupDocumentModel listModel;
    MarkupDocumentModel inlineModel;
    listModel.reconcileSource(QStringLiteral("1. **排版**：计算文字宽度"), Format::Markdown);
    inlineModel.reconcileSource(QStringLiteral("1\\. **排版**：计算文字宽度"), Format::Markdown);
    QTRY_COMPARE(listModel.rowCount(), 1);
    QTRY_COMPARE(inlineModel.rowCount(), 1);
    QQuickWindow window;
    window.setColor(Qt::white);
    window.resize(600, 180);
    NativeText actual(payload(&listModel, 0));
    NativeText reference(payload(&inlineModel, 0));
    QVERIFY(actual.object && reference.object);
    auto* editor = qobject_cast<QQuickItem*>(actual.object.get());
    auto* inlineEditor = qobject_cast<QQuickItem*>(reference.object.get());
    for (auto* item : { editor, inlineEditor }) {
        item->setParentItem(window.contentItem());
        QVERIFY(item->setProperty("width", 220));
        QVERIFY(item->setProperty("renderType", renderType));
        QVERIFY(item->setProperty("color", QColor(Qt::black)));
    }
    editor->setPosition(QPointF(20, 20));
    inlineEditor->setPosition(QPointF(320, 20));
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    const auto grab = [&] {
        QCoreApplication::processEvents();
        return window.grabWindow();
    };
    const auto inkBounds = [](const QImage& image, const QRect& region) {
        QRect result;
        for (int y = region.top(); y <= region.bottom(); ++y)
            for (int x = region.left(); x <= region.right(); ++x)
                if (image.pixelColor(x, y).value() < 128)
                    result |= QRect(x, y, 1, 1);
        return result;
    };
    for (const int size : { 14, 24 }) {
        auto font = QGuiApplication::font();
        font.setPixelSize(size);
        QVERIFY(editor->setProperty("font", font));
        QVERIFY(inlineEditor->setProperty("font", font));
        const auto image = grab();
        QVERIFY(!image.isNull());
        const qreal dpr = image.width() / qreal(window.width());
        const auto first = actual.document()->firstBlock().layout()->lineAt(0);
        const auto inlineFirst = reference.document()->firstBlock().layout()->lineAt(0);
        const auto pixels = [dpr](const QRectF& rect) {
            return QRectF(rect.topLeft() * dpr, rect.size() * dpr).toAlignedRect();
        };
        const auto gutter = pixels(QRectF(editor->position(), QSizeF(first.x() - 1, first.rect().bottom())));
        const auto inlineNumber =
          pixels(QRectF(inlineEditor->position(), QSizeF(inlineFirst.cursorToX(2), inlineFirst.rect().bottom())));
        const auto markerInk = inkBounds(image, gutter);
        const auto inlineInk = inkBounds(image, inlineNumber);
        QVERIFY(!markerInk.isEmpty() && !inlineInk.isEmpty());
        // Inline numbers already share the fallback glyphs' baseline through Qt's layout.
        QVERIFY(qAbs(markerInk.top() - inlineInk.top()) <= 1);
        QVERIFY(qAbs(markerInk.bottom() - inlineInk.bottom()) <= 1);
        const qreal gap = editor->x() + first.x() - (markerInk.right() + 1) / dpr;
        QVERIFY(gap >= size * 0.65 && gap <= size * 0.9);
        QVERIFY(markerInk.left() >= editor->x() * dpr);

        // Selection must neither move the decoration nor revive the transparent native marker.
        QVERIFY(QMetaObject::invokeMethod(editor, "selectAll"));
        QCOMPARE(grab().copy(gutter), image.copy(gutter));
        QCOMPARE(editor->property("selectedText").toString(), actual.document()->toPlainText());
        QVERIFY(QMetaObject::invokeMethod(editor, "deselect"));
        const auto markers = visualItems(editor, QStringLiteral("markupListMarkers"));
        QCOMPARE(markers.size(), 1);
        markers.first()->setVisible(false);
        QVERIFY(inkBounds(grab(), gutter).isEmpty());
        markers.first()->setVisible(true);
        QCOMPARE(grab().copy(gutter), image.copy(gutter));
    }
}

void
MarkupSemanticTest::preservesListLayoutAcrossSegments_data()
{
    QTest::addColumn<bool>("loose");
    QTest::newRow("tight") << false;
    QTest::newRow("loose") << true;
}

void
MarkupSemanticTest::preservesListLayoutAcrossSegments()
{
    QFETCH(bool, loose);
    const auto sourceFor = [&](int count) {
        QStringList items;
        for (int i = 0; i < count; ++i)
            items.append(QStringLiteral("%1. Item %2").arg(9980 + i).arg(i));
        return items.join(loose ? QStringLiteral("\n\n") : QStringLiteral("\n"));
    };
    MarkupDocumentModel model;
    model.reconcileSource(sourceFor(20), Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 2);
    SelectionScene scene(&model);
    QVERIFY2(scene.view, qPrintable(scene.component.errorString()));
    scene.window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&scene.window));
    auto* editor = scene.editorContaining(QStringLiteral("Item 0"));
    QVERIFY(editor);
    auto* document = editor->property("textDocument").value<QQuickTextDocument*>()->textDocument();
    const auto oldIndent = document->indentWidth();
    model.selection()->selectAll();
    const auto selected = model.selection()->text();
    QTRY_VERIFY(!editor->property("selectedText").toString().isEmpty());
    const auto localSelection = editor->property("selectedText").toString();
    QSignalSpy reconciled(&model, &MarkupDocumentModel::documentReconciled);
    model.reconcileSource(sourceFor(21), Format::Markdown);
    QTRY_COMPARE(reconciled.size(), 1);
    // Updated render parts may rematerialize the selected text surface.
    editor = scene.editorContaining(QStringLiteral("Item 0"));
    QVERIFY(editor);
    document = editor->property("textDocument").value<QQuickTextDocument*>()->textDocument();
    QVERIFY(document->indentWidth() > oldIndent);
    QTRY_COMPARE(editor->property("selectedText").toString(), localSelection);
    QCOMPARE(model.selection()->text(), selected);
    const QFontMetricsF metrics(document->defaultFont());
    QVERIFY(document->indentWidth() >= metrics.horizontalAdvance(QStringLiteral("10000. ")));
    for (int row = 0; row < model.rowCount(); ++row) {
        const auto parts = payload(&model, row).toList();
        QCOMPARE(parts.last().toMap().value(QStringLiteral("spacingAfter"), 8).toInt(),
                 row == 0 ? (loose ? 10 : 0) : 8);
        NativeText fixture(payload(&model, row));
        QVERIFY2(fixture.object, qPrintable(fixture.component.errorString()));
        QCOMPARE(fixture.document()->indentWidth(), document->indentWidth());
        QCOMPARE(fixture.document()->firstBlock().textList()->itemText(fixture.document()->firstBlock()),
                 QStringLiteral("%1.").arg(9980 + row * 16));
    }
}

void
MarkupSemanticTest::preservesListSpacingAcrossEmptySegments_data()
{
    QTest::addColumn<bool>("ordered");
    QTest::addColumn<bool>("loose");
    QTest::newRow("tight-numbers") << true << false;
    QTest::newRow("loose-numbers") << true << true;
    QTest::newRow("tight-bullets") << false << false;
    QTest::newRow("loose-bullets") << false << true;
}

void
MarkupSemanticTest::preservesListSpacingAcrossEmptySegments()
{
    QFETCH(bool, ordered);
    QFETCH(bool, loose);
    QStringList items;
    for (int i = 1; i <= 32; ++i)
        items.append(ordered ? QStringLiteral("%1.").arg(i) : QStringLiteral("-"));
    const auto separator = loose ? QStringLiteral("\n\n") : QStringLiteral("\n");
    MarkupDocumentModel model;
    model.reconcileSource(items.join(separator), Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 2);
    // Appending text reveals the complete list's loose paragraph structure.
    items.append(ordered ? QStringLiteral("33. Content") : QStringLiteral("- Content"));
    model.reconcileSource(items.join(separator), Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 3);
    const qreal spacing = loose ? 10 : 0;
    for (int row = 0; row < model.rowCount(); ++row) {
        const auto parts = payload(&model, row).toList();
        QCOMPARE(parts.last().toMap().value(QStringLiteral("spacingAfter"), 8).toReal(), row < 2 ? spacing : 8);
        NativeText fixture(payload(&model, row));
        QVERIFY2(fixture.object, qPrintable(fixture.component.errorString()));
        const auto* document = fixture.document();
        QCOMPARE(document->blockCount(), row < 2 ? 16 : 1);
        for (auto block = document->begin(); block.isValid(); block = block.next())
            QCOMPARE(block.blockFormat().topMargin(), block == document->firstBlock() ? 0 : spacing);
        if (ordered)
            QCOMPARE(document->firstBlock().textList()->itemText(document->firstBlock()),
                     QStringLiteral("%1.").arg(row * 16 + 1));
    }
    model.selection()->selectAll();
    QCOMPARE(model.selection()->text(), QString(32, QLatin1Char('\n')) + QStringLiteral("Content"));
}

void
MarkupSemanticTest::alignsNestedListTablesWithText()
{
    MarkupDocumentModel model;
    model.reconcileSource(QStringLiteral("12345. Parent\n\n"
                                         "       Continuation paragraph.\n\n"
                                         "       | A | B |\n"
                                         "       | --- | --- |\n"
                                         "       | one | two |\n\n"
                                         "12346. Next item.\n"),
                          Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    QQmlEngine engine;
    engine.rootContext()->setContextProperty(QStringLiteral("semanticPayload"), payload(&model, 0));
    QQmlComponent component(&engine);
    component.setData(R"(
        import QtQuick
        import Craftward.Components
        MarkupSegmentView {
            width: 480
            codeBlock: false
            segmentText: ""
            language: ""
            font.pixelSize: 16
            renderParts: semanticPayload
        }
    )",
                      QUrl(QStringLiteral("qrc:/NestedListTableTest.qml")));
    const std::unique_ptr<QQuickItem> view(qobject_cast<QQuickItem*>(component.create()));
    QVERIFY2(view, qPrintable(component.errorString()));
    const auto editors = visualItems(view.get(), QStringLiteral("markupProseText"));
    auto* paragraph = editors.value(0);
    auto* next = editors.value(editors.size() - 1);
    auto* table = visualItems(view.get(), QStringLiteral("markupTable")).value(0);
    QVERIFY(paragraph && next && table);
    QCOMPARE(next->property("textDocument").value<QQuickTextDocument*>()->textDocument()->toPlainText(),
             QStringLiteral("Next item."));
    auto* document = paragraph->property("textDocument").value<QQuickTextDocument*>()->textDocument();
    for (const int size : { 16, 24 }) {
        auto font = view->property("font").value<QFont>();
        font.setPixelSize(size);
        QVERIFY(view->setProperty("font", font));
        QCoreApplication::processEvents();
        document->documentLayout()->documentSize();
        const qreal indent = document->firstBlock().layout()->lineAt(0).x();
        QVERIFY(indent > size * 2);
        QCOMPARE(document->firstBlock().next().layout()->lineAt(0).x(), indent);
        QTRY_COMPARE(table->mapToItem(view.get(), QPointF()).x(), indent);
        QTRY_COMPARE(next->mapToItem(view.get(), QPointF()).y() -
                       table->mapToItem(view.get(), QPointF(0, table->height())).y(),
                     10);
    }
}

void
MarkupSemanticTest::preservesBlockSpacingBeforeLists_data()
{
    QTest::addColumn<QString>("source");
    QTest::addColumn<QString>("followingText");
    QTest::addColumn<qreal>("gap");
    QTest::newRow("quoted-table-then-tight-list")
      << QStringLiteral("> | A |\n> | --- |\n> | B |\n>\n> - First\n> - Second") << QStringLiteral("First") << qreal(8);
    QTest::newRow("quoted-table-then-loose-list")
      << QStringLiteral("> | A |\n> | --- |\n> | B |\n>\n> - First\n>\n> - Second") << QStringLiteral("First")
      << qreal(8);
    QTest::newRow("table-then-nested-list")
      << QStringLiteral("- Parent\n\n  | A |\n  | --- |\n  | B |\n\n  - Nested\n\n- Next") << QStringLiteral("Nested")
      << qreal(8);
    QTest::newRow("table-then-sibling-item")
      << QStringLiteral("- Parent\n\n  | A |\n  | --- |\n  | B |\n\n- Next") << QStringLiteral("Next") << qreal(10);
}

void
MarkupSemanticTest::preservesBlockSpacingBeforeLists()
{
    QFETCH(QString, source);
    QFETCH(QString, followingText);
    QFETCH(qreal, gap);
    MarkupDocumentModel model;
    model.reconcileSource(source, Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    SelectionScene scene(&model);
    QVERIFY2(scene.view, qPrintable(scene.component.errorString()));
    scene.window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&scene.window));
    auto* following = scene.editorContaining(followingText);
    const auto tables = visualItems(scene.view.get(), QStringLiteral("markupTable"));
    QVERIFY(following);
    QCOMPARE(tables.size(), 1);
    auto* table = tables.first();
    QTRY_COMPARE(following->mapToItem(scene.view.get(), QPointF()).y() -
                   table->mapToItem(scene.view.get(), QPointF(0, table->height())).y(),
                 gap);
}

void
MarkupSemanticTest::placesNestedTablesBelowThePrecedingParagraph()
{
    MarkupDocumentModel document;
    document.reconcileSource(QStringLiteral("> Introduction\n>\n> | A | B |\n> | --- | --- |\n> | one | two |\n"),
                             Format::Markdown);
    auto* model = &document;
    QTRY_COMPARE(model->rowCount(), 1);
    const auto parts = payload(model, 0).toList();
    QCOMPARE(parts.size(), 2);
    NativeText text(parts.first().toMap().value(QStringLiteral("surface")));
    QVERIFY(text.object);
    QCOMPARE(text.document()->firstBlock().text(), QStringLiteral("Introduction"));
    QVERIFY(text.document()->firstBlock().blockFormat().leftMargin() > 0);
    QQmlEngine engine;
    engine.rootContext()->setContextProperty(QStringLiteral("semanticPayload"), payload(model, 0));
    QQmlComponent component(&engine);
    component.setData(R"(
        import QtQuick
        import Craftward.Components
        MarkupSegmentView {
            width: 480
            codeBlock: false
            segmentText: ""
            language: ""
            renderParts: semanticPayload
        }
    )",
                      QUrl(QStringLiteral("qrc:/NestedTableTest.qml")));
    const std::unique_ptr<QQuickItem> view(qobject_cast<QQuickItem*>(component.create()));
    QVERIFY2(view, qPrintable(component.errorString()));
    auto* paragraph = visualItems(view.get(), QStringLiteral("markupProseText")).value(0);
    auto* table = visualItems(view.get(), QStringLiteral("markupTable")).value(0);
    QVERIFY(paragraph && table);
    QTRY_VERIFY(table->mapToItem(view.get(), QPointF()).y() >=
                paragraph->mapToItem(view.get(), QPointF(0, paragraph->height())).y());
    QVERIFY(table->implicitHeight() > 30);
}

void
MarkupSemanticTest::preservesCodeAndUnsupportedSource()
{
    MarkupDocumentModel document;
    document.reconcileSource(
      QStringLiteral("```python\n  print(\"hello\")\n\n```\n\n![alt](image.png)\n\n<div>literal &amp;</div>\n"),
      Format::Markdown);
    auto* model = &document;
    QTRY_COMPARE(model->rowCount(), 3);
    QCOMPARE(model->data(model->index(0, 0), MarkupDocumentModel::CodeBlockRole).toBool(), true);
    QCOMPARE(model->data(model->index(0, 0), MarkupDocumentModel::SegmentTextRole).toString(),
             QStringLiteral("  print(\"hello\")\n"));
    NativeText image(textPayload(payload(model, 1)));
    QCOMPARE(image.document()->toPlainText().trimmed(), QStringLiteral("![alt](image.png)"));
    QCOMPARE(model->data(model->index(1, 0), MarkupDocumentModel::SegmentTextRole).toString().trimmed(),
             QStringLiteral("![alt](image.png)"));
    NativeText literal(textPayload(payload(model, 2)));
    QVERIFY(literal.document()->toPlainText().contains(QStringLiteral("<div>literal &amp;</div>")));
    QVERIFY(model->data(model->index(2, 0), MarkupDocumentModel::SegmentTextRole)
              .toString()
              .contains(QStringLiteral("<div>literal &amp;</div>")));

    document.reconcileSource(QStringLiteral("**plain** :codex-annotation{index=\"4\"}"), Format::PlainText);
    QTRY_COMPARE(model->rowCount(), 1);
    NativeText plain(payload(model, 0));
    QVERIFY(plain.object);
    QCOMPARE(plain.document()->toPlainText(), QStringLiteral("**plain** :codex-annotation{index=\"4\"}"));

    document.reconcileSource(QStringLiteral("```\nunlabelled code\n```"), Format::Markdown);
    QTRY_VERIFY(model->data(model->index(0, 0), MarkupDocumentModel::CodeBlockRole).toBool());
    QCOMPARE(model->data(model->index(0, 0), MarkupDocumentModel::SegmentTextRole).toString(),
             QStringLiteral("unlabelled code"));
    QVERIFY(model->data(model->index(0, 0), MarkupDocumentModel::LanguageRole).toString().isEmpty());
}

void
MarkupSemanticTest::keepsUnchangedTextSelectionAndReleasesDocuments()
{
    MarkupDocumentModel document;
    document.reconcileSource(QStringLiteral("Select **this** text."), Format::Markdown);
    auto* model = &document;
    QTRY_COMPARE(model->rowCount(), 1);
    NativeText text(payload(model, 0));
    QVERIFY(text.object);
    auto* adapter = text.object->findChild<MarkupTextDocument*>();
    QVERIFY(adapter);
    QMetaObject::invokeMethod(text.object.get(), "selectAll");
    const auto selected = text.object->property("selectedText");
    QSignalSpy changed(text.document(), &QTextDocument::contentsChanged);
    adapter->setSurface(textPayload(payload(model, 0)));
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
    auto* model = &document;
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
    auto* model = &document;
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

                    renderParts: semanticPayloads[index]
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
    auto* adapter = nativeAdapter(first);
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
    auto* model = &document;
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
        adapter->setSurface(textPayload(payload(model, 0)));
        NativeText fresh(payload(model, 0));
        QVERIFY(fresh.object);
        QCOMPARE(text.document()->toPlainText(), fresh.document()->toPlainText());
        QCOMPARE(text.object->property("implicitHeight"), fresh.object->property("implicitHeight"));
        QCOMPARE(text.document()->size(), fresh.document()->size());
    }
}

void
MarkupSemanticTest::tableSelectionPreservesGlyphRendering_data()
{
    QTest::addColumn<int>("renderType");
    QTest::newRow("qt") << 0;
    QTest::newRow("native") << 1;
}

void
MarkupSemanticTest::tableSelectionPreservesGlyphRendering()
{
    QFETCH(int, renderType);
    MarkupDocumentModel model;
    model.reconcileSource(QStringLiteral("| First | Second |\n| --- | --- |\n"
                                         "| Alpha glyphs | Beta glyphs |\n"
                                         "| Wrapped text in another row | More text |\n"),
                          Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    QQmlEngine engine;
    engine.rootContext()->setContextProperty(QStringLiteral("semanticPayload"), payload(&model, 0));
    QQmlComponent component(&engine);
    component.setData(R"(
        import QtQuick
        import Craftward.Components
        MarkupSegmentView {
            width: 360
            font { family: "Helvetica Neue"; pixelSize: 16 }
            textColor: "black"
            palette.link: "blue"
            codeBlock: false
            segmentText: ""
            language: ""
            renderParts: semanticPayload
        }
    )",
                      QUrl(QStringLiteral("qrc:/SemanticTableSelectionTest.qml")));
    QQuickWindow window;
    window.setColor(Qt::white);
    window.resize(360, 260);
    const std::unique_ptr<QQuickItem> view(qobject_cast<QQuickItem*>(component.create()));
    QVERIFY2(view, qPrintable(component.errorString()));
    view->setParentItem(window.contentItem());
    const auto editors = visualItems(view.get(), QStringLiteral("markupProseText"));
    QCOMPARE(editors.size(), 6);
    for (auto* editor : editors) {
        QVERIFY(editor->setProperty("renderType", renderType));
        QVERIFY(editor->setProperty("persistentSelection", true));
    }
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    auto grab = [&window] {
        QCoreApplication::processEvents();
        return window.grabWindow().convertToFormat(QImage::Format_ARGB32);
    };
    const QImage baseline = grab();
    QVERIFY(!baseline.isNull());
    const auto artifactDirectory = qEnvironmentVariable("CRAFTWARD_TEST_ARTIFACT_DIR");
    if (!artifactDirectory.isEmpty())
        QVERIFY(baseline.save(artifactDirectory + QStringLiteral("/table-%1.png").arg(renderType)));

    const auto changedPixels = [](const QImage& before, const QImage& after) {
        if (before.size() != after.size())
            return -1;
        int changed = 0;
        for (int y = 0; y < before.height(); ++y) {
            for (int x = 0; x < before.width(); ++x) {
                const auto first = before.pixelColor(x, y);
                const auto second = after.pixelColor(x, y);
                // Allow rounding in glyph shaders, but catch darkened antialiased edges.
                if (qAbs(first.red() - second.red()) > 2 || qAbs(first.green() - second.green()) > 2 ||
                    qAbs(first.blue() - second.blue()) > 2)
                    ++changed;
            }
        }
        return changed;
    };
    const qreal height = view->implicitHeight();
    for (auto* editor : editors) {
        auto* document = editor->property("textDocument").value<QQuickTextDocument*>()->textDocument();
        const auto contents = document->toHtml();
        const auto pointAt = [editor](int position) {
            QRectF rect;
            QMetaObject::invokeMethod(editor, "positionToRectangle", Q_RETURN_ARG(QRectF, rect), Q_ARG(int, position));
            return editor->mapToScene(QPointF(rect.x(), rect.center().y())).toPoint();
        };
        const int start = 1;
        const int end = document->toPlainText().size() - 1;
        QTest::mousePress(&window, Qt::LeftButton, Qt::NoModifier, pointAt(start));
        QTest::mouseMove(&window, pointAt(end));
        QVERIFY(!grab().isNull());
        QTest::mouseMove(&window, pointAt(end - 1));
        const QImage selected = grab();
        const auto selectedText = editor->property("selectedText").toString();
        QVERIFY(!selectedText.isEmpty());
        QTest::mouseRelease(&window, Qt::LeftButton, Qt::NoModifier, pointAt(end - 1));
        QCOMPARE(changedPixels(selected, grab()), 0);
        QCOMPARE(editor->property("selectedText").toString(), selectedText);
        QVERIFY(QMetaObject::invokeMethod(editor, "deselect"));
        QCOMPARE(changedPixels(baseline, grab()), 0);
        QCOMPARE(view->implicitHeight(), height);
        QCOMPARE(document->toHtml(), contents);
    }
}

void
MarkupSemanticTest::discardsObsoleteSnapshots()
{
    MarkupDocumentModel document;
    auto* model = &document;
    document.reconcileSource(QStringLiteral("A paragraph.\n\n").repeated(3000), Format::Markdown);
    QCoreApplication::processEvents();
    document.reconcileSource(QStringLiteral("Newest snapshot"), Format::PlainText);
    QTRY_COMPARE(model->rowCount(), 1);
    QCOMPARE(model->data(model->index(0, 0), MarkupDocumentModel::SegmentTextRole).toString(),
             QStringLiteral("Newest snapshot"));
    document.reconcileSource({}, Format::Markdown);
    QTRY_COMPARE(model->rowCount(), 0);
}

void
MarkupSemanticTest::copiesAcrossSegmentsAndPreservesStreamingSelection()
{
    MarkupDocumentModel model;
    const QString source = QStringLiteral("Before **bold**.\n\n```cpp\n  first();\n\n  second();\n```\n\n"
                                          "| A | B |\n| --- | --- |\n") +
                           QStringLiteral("| `left` | [right](https://example.com) |\n").repeated(33) +
                           QString::fromUtf8("\nAfter 👩‍💻 é عربي.");
    model.reconcileSource(source, Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 6);
    auto* selection = model.selection();
    selection->selectAll();
    const QString expected = QStringLiteral("Before bold.\n  first();\n\n  second();\nA\tB\n") +
                             QStringLiteral("left\tright\n").repeated(33) +
                             QString::fromUtf8("After 👩‍💻 é عربي.");
    QCOMPARE(selection->text(), expected);
    selection->copy();
    QCOMPARE(QGuiApplication::clipboard()->text(), expected);

    const auto first = markupSurfaces(payload(&model, 0).toList()).first();
    const auto last = markupSurfaces(payload(&model, 5).toList()).last();
    MarkupTextDocument start;
    start.setSurface(QVariant::fromValue(first));
    MarkupTextDocument end;
    end.setSurface(QVariant::fromValue(last));
    selection->begin(start.endpointAt(7));
    selection->extend(end.endpointAt(5));
    const auto selected = selection->text();
    QVERIFY(selected.startsWith(QStringLiteral("bold.")));
    QVERIFY(selected.endsWith(QStringLiteral("After")));
    selection->begin(end.endpointAt(5));
    selection->extend(start.endpointAt(7));
    QCOMPARE(selection->text(), selected);

    // A position inside a joined emoji must snap to its grapheme start.
    selection->begin(end.endpointAt(6));
    selection->extend(end.endpointAt(8));
    QVERIFY(!selection->hasSelection());
    selection->extend(end.endpointAt(11));
    QCOMPARE(selection->text(), QString::fromUtf8("👩‍💻"));

    model.reconcileSource(source + QStringLiteral(" Appended text."), Format::Markdown, false);
    QTRY_VERIFY(model.data(model.index(5), MarkupDocumentModel::SegmentTextRole)
                  .toString()
                  .endsWith(QStringLiteral(" Appended text.")));
    QCOMPARE(selection->text(), QString::fromUtf8("👩‍💻"));
    model.reconcileSource(QStringLiteral("abc"), Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    model.selection()->selectAll();
    model.reconcileSource(QStringLiteral("abc**def**"), Format::Markdown);
    QTRY_COMPARE(model.data(model.index(0), MarkupDocumentModel::SegmentTextRole).toString(), QStringLiteral("abcdef"));
    QCOMPARE(model.selection()->text(), QStringLiteral("abc"));
    model.reconcileSource(QStringLiteral("abc"), Format::Markdown);
    QTRY_COMPARE(model.data(model.index(0), MarkupDocumentModel::SegmentTextRole).toString(), QStringLiteral("abc"));
    QCOMPARE(model.selection()->text(), QStringLiteral("abc"));
    model.reconcileSource(QStringLiteral("# Replacement"), Format::Markdown);
    QTRY_COMPARE(model.data(model.index(0), MarkupDocumentModel::SegmentTextRole).toString(),
                 QStringLiteral("Replacement"));
    QVERIFY(!selection->hasSelection());
}

void
MarkupSemanticTest::mouseSelectionSpansProseCodeAndCells()
{
    MarkupDocumentModel model;
    model.reconcileSource(QStringLiteral("Before **bold**.\n\n```cpp\n  code();\n```\n\n"
                                         "| A | B |\n| --- | --- |\n| first cell | last cell |\n\nAfter."),
                          Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 4);
    SelectionScene scene(&model);
    QVERIFY2(scene.view, qPrintable(scene.component.errorString()));
    scene.window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&scene.window));
    QTRY_VERIFY(scene.editorContaining(QStringLiteral("last cell")));
    auto* first = scene.editorContaining(QStringLiteral("Before"));
    auto* last = scene.editorContaining(QStringLiteral("last cell"));
    QVERIFY(first && last);
    QTest::mousePress(&scene.window, Qt::LeftButton, Qt::NoModifier, SelectionScene::pointAt(first, 7));
    QTest::mouseMove(&scene.window, SelectionScene::pointAt(last, 4));
    QTest::mouseRelease(&scene.window, Qt::LeftButton, Qt::NoModifier, SelectionScene::pointAt(last, 4));
    QTRY_COMPARE(model.selection()->text(), QStringLiteral("bold.\n  code();\nA\tB\nfirst cell\tlast"));
    QCOMPARE(last->property("selectedText").toString(), QStringLiteral("last"));
    auto* code = scene.editorContaining(QStringLiteral("code();"));
    QVERIFY(code);
    QCOMPARE(code->property("selectedText").toString(), QStringLiteral("  code();"));
    QTest::keyClick(&scene.window, Qt::Key_C, Qt::ControlModifier);
    QCOMPARE(QGuiApplication::clipboard()->text(), model.selection()->text());
    QTest::mouseMove(&scene.window, SelectionScene::pointAt(code, 3));
    auto* copy = visualItems(scene.view.get(), QStringLiteral("markupCodeCopyButton")).first();
    QTRY_VERIFY(copy->isVisible());
    // Hover adds the button to a Row; render the updated layout before reading its hit position.
    QVERIFY(!scene.window.grabWindow().isNull());
    const auto copyPoint = copy->mapToScene(QPointF(copy->width() / 2, copy->height() / 2)).toPoint();
    QVERIFY(scene.window.contentItem()->contains(copyPoint));
    QTest::mouseClick(&scene.window, Qt::LeftButton, Qt::NoModifier, copyPoint);
    QCOMPARE(QGuiApplication::clipboard()->text(), QStringLiteral("  code();"));
    auto* host = visualItems(scene.view.get(), QStringLiteral("selectionHost")).first();
    auto* pointer = host->findChild<QQuickItem*>(QStringLiteral("markupSelectionPointer"));
    QVERIFY(pointer);
    pointer->forceActiveFocus();
    QTRY_VERIFY(pointer->hasActiveFocus());
    const qreal tableHeight = visualItems(scene.view.get(), QStringLiteral("markupTable")).first()->height();
    scene.view->setWidth(350);
    QTRY_VERIFY(visualItems(scene.view.get(), QStringLiteral("markupTable")).first()->width() <= 350);
    QCOMPARE(model.selection()->text(), QStringLiteral("bold.\n  code();\nA\tB\nfirst cell\tlast"));
    QVERIFY(visualItems(scene.view.get(), QStringLiteral("markupTable")).first()->height() >= tableHeight);
    QTest::keyClick(&scene.window, Qt::Key_Escape);
    QTRY_VERIFY(!model.selection()->hasSelection());
}

void
MarkupSemanticTest::selectionSurvivesVirtualizedDelegateDestruction()
{
    MarkupDocumentModel model;
    model.reconcileSource(QStringLiteral("| A | B |\n| --- | --- |\n") +
                            QStringLiteral("| left text | right text |\n").repeated(400),
                          Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 26);
    SelectionScene scene(&model, true);
    QVERIFY2(scene.view, qPrintable(scene.component.errorString()));
    scene.window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&scene.window));
    QTRY_VERIFY(scene.editorContaining(QStringLiteral("left text")));
    QPointer<QQuickItem> first = scene.editorContaining(QStringLiteral("left text"));
    auto* last = scene.editorContaining(QStringLiteral("right text"));
    QTest::mousePress(&scene.window, Qt::LeftButton, Qt::NoModifier, SelectionScene::pointAt(first, 0));
    QTest::mouseMove(&scene.window, SelectionScene::pointAt(last, 5));
    QTest::mouseRelease(&scene.window, Qt::LeftButton, Qt::NoModifier, SelectionScene::pointAt(last, 5));
    QTRY_COMPARE(model.selection()->text(), QStringLiteral("left text\tright"));
    auto* timeline = visualItems(scene.view.get(), QStringLiteral("selectionTimeline")).first();
    QVERIFY(QMetaObject::invokeMethod(timeline, "positionViewAtIndex", Q_ARG(int, 24), Q_ARG(int, 0)));
    QTRY_VERIFY(first.isNull());
    QCOMPARE(model.selection()->text(), QStringLiteral("left text\tright"));
    QVERIFY(scene.editors().size() < 160);
    QVERIFY(QMetaObject::invokeMethod(timeline, "positionViewAtIndex", Q_ARG(int, 0), Q_ARG(int, 0)));
    QTRY_VERIFY(scene.editorContaining(QStringLiteral("left text")));
    QTRY_COMPARE(scene.editorContaining(QStringLiteral("left text"))->property("selectedText").toString(),
                 QStringLiteral("left text"));
}

void
MarkupSemanticTest::confinesMouseSelectionToOneMessage()
{
    MarkupDocumentModel firstMessage;
    MarkupDocumentModel secondMessage;
    firstMessage.reconcileSource(QStringLiteral("First message."), Format::Markdown);
    secondMessage.reconcileSource(QStringLiteral("Second message."), Format::Markdown);
    QTRY_COMPARE(firstMessage.rowCount(), 1);
    QTRY_COMPARE(secondMessage.rowCount(), 1);
    SelectionScene scene(&firstMessage, false, &secondMessage);
    QVERIFY2(scene.view, qPrintable(scene.component.errorString()));
    scene.window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&scene.window));
    QTRY_VERIFY(scene.editorContaining(QStringLiteral("Second")));
    auto* first = scene.editorContaining(QStringLiteral("First"));
    auto* second = scene.editorContaining(QStringLiteral("Second"));
    QVERIFY(first && second);
    QTest::mousePress(&scene.window, Qt::LeftButton, Qt::NoModifier, SelectionScene::pointAt(first, 6));
    QTest::mouseMove(&scene.window, SelectionScene::pointAt(second, 7));
    QTest::mouseRelease(&scene.window, Qt::LeftButton, Qt::NoModifier, SelectionScene::pointAt(second, 7));
    QCOMPARE(firstMessage.selection()->text(), QStringLiteral("message."));
    QVERIFY(!secondMessage.selection()->hasSelection());
    QTest::mouseClick(&scene.window, Qt::LeftButton, Qt::ShiftModifier, SelectionScene::pointAt(second, 7));
    QCOMPARE(firstMessage.selection()->text(), QStringLiteral("message."));
    QVERIFY(!secondMessage.selection()->hasSelection());
    QTest::mousePress(&scene.window, Qt::LeftButton, Qt::NoModifier, SelectionScene::pointAt(second, 0));
    QTest::mouseMove(&scene.window, SelectionScene::pointAt(second, 6));
    QTest::mouseRelease(&scene.window, Qt::LeftButton, Qt::NoModifier, SelectionScene::pointAt(second, 6));
    QCOMPARE(secondMessage.selection()->text(), QStringLiteral("Second"));
    QVERIFY(!firstMessage.selection()->hasSelection());
    QTest::keyClick(&scene.window, Qt::Key_A, Qt::ControlModifier);
    QCOMPARE(secondMessage.selection()->text(), QStringLiteral("Second message."));
    QVERIFY(!firstMessage.selection()->hasSelection());
}

void
MarkupSemanticTest::copiesEmptyCellsInReadingOrder()
{
    MarkupDocumentModel model;
    model.reconcileSource(QStringLiteral("| A | B | C |\n| --- | --- | --- |\n| | middle | |\n| last | | end |"),
                          Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    model.selection()->selectAll();
    QCOMPARE(model.selection()->text(), QStringLiteral("A\tB\tC\n\tmiddle\t\nlast\t\tend"));
    const auto cells = markupSurfaces(payload(&model, 0).toList());
    QCOMPARE(cells.size(), 9);
    MarkupTextDocument empty;
    empty.setSurface(QVariant::fromValue(cells.at(3)));
    MarkupTextDocument last;
    last.setSurface(QVariant::fromValue(cells.at(8)));
    model.selection()->begin(empty.endpointAt(0));
    model.selection()->extend(last.endpointAt(2));
    QCOMPARE(model.selection()->text(), QStringLiteral("\tmiddle\t\nlast\t\ten"));
}

void
MarkupSemanticTest::selectsTextAfterNormalizedLineBreaks_data()
{
    QTest::addColumn<QString>("source");
    QTest::addColumn<Format>("format");
    QTest::newRow("plain-crlf") << QStringLiteral("👩‍💻 first\r\nmiddle\r\nbravo tail") << Format::PlainText;
    QTest::newRow("literal-html-crlf") << QStringLiteral("<div>\r\n👩‍💻 first\r\nbravo tail\r\n</div>")
                                       << Format::Markdown;
    QTest::newRow("plain-lf") << QStringLiteral("👩‍💻 first\nmiddle\nbravo tail") << Format::PlainText;
    QTest::newRow("plain-cr") << QStringLiteral("👩‍💻 first\rmiddle\rbravo tail") << Format::PlainText;
}

void
MarkupSemanticTest::selectsTextAfterNormalizedLineBreaks()
{
    QFETCH(QString, source);
    QFETCH(Format, format);
    MarkupDocumentModel model;
    model.reconcileSource(source, format);
    QTRY_COMPARE(model.rowCount(), 1);
    SelectionScene scene(&model);
    QVERIFY2(scene.view, qPrintable(scene.component.errorString()));
    scene.window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&scene.window));
    QTRY_VERIFY(scene.editorContaining(QStringLiteral("bravo")));
    auto* editor = scene.editorContaining(QStringLiteral("bravo"));
    auto* document = editor->property("textDocument").value<QQuickTextDocument*>()->textDocument();
    const int position = document->toPlainText().indexOf(QStringLiteral("bravo"));
    for (const bool reverse : { false, true }) {
        const auto start = SelectionScene::pointAt(editor, position + (reverse ? 1 : 0));
        const auto end = SelectionScene::pointAt(editor, position + (reverse ? 0 : 1));
        QTest::mousePress(&scene.window, Qt::LeftButton, Qt::NoModifier, start);
        QTest::mouseMove(&scene.window, end);
        QTest::mouseRelease(&scene.window, Qt::LeftButton, Qt::NoModifier, end);
        QCOMPARE(model.selection()->text(), QStringLiteral("b"));
        QTRY_COMPARE(editor->property("selectedText").toString(), QStringLiteral("b"));
        model.selection()->copy();
        QCOMPARE(QGuiApplication::clipboard()->text(), QStringLiteral("b"));
    }

    QTest::mouseDClick(&scene.window, Qt::LeftButton, Qt::NoModifier, SelectionScene::pointAt(editor, position + 2));
    QTRY_COMPARE(model.selection()->text(), QStringLiteral("bravo"));
    QTRY_COMPARE(editor->property("selectedText").toString(), QStringLiteral("bravo"));
    auto* adapter = editor->findChild<MarkupTextDocument*>();
    QVERIFY(adapter);
    QVERIFY(adapter->setProperty("textColor", QColor(Qt::darkGray)));
    QTRY_COMPARE(editor->property("selectedText").toString(), QStringLiteral("bravo"));
    QCOMPARE(model.selection()->text(), QStringLiteral("bravo"));

    model.selection()->selectAll();
    model.selection()->copy();
    QCOMPARE(QGuiApplication::clipboard()->text(), source);
}

void
MarkupSemanticTest::hydratesCodeAfterDelegateCreation_data()
{
    QTest::addColumn<QString>("language");
    QTest::addColumn<QString>("code");
    QTest::addColumn<bool>("partsFirst");
    const auto shell = QStringLiteral("git submodule sync --recursive\nprintf 'ready\\n'");
    QTest::newRow("bash-text-first") << QStringLiteral("bash") << shell << false;
    QTest::newRow("bash-parts-first") << QStringLiteral("bash") << shell << true;
    QTest::newRow("plain-text") << QStringLiteral("text")
                                << QStringLiteral("subject: preserve <literal> & whitespace\n\n    indented line\n")
                                << false;
    QTest::newRow("unlabelled") << QString() << QStringLiteral("print(\"hello world\")") << false;
}

void
MarkupSemanticTest::hydratesCodeAfterDelegateCreation()
{
    QFETCH(QString, language);
    QFETCH(QString, code);
    QFETCH(bool, partsFirst);
    MarkupDocumentModel model;
    const auto markdown = [&language](const QString& text) {
        return QStringLiteral("```") + language + QLatin1Char('\n') + text + QStringLiteral("\n```");
    };
    model.reconcileSource(markdown(code), Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    QQmlEngine engine;
    QQmlComponent component(&engine);
    component.setData(R"(
        import QtQuick
        import Craftward.Components
        MarkupSegmentView {
            width: 480
            codeBlock: true
            segmentText: ""
            language: ""
            font { family: "Helvetica Neue"; pixelSize: 16 }
            codeFont { family: "Menlo"; pixelSize: 16 }
        }
    )",
                      QUrl(QStringLiteral("qrc:/DeferredCodeHydrationTest.qml")));
    QQuickWindow window;
    window.setColor(Qt::white);
    window.resize(480, 240);
    const std::unique_ptr<QQuickItem> view(qobject_cast<QQuickItem*>(component.create()));
    QVERIFY2(view, qPrintable(component.errorString()));
    view->setParentItem(window.contentItem());
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    // Timeline slots finish creating the row before assigning its sourceRow/dataRevision.
    QVERIFY(view->setProperty("language", language));
    QVERIFY(view->setProperty("selectionCoordinator", QVariant::fromValue(model.selection())));
    if (partsFirst)
        QVERIFY(view->setProperty("renderParts", payload(&model, 0)));
    QVERIFY(view->setProperty("segmentText", code));
    auto* editor = visualItems(view.get(), QStringLiteral("markupCodeText")).value(0);
    QVERIFY(editor);
    auto* document = editor->property("textDocument").value<QQuickTextDocument*>()->textDocument();
    QCOMPARE(document->toPlainText(), code);
    if (!partsFirst)
        QVERIFY(view->setProperty("renderParts", payload(&model, 0)));
    QCoreApplication::processEvents();
    QCOMPARE(document->toPlainText(), code);
    QCOMPARE(document->blockCount(), code.count(QLatin1Char('\n')) + 1);
    if (document->blockCount() > 1)
        QVERIFY(editor->implicitHeight() > QFontMetricsF(editor->property("font").value<QFont>()).height() * 1.5);
    else
        QVERIFY(editor->implicitHeight() > 0);
    const auto artifactDirectory = qEnvironmentVariable("CRAFTWARD_TEST_ARTIFACT_DIR");
    if (!artifactDirectory.isEmpty())
        QVERIFY(window.grabWindow().save(
          artifactDirectory + QStringLiteral("/hydrated-%1.png").arg(QString::fromLatin1(QTest::currentDataTag()))));

    model.selection()->selectAll();
    QTRY_COMPARE(editor->property("selectedText").toString(), code);
    QVERIFY(view->setProperty("textColor", QColor(Qt::darkGray)));
    QTRY_COMPARE(editor->property("selectedText").toString(), code);
    QCOMPARE(document->toPlainText(), code);

    const auto appended = code + QStringLiteral("\n# appended");
    model.reconcileSource(markdown(appended), Format::Markdown, false);
    QTRY_COMPARE(model.data(model.index(0), MarkupDocumentModel::SegmentTextRole).toString(), appended);
    QVERIFY(view->setProperty("segmentText", appended));
    QVERIFY(view->setProperty("renderParts", payload(&model, 0)));
    QTRY_COMPARE(document->toPlainText(), appended);
    QCOMPARE(model.selection()->text(), code);
    QTRY_COMPARE(editor->property("selectedText").toString(), code);
}

void
MarkupSemanticTest::preservesCodeHighlightingDuringSelection_data()
{
    QTest::addColumn<int>("renderType");
    QTest::addColumn<bool>("darkTheme");
    QTest::newRow("qt-light") << 0 << false;
    QTest::newRow("native-light") << 1 << false;
    QTest::newRow("qt-dark") << 0 << true;
    QTest::newRow("native-dark") << 1 << true;
}

void
MarkupSemanticTest::preservesCodeHighlightingDuringSelection()
{
    QFETCH(int, renderType);
    QFETCH(bool, darkTheme);
    const auto originalPalette = QGuiApplication::palette();
    auto palette = originalPalette;
    palette.setColor(QPalette::Window, darkTheme ? QColor("#202020") : Qt::white);
    palette.setColor(QPalette::WindowText, darkTheme ? Qt::white : Qt::black);
    QGuiApplication::setPalette(palette);
    const auto restorePalette = qScopeGuard([&] { QGuiApplication::setPalette(originalPalette); });
    MarkupDocumentModel model;
    const auto code = QStringLiteral("let answer = 42;\nlet message = \"hello\";");
    model.reconcileSource(QStringLiteral("```rust\n") + code + QStringLiteral("\n```"), Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    SelectionScene fixture(&model);
    QVERIFY2(fixture.view, qPrintable(fixture.component.errorString()));
    fixture.window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&fixture.window));
    auto* editor = fixture.editorContaining(code);
    QVERIFY(editor);
    QVERIFY(editor->setProperty("renderType", renderType));
    auto* document = editor->property("textDocument").value<QQuickTextDocument*>()->textDocument();
    const auto formats = [document] {
        QList<QList<QTextLayout::FormatRange>> result;
        for (auto block = document->begin(); block.isValid(); block = block.next())
            result.append(block.layout()->formats());
        return result;
    };
    QTRY_VERIFY(document->firstBlock().layout()->formats().size() > 1);
    const auto baseline = formats();
    QRectF tokenStart;
    QRectF tokenEnd;
    QVERIFY(QMetaObject::invokeMethod(editor, "positionToRectangle", Q_RETURN_ARG(QRectF, tokenStart), Q_ARG(int, 0)));
    QVERIFY(QMetaObject::invokeMethod(editor, "positionToRectangle", Q_RETURN_ARG(QRectF, tokenEnd), Q_ARG(int, 3)));
    const auto tokenRect =
      QRectF(editor->mapToScene(tokenStart.topLeft()), QSizeF(tokenEnd.x() - tokenStart.x(), tokenStart.height()));
    const auto keywordColor = baseline.first().first().format.foreground().color();
    const auto coloredKeywordPixels = [&](const QImage& image) {
        const qreal scale = image.devicePixelRatio();
        const auto region = image.copy(QRectF(tokenRect.topLeft() * scale, tokenRect.size() * scale).toAlignedRect());
        int pixels = 0;
        // At 1x, distance-field glyphs can have coverage blending at every
        // pixel. Require the syntax color with a small per-channel tolerance.
        constexpr int tolerance = 16;
        for (int y = 0; y < region.height(); ++y)
            for (int x = 0; x < region.width(); ++x) {
                const auto pixel = region.pixelColor(x, y);
                pixels += qAbs(pixel.red() - keywordColor.red()) <= tolerance &&
                          qAbs(pixel.green() - keywordColor.green()) <= tolerance &&
                          qAbs(pixel.blue() - keywordColor.blue()) <= tolerance;
            }
        return pixels;
    };
    QVERIFY(coloredKeywordPixels(fixture.window.grabWindow()) > 0);
    const auto stableFrame = [&] {
        if (formats() != baseline)
            return false;
        const auto frame = fixture.window.grabWindow();
        return !frame.isNull() && formats() == baseline;
    };
    const auto artifactDirectory = qEnvironmentVariable("CRAFTWARD_TEST_ARTIFACT_DIR");
    for (int iteration = 0; iteration < 3; ++iteration) {
        QTest::mousePress(&fixture.window, Qt::LeftButton, Qt::NoModifier, SelectionScene::pointAt(editor, 0));
        QVERIFY2(stableFrame(), "Pressing code must retain its syntax formats.");
        for (int position = 1; position < 15; ++position) {
            QTest::mouseMove(&fixture.window, SelectionScene::pointAt(editor, position), 5);
            QVERIFY2(stableFrame(), "Extending selection must retain syntax formats, including unselected lines.");
        }
        QVERIFY(!editor->property("selectedText").toString().isEmpty());
        QVERIFY2(coloredKeywordPixels(fixture.window.grabWindow()) > 0,
                 "The selected keyword must retain its syntax foreground color.");
        if (iteration == 0 && !artifactDirectory.isEmpty())
            QVERIFY(fixture.window.grabWindow().save(
              artifactDirectory + QStringLiteral("/selected-code-%1-%2.png").arg(renderType).arg(darkTheme)));
        QTest::mouseRelease(&fixture.window, Qt::LeftButton, Qt::NoModifier, SelectionScene::pointAt(editor, 14));
        QVERIFY2(stableFrame(), "Releasing selection must retain syntax formats.");
        model.selection()->clear();
        QVERIFY2(stableFrame(), "Clearing selection must retain syntax formats.");
        QCOMPARE(document->toPlainText(), code);
    }

    // Ensure the image check still detects loss of syntax color inside selection.
    model.selection()->selectAll();
    QTRY_COMPARE(editor->property("selectedText").toString(), code);
    QVERIFY(editor->setProperty("selectedTextColor", QColor(Qt::black)));
    QVERIFY2(coloredKeywordPixels(fixture.window.grabWindow()) == 0,
             "A uniform selected foreground must fail the syntax-color check.");
}

void
MarkupSemanticTest::codeSelectionBackgroundMatchesNativeGeometry_data()
{
    QTest::addColumn<QString>("text");
    QTest::addColumn<QString>("family");
    QTest::addColumn<int>("start");
    QTest::addColumn<int>("end");
    QTest::addColumn<bool>("wrap");
    QTest::newRow("partial") << QStringLiteral("let answer = 42;") << QStringLiteral("Menlo") << 2 << 12 << false;
    QTest::newRow("spaces") << QStringLiteral("a   b   c") << QStringLiteral("Menlo") << 2 << 7 << false;
    QTest::newRow("tabs") << QStringLiteral("a\tb\tc") << QStringLiteral("Menlo") << 1 << 4 << false;
    QTest::newRow("bidi") << QString::fromUtf8("abc אבג def") << QStringLiteral("Menlo") << 1 << 6 << false;
    QTest::newRow("rtl") << QString::fromUtf8("مرحبا بالعالم") << QStringLiteral("Menlo") << 2 << 9 << false;
    QTest::newRow("ligature") << QStringLiteral("office") << QStringLiteral("Times New Roman") << 2 << 3 << false;
    QTest::newRow("arabic-ligature") << QString::fromUtf8("سلام") << QStringLiteral("Times New Roman") << 1 << 2
                                     << false;
    QTest::newRow("wrapped-bidi") << QString::fromUtf8("abc אבג long text with spaces and more text")
                                  << QStringLiteral("Menlo") << 2 << 37 << true;
}

void
MarkupSemanticTest::codeSelectionBackgroundMatchesNativeGeometry()
{
    QFETCH(QString, text);
    QFETCH(QString, family);
    QFETCH(int, start);
    QFETCH(int, end);
    QFETCH(bool, wrap);
    QQmlEngine engine;
    engine.rootContext()->setContextProperty(QStringLiteral("fixtureText"), text);
    engine.rootContext()->setContextProperty(QStringLiteral("fixtureFamily"), family);
    engine.rootContext()->setContextProperty(QStringLiteral("fixtureWrap"), wrap);
    QQmlComponent component(&engine);
    component.setData(R"(
        import QtQuick
        import Craftward.Markup
        Item {
            id: fixture
            width: 380
            height: 320
            property bool separateBackground: false
            TextEdit {
                id: text
                objectName: "geometryText"
                x: 12; y: 12
                width: fixtureWrap ? 135 : 350
                text: fixtureText
                font { family: fixtureFamily; pixelSize: 24 }
                textFormat: TextEdit.PlainText
                wrapMode: fixtureWrap ? TextEdit.Wrap : TextEdit.NoWrap
                color: "transparent"
                selectedTextColor: "transparent"
                selectionColor: fixture.separateBackground ? "transparent" : "#bbddff"
                readOnly: true
                persistentSelection: true
                MarkupSelectionBackground {
                    anchors.fill: parent
                    z: -1
                    visible: fixture.separateBackground
                    textEdit: text
                    color: "#bbddff"
                }
            }
        }
    )",
                      QUrl(QStringLiteral("qrc:/SelectionBackgroundGeometry.qml")));
    QQuickWindow window;
    window.resize(380, 320);
    window.setColor(Qt::white);
    const std::unique_ptr<QQuickItem> fixture(qobject_cast<QQuickItem*>(component.create()));
    QVERIFY2(fixture, qPrintable(component.errorString()));
    fixture->setParentItem(window.contentItem());
    auto* editor = visualItems(fixture.get(), QStringLiteral("geometryText")).value(0);
    QVERIFY(editor);
    QVERIFY(QMetaObject::invokeMethod(editor, "select", Q_ARG(int, start), Q_ARG(int, end)));
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    const auto reference = window.grabWindow();
    QVERIFY(fixture->setProperty("separateBackground", true));
    QCoreApplication::processEvents();
    const auto actual = window.grabWindow();
    const auto artifactDirectory = qEnvironmentVariable("CRAFTWARD_TEST_ARTIFACT_DIR");
    if (!artifactDirectory.isEmpty()) {
        const auto name = QString::fromLatin1(QTest::currentDataTag());
        QVERIFY(reference.save(artifactDirectory + QStringLiteral("/geometry-%1-native.png").arg(name)));
        QVERIFY(actual.save(artifactDirectory + QStringLiteral("/geometry-%1-custom.png").arg(name)));
    }
    QCOMPARE(actual.size(), reference.size());
    int difference = 0;
    for (int y = 0; y < actual.height(); ++y)
        for (int x = 0; x < actual.width(); ++x)
            difference += actual.pixelColor(x, y) != reference.pixelColor(x, y);
    QCOMPARE(difference, 0);
    QCOMPARE(editor->property("selectedText").toString(), text.mid(start, end - start));
}

void
MarkupSemanticTest::codeSelectionBackgroundFollowsLayoutAndScrolling()
{
    const auto code = QStringLiteral("let long_line = \"") + QStringLiteral("abcdef ").repeated(20) +
                      QStringLiteral("\";\n\nlet last = 42;");
    MarkupDocumentModel model;
    model.reconcileSource(QStringLiteral("```rust\n") + code + QStringLiteral("\n```"), Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    SelectionScene scene(&model);
    QVERIFY2(scene.view, qPrintable(scene.component.errorString()));
    scene.window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&scene.window));
    auto* editor = scene.editorContaining(code);
    QVERIFY(editor);
    auto* background = visualItems(editor, QStringLiteral("markupSelectionBackground")).value(0);
    QVERIFY(background);
    const auto selectionColor = background->property("color").value<QColor>();
    auto* flick = editor->parentItem();
    while (flick && flick->metaObject()->indexOfProperty("contentX") < 0)
        flick = flick->parentItem();
    QVERIFY(flick);
    const int blankLine = code.indexOf(QLatin1Char('\n')) + 1;
    const auto blankPoint = [&] {
        QRectF cursor;
        QMetaObject::invokeMethod(editor, "positionToRectangle", Q_RETURN_ARG(QRectF, cursor), Q_ARG(int, blankLine));
        const auto font = editor->property("font").value<QFont>();
        return editor->mapToScene(
          QPointF(cursor.x() + QFontMetricsF(font).horizontalAdvance(QLatin1Char(' ')) / 2, cursor.center().y()));
    };
    const auto colorAt = [&](const QPointF& point) {
        const auto image = scene.window.grabWindow();
        return image.pixelColor((point * image.devicePixelRatio()).toPoint());
    };
    model.selection()->selectAll();
    QTRY_COMPARE(editor->property("selectedText").toString(), code);
    QCOMPARE(colorAt(blankPoint()), selectionColor);

    auto larger = editor->property("font").value<QFont>();
    larger.setPixelSize(24);
    QVERIFY(editor->setProperty("font", larger));
    QTRY_COMPARE(editor->property("selectedText").toString(), code);
    QCOMPARE(colorAt(blankPoint()), selectionColor);

    QVERIFY(flick->property("contentWidth").toReal() > flick->width());
    QVERIFY(flick->setProperty("contentX", 90.0));
    QRectF firstLine;
    QVERIFY(QMetaObject::invokeMethod(editor, "positionToRectangle", Q_RETURN_ARG(QRectF, firstLine), Q_ARG(int, 0)));
    // Added leading can precede the first line, so sample within its native bounds.
    const qreal selectionY = editor->mapToItem(flick, firstLine.topLeft()).y() + 1;
    const auto inside = flick->mapToScene(QPointF(flick->width() / 2, selectionY));
    QCOMPARE(colorAt(inside), selectionColor);
    const auto outside = flick->mapToScene(QPointF(-2, selectionY));
    QVERIFY(colorAt(outside) != selectionColor);
    QCOMPARE(editor->property("selectedText").toString(), code);

    QVERIFY(flick->setProperty("contentX", 0.0));
    const auto appended = code + QStringLiteral("\nlet appended = true;");
    model.reconcileSource(QStringLiteral("```rust\n") + appended + QStringLiteral("\n```"), Format::Markdown, false);
    QTRY_COMPARE(editor->property("textDocument").value<QQuickTextDocument*>()->textDocument()->toPlainText(),
                 appended);
    QTRY_COMPARE(editor->property("selectedText").toString(), code);
    QCOMPARE(colorAt(blankPoint()), selectionColor);
    QCOMPARE(model.selection()->text(), code);
    model.selection()->clear();
    QTRY_VERIFY(editor->property("selectedText").toString().isEmpty());
    QVERIFY(colorAt(blankPoint()) != selectionColor);
}

void
MarkupSemanticTest::joinsSelectionWithinParagraphs_data()
{
    QTest::addColumn<int>("renderType");
    QTest::addColumn<bool>("hardBreak");
    QTest::newRow("native-wrap") << 1 << false;
    QTest::newRow("native-hard-break") << 1 << true;
    QTest::newRow("qt-wrap") << 0 << false;
    QTest::newRow("qt-hard-break") << 0 << true;
}

void
MarkupSemanticTest::joinsSelectionWithinParagraphs()
{
    QFETCH(int, renderType);
    QFETCH(bool, hardBreak);
    MarkupDocumentModel model;
    const auto paragraph =
      hardBreak ? QStringLiteral("> Alpha **粗体 Bold** `code`  \n> Beta [link](https://example.com) gamma.")
                : QStringLiteral("> Alpha **粗体 Bold** `code` beta [link](https://example.com) gamma delta "
                                 "epsilon zeta eta theta iota kappa lambda mu nu xi omicron pi rho sigma.");
    model.reconcileSource(paragraph + QStringLiteral("\n>\n> Separate paragraph."), Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    NativeText fixture(payload(&model, 0));
    QVERIFY2(fixture.object, qPrintable(fixture.component.errorString()));
    auto* editor = qobject_cast<QQuickItem*>(fixture.object.get());
    QVERIFY(editor);
    QVERIFY(editor->setProperty("renderType", renderType));
    editor->setWidth(280);
    editor->setPosition(QPointF(12, 12));
    QQuickWindow window;
    window.setColor(Qt::white);
    window.resize(320, 450);
    editor->setParentItem(window.contentItem());
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    auto* document = fixture.document();
    QCOMPARE(document->blockCount(), 2);

    for (const qreal scale : { qreal(1.6), qreal(2.2) }) {
        QVERIFY(editor->setProperty("lineHeightScale", scale));
        QVERIFY(QMetaObject::invokeMethod(editor, "selectAll"));
        QCoreApplication::processEvents();
        window.resize(320, qCeil(editor->implicitHeight()) + 24);
        const auto selected = editor->property("selectedText").toString();
        const auto selectionColor = editor->property("selectionColor").value<QColor>();
        auto* background = visualItems(editor, QStringLiteral("markupSelectionBackground")).value(0);
        if (background)
            background->setVisible(false);
        const auto reference = window.grabWindow();
        if (background)
            background->setVisible(true);
        const auto actual = window.grabWindow();
        const qreal dpr = actual.devicePixelRatio();
        const auto artifacts = qEnvironmentVariable("CRAFTWARD_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty()) {
            const auto name = QStringLiteral("/paragraph-%1-%2").arg(QTest::currentDataTag()).arg(scale);
            QVERIFY(reference.save(artifacts + name + QStringLiteral("-before.png")));
            QVERIFY(actual.save(artifacts + name + QStringLiteral("-after.png")));
        }

        QList<QRectF> gaps;
        const auto* layout = document->firstBlock().layout();
        QVERIFY(layout->lineCount() >= 2);
        QRectF cursor;
        QVERIFY(QMetaObject::invokeMethod(editor, "positionToRectangle", Q_RETURN_ARG(QRectF, cursor), Q_ARG(int, 0)));
        const auto origin = editor->mapToScene(cursor.topLeft()) - layout->position() -
                            QPointF(layout->lineAt(0).cursorToX(0), layout->lineAt(0).y());
        for (int i = 1; i < layout->lineCount(); ++i) {
            const auto previous = layout->lineAt(i - 1);
            const auto line = layout->lineAt(i);
            const qreal top = previous.y() + previous.height();
            const qreal bottom = line.y();
            QVERIFY(bottom > top);
            const QRectF gap(origin + layout->position() + QPointF(0, top), QSizeF(editor->width(), bottom - top));
            gaps.append(gap);
            const auto shared = previous.naturalTextRect().intersected(
              QRectF(line.naturalTextRect().x(), previous.y(), line.naturalTextWidth(), previous.height()));
            QVERIFY(shared.width() > 4);
            const int x = qRound((origin.x() + layout->position().x() + shared.center().x()) * dpr);
            for (int y = qCeil(gap.top() * dpr); y < qFloor(gap.bottom() * dpr); ++y)
                QCOMPARE(actual.pixelColor(x, y), selectionColor);
        }
        QList<QRectF> additions = gaps;
        const auto blockText = document->firstBlock().text();
        const qreal space = QFontMetricsF(document->defaultFont()).horizontalAdvance(QLatin1Char(' '));
        for (int i = 0; i < layout->lineCount(); ++i) {
            const auto line = layout->lineAt(i);
            const int last = line.textStart() + line.textLength() - 1;
            const bool hardBreak = last >= 0 && blockText.at(last) == QChar::LineSeparator;
            if (hardBreak || i + 1 == layout->lineCount()) {
                const int position = hardBreak ? last : blockText.size();
                additions.append(QRectF(origin + layout->position() + QPointF(line.cursorToX(position), line.y()),
                                        QSizeF(space, line.height())));
            }
        }
        // Leading and break markers must not change glyphs, native colors, or paragraph spacing.
        for (int y = 0; y < actual.height(); ++y)
            for (int x = 0; x < actual.width(); ++x) {
                const auto point = QPointF(x + 0.5, y + 0.5) / dpr;
                const bool inAddition = std::any_of(additions.cbegin(), additions.cend(), [&](const QRectF& rectangle) {
                    return rectangle.contains(point);
                });
                if (!inAddition)
                    QCOMPARE(actual.pixelColor(x, y), reference.pixelColor(x, y));
            }
        QCOMPARE(editor->property("selectedText").toString(), selected);

        if (scale > 2) {
            const auto first = layout->lineAt(0);
            const auto second = layout->lineAt(1);
            const int start = first.textStart() + 6;
            const int end = second.textStart() + second.textLength() / 2;
            QVERIFY(QMetaObject::invokeMethod(editor, "select", Q_ARG(int, start), Q_ARG(int, end)));
            const auto partial = window.grabWindow();
            const auto colorAt = [&](qreal x, qreal fraction) {
                return partial.pixelColor((QPointF(origin.x() + layout->position().x() + x,
                                                   gaps.first().top() + gaps.first().height() * fraction) *
                                           dpr)
                                            .toPoint());
            };
            // The first and last lines retain their partial horizontal ranges.
            QCOMPARE(colorAt((first.cursorToX(start - 1) + first.cursorToX(start)) / 2, 0.25), Qt::white);
            QCOMPARE(colorAt((first.cursorToX(start) + first.naturalTextRect().right()) / 2, 0.25), selectionColor);
            QCOMPARE(colorAt((second.naturalTextRect().left() + second.cursorToX(end)) / 2, 0.75), selectionColor);
            QCOMPARE(colorAt((second.cursorToX(end) + second.naturalTextRect().right()) / 2, 0.75), Qt::white);
            for (int i = 1; i < gaps.size(); ++i)
                QCOMPARE(partial.pixelColor((gaps.at(i).center() * dpr).toPoint()), Qt::white);
        }

        QVERIFY(QMetaObject::invokeMethod(editor, "select", Q_ARG(int, 0), Q_ARG(int, 5)));
        const auto singleLine = window.grabWindow();
        for (const auto& gap : gaps)
            QCOMPARE(singleLine.pixelColor((gap.center() * dpr).toPoint()), Qt::white);
        QVERIFY(QMetaObject::invokeMethod(editor, "deselect"));
        const auto cleared = window.grabWindow();
        for (const auto& gap : gaps)
            QCOMPARE(cleared.pixelColor((gap.center() * dpr).toPoint()), Qt::white);
    }
}

void
MarkupSemanticTest::selectsEmptyLinesWithContinuousBackground_data()
{
    QTest::addColumn<int>("renderType");
    QTest::addColumn<bool>("codeBlock");
    QTest::newRow("native-prose") << 1 << false;
    QTest::newRow("qt-prose") << 0 << false;
    QTest::newRow("native-code") << 1 << true;
    QTest::newRow("qt-code") << 0 << true;
}

void
MarkupSemanticTest::selectsEmptyLinesWithContinuousBackground()
{
    QFETCH(int, renderType);
    QFETCH(bool, codeBlock);
    MarkupDocumentModel model;
    model.reconcileSource(codeBlock ? QStringLiteral("```text\nAlpha\n\nBeta\n```")
                                    : QStringLiteral("Alpha\\\n\\\nBeta"),
                          Format::Markdown);
    QTRY_COMPARE(model.rowCount(), 1);
    SelectionScene scene(&model);
    QVERIFY2(scene.view, qPrintable(scene.component.errorString()));
    scene.window.resize(520, 180);
    scene.window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&scene.window));
    auto* editor = scene.editorContaining(QStringLiteral("Alpha"));
    QVERIFY(editor);
    QVERIFY(editor->setProperty("renderType", renderType));
    auto* document = editor->property("textDocument").value<QQuickTextDocument*>()->textDocument();

    for (const qreal scale : { codeBlock ? qreal(1.45) : qreal(1.6), qreal(2.2) }) {
        QCOMPARE(document->blockCount(), codeBlock ? 3 : 1);
        QVERIFY(editor->setProperty("lineHeightScale", scale));
        model.selection()->selectAll();
        QTRY_COMPARE(editor->property("selectedText").toString().replace(QChar::LineSeparator, QLatin1Char('\n')),
                     QStringLiteral("Alpha\n\nBeta"));
        auto* background = visualItems(editor, QStringLiteral("markupSelectionBackground")).value(0);
        QVERIFY(background);
        const auto selectionColor = background->property("color").value<QColor>();
        const qreal space = QFontMetricsF(document->defaultFont()).horizontalAdvance(QLatin1Char(' '));
        QList<QRectF> lines;
        for (const int position : { 0, 6, 7 }) {
            QRectF cursor;
            QVERIFY(QMetaObject::invokeMethod(
              editor, "positionToRectangle", Q_RETURN_ARG(QRectF, cursor), Q_ARG(int, position)));
            lines.append(editor->mapRectToScene(cursor));
        }
        const auto image = scene.window.grabWindow();
        const auto colorAt = [&](const QPointF& point) {
            return image.pixelColor((point * image.devicePixelRatio()).toPoint());
        };
        const auto artifacts = qEnvironmentVariable("CRAFTWARD_TEST_ARTIFACT_DIR");
        if (!artifacts.isEmpty())
            QVERIFY(
              image.save(artifacts + QStringLiteral("/empty-line-%1-%2.png").arg(QTest::currentDataTag()).arg(scale)));
        // A selected empty line has one space of feedback, not an entire row.
        const auto marker = QPointF(lines.at(1).left() + space / 2, lines.at(1).center().y());
        QCOMPARE(colorAt(marker), selectionColor);
        QVERIFY(colorAt(marker + QPointF(space * 2, 0)) != selectionColor);
        // Both sides of the empty line join at the production line-height scale.
        for (int i = 1; i < lines.size(); ++i) {
            const qreal top = lines.at(i - 1).bottom();
            const qreal bottom = lines.at(i).top();
            QVERIFY(bottom > top);
            const qreal x = lines.at(i).left() + space / 2;
            for (const qreal fraction : { qreal(0.25), qreal(0.75) })
                QCOMPARE(colorAt(QPointF(x, top + (bottom - top) * fraction)), selectionColor);
        }
        QCOMPARE(model.selection()->text(), QStringLiteral("Alpha\n\nBeta"));

        // Selecting only the line break still gives visible feedback.
        QVERIFY(QMetaObject::invokeMethod(editor, "select", Q_ARG(int, 6), Q_ARG(int, 7)));
        auto partial = scene.window.grabWindow();
        QCOMPARE(partial.pixelColor((marker * partial.devicePixelRatio()).toPoint()), selectionColor);
        QVERIFY(QMetaObject::invokeMethod(editor, "select", Q_ARG(int, 0), Q_ARG(int, 6)));
        partial = scene.window.grabWindow();
        QVERIFY(partial.pixelColor((marker * partial.devicePixelRatio()).toPoint()) != selectionColor);
        model.selection()->clear();
        QTRY_VERIFY(editor->property("selectedText").toString().isEmpty());
        const auto cleared = scene.window.grabWindow();
        QVERIFY(cleared.pixelColor((marker * cleared.devicePixelRatio()).toPoint()) != selectionColor);
    }
}

void
MarkupSemanticTest::preservesSelectedCodeDecorations_data()
{
    QTest::addColumn<int>("renderType");
    QTest::addColumn<bool>("partial");
    QTest::newRow("qt-full") << 0 << false;
    QTest::newRow("native-full") << 1 << false;
    QTest::newRow("qt-partial") << 0 << true;
    QTest::newRow("native-partial") << 1 << true;
}

void
MarkupSemanticTest::preservesSelectedCodeDecorations()
{
    QFETCH(int, renderType);
    QFETCH(bool, partial);
    QQmlEngine engine;
    QQmlComponent component(&engine);
    component.setData(R"(
        import QtQuick
        import Craftward.Markup
        TextEdit {
            id: editor
            width: 400; height: 100
            text: "let alpha = beta;"
            color: "#880088"
            selectedTextColor: "transparent"
            selectionColor: "transparent"
            font { family: "Menlo"; pixelSize: 24 }
            textFormat: TextEdit.PlainText
            readOnly: true
            persistentSelection: true
            MarkupSelectionBackground {
                anchors.fill: parent
                z: -1
                textEdit: editor
                color: "transparent"
            }
        }
    )",
                      QUrl(QStringLiteral("qrc:/SelectedCodeDecorations.qml")));
    const std::unique_ptr<QQuickItem> editor(qobject_cast<QQuickItem*>(component.create()));
    QVERIFY2(editor, qPrintable(component.errorString()));
    QVERIFY(editor->setProperty("renderType", renderType));
    auto* document = editor->property("textDocument").value<QQuickTextDocument*>()->textDocument();
    QTextCharFormat keyword;
    keyword.setForeground(QColor("#006600"));
    keyword.setFontUnderline(true);
    keyword.setFontWeight(QFont::Bold);
    QTextCharFormat name;
    name.setForeground(QColor("#cc4400"));
    name.setFontUnderline(true);
    name.setFontItalic(true);
    document->firstBlock().layout()->setFormats({ { 0, 3, keyword }, { 4, 5, name } });
    document->markContentsDirty(0, document->characterCount());
    QQuickWindow window;
    window.resize(400, 100);
    window.setColor(Qt::white);
    editor->setParentItem(window.contentItem());
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    const auto reference = window.grabWindow();
    if (partial)
        QVERIFY(QMetaObject::invokeMethod(editor.get(), "select", Q_ARG(int, 1), Q_ARG(int, 7)));
    else
        QVERIFY(QMetaObject::invokeMethod(editor.get(), "selectAll"));
    QCoreApplication::processEvents();
    const auto selected = window.grabWindow();
    const auto artifactDirectory = qEnvironmentVariable("CRAFTWARD_TEST_ARTIFACT_DIR");
    if (!artifactDirectory.isEmpty()) {
        const auto name = QString::fromLatin1(QTest::currentDataTag());
        QVERIFY(reference.save(artifactDirectory + QStringLiteral("/decorations-%1-reference.png").arg(name)));
        QVERIFY(selected.save(artifactDirectory + QStringLiteral("/decorations-%1-selected.png").arg(name)));
    }
    QCOMPARE(selected.size(), reference.size());
    int maximumChannelDifference = 0;
    for (int y = 0; y < selected.height(); ++y)
        for (int x = 0; x < selected.width(); ++x) {
            const auto actual = selected.pixelColor(x, y);
            const auto expected = reference.pixelColor(x, y);
            maximumChannelDifference = std::max({ maximumChannelDifference,
                                                  qAbs(actual.red() - expected.red()),
                                                  qAbs(actual.green() - expected.green()),
                                                  qAbs(actual.blue() - expected.blue()),
                                                  qAbs(actual.alpha() - expected.alpha()) });
        }
    // Layer compositing can round one color step differently at glyph/underline intersections.
    QVERIFY2(maximumChannelDifference <= 1, qPrintable(QString::number(maximumChannelDifference)));
}

QTEST_MAIN(MarkupSemanticTest)

#include "markupsemantictest.moc"
