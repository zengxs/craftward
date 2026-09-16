// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "Scintilla.h"
#include "highlighting/syntaxhighlightingdocument.h"
#include "highlighting/syntaxhighlightingengine.h"
#include "scintillaeditorbackend.h"

#include <QClipboard>
#include <QDragEnterEvent>
#include <QDropEvent>
#include <QFile>
#include <QGuiApplication>
#include <QInputMethod>
#include <QInputMethodEvent>
#include <QKeyEvent>
#include <QMimeData>
#include <QPainter>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQmlExtensionPlugin>
#include <QQuickWindow>
#include <QScopeGuard>
#include <QScopedValueRollback>
#include <QSignalSpy>
#include <QTest>
#include <QTextCharFormat>
#include <QWheelEvent>
#include <QtGui/private/qinputmethod_p.h>
#include <QtMath>

#include <functional>
#include <utility>

Q_IMPORT_QML_PLUGIN(Craftward_EditorPlugin)

class ScintillaQuickTest : public QObject
{
    Q_OBJECT
  private slots:
    void initTestCase();
    void syntaxHighlightingLifecycle();
    void syntaxHighlightingDuringComposition();
    void syntaxHighlightingAfterRapidReplacement();
    void syntaxHighlightingRendersAndReplacesDocuments();
    void syntaxHighlightingRejectsSharedDocuments();
    void syntaxHighlightingTransfersDetachedDocuments_data();
    void syntaxHighlightingTransfersDetachedDocuments();
    void documentSelfAttachmentIsNoOp();
    void syntaxHighlightingRecoversAfterAnObsoleteError();
    void syntaxHighlightingRecoversWithPendingConfiguration_data();
    void syntaxHighlightingRecoversWithPendingConfiguration();
    void selectionPreservesSyntaxColors_data();
    void selectionPreservesSyntaxColors();
    void selectionForegroundOverrideCanBeReset();
    void initialTextWaitsForVisibleStyles_data();
    void initialTextWaitsForVisibleStyles();
    void initialPresentationRecoversAfterReplacementAndFailure();
    void textAndReadOnly();
    void readOnlyPreservesViewport();
    void editingAndUndo();
    void multipleSelections();
    void imeUtf16AndUndo();
    void imeInsertionOffset_data();
    void imeInsertionOffset();
    void imeCommitWithNextPreedit();
    void cancelledCompositionRestoresSelection();
    void imeCommitReplacesSelection_data();
    void imeCommitReplacesSelection();
    void imePreeditFormats_data();
    void imePreeditFormats();
    void focusAfterVisibility_data();
    void focusAfterVisibility();
    void completionUsesQuickItems();
    void completionScrolling_data();
    void completionScrolling();
    void textDropAndReadOnly();
    void textDropDuringComposition_data();
    void textDropDuringComposition();
    void wrappingAndHorizontalScroll();
    void horizontalScrollInputs_data();
    void horizontalScrollInputs();
    void horizontalScrollRangeChanges();
    void horizontalRangeAfterContentShrinks_data();
    void horizontalRangeAfterContentShrinks();
    void horizontalExtentIncludesOffscreenLines();
    void cargoLockScrollRange();
    void editorScrollBars_data();
    void editorScrollBars();
    void horizontalScrollCaretExpansion_data();
    void horizontalScrollCaretExpansion();
    void horizontalRangeWithVirtualCaret_data();
    void horizontalRangeWithVirtualCaret();
    void horizontalRangeWithVirtualSelections();
    void horizontalRangeWithOffscreenVirtualSelections_data();
    void horizontalRangeWithOffscreenVirtualSelections();
    void lineNumberWidth_data();
    void lineNumberWidth();
    void lineNumberFontAndZoom();
    void lineNumbersDuringComposition();
    void lineNumbersAndWrappedSelection();
    void lineNumbersInViews_data();
    void lineNumbersInViews();
    void rendersInsideQuickScene();
    void publicQmlControl();
    void partialPaintAndScrollReuse_data();
    void partialPaintAndScrollReuse();
};

namespace {
void
key(ScintillaEditorBackend& editor, int code, const QString& text = {}, Qt::KeyboardModifiers modifiers = {})
{
    QKeyEvent event(QEvent::KeyPress, code, modifiers, text);
    QCoreApplication::sendEvent(&editor, &event);
}
void
compose(ScintillaEditorBackend& editor,
        const QString& preedit,
        const QString& commit = {},
        int start = 0,
        int length = 0)
{
    QInputMethodEvent event(preedit, {});
    event.setCommitString(commit, start, length);
    QCoreApplication::sendEvent(&editor, &event);
}
class PaintedEditor : public ScintillaEditorBackend
{
  public:
    using ScintillaEditorBackend::ScintillaEditorBackend;
    QList<QRect> painted;
    QImage renderImage()
    {
        QImage image(QSize(qCeil(width()), qCeil(height())), QImage::Format_ARGB32_Premultiplied);
        image.fill(Qt::transparent);
        QPainter painter(&image);
        painter.setRenderHint(QPainter::TextAntialiasing);
        if (!paintImage(painter, image.rect()))
            paintImage(painter, image.rect());
        return image;
    }
    bool paintImage(QPainter& painter, const QRect& rect) override
    {
        painted.append(rect);
        return ScintillaEditorBackend::paintImage(painter, rect);
    }
};
class TestInputContext : public QPlatformInputContext
{
  public:
    std::function<void()> onCommit;
    std::function<void()> onReset;
    void commit() override { onCommit(); }
    void reset() override { onReset(); }
};
class BeforeQueuedCall : public QObject
{
  public:
    std::function<void()> callback;

  protected:
    bool eventFilter(QObject*, QEvent* event) override
    {
        if (event->type() == QEvent::MetaCall && callback)
            std::exchange(callback, {})();
        return false;
    }
};
}

void
ScintillaQuickTest::initTestCase()
{
    qmlRegisterType<ScintillaEditorBackend>("Craftward.TestEditor", 1, 0, "Editor");
}

namespace {
qintptr
syntaxColor(ScintillaEditorBackend& editor, qsizetype position)
{
    return editor.sendMessage(SCI_STYLEGETFORE, editor.sendMessage(SCI_GETSTYLEINDEXAT, position));
}
qintptr
expectedSyntaxColor(const QByteArray& text, const QByteArray& language, bool dark, qsizetype position)
{
    using namespace craftward::highlighting;
    const auto result =
      SyntaxHighlightingEngine::shared()->highlight(text, language, dark ? Theme::Dark : Theme::Light);
    for (const auto& span : result.spans) {
        if (span.utf8Start <= position && span.utf8End > position)
            return span.style.foreground.red() | (span.style.foreground.green() << 8) |
                   (span.style.foreground.blue() << 16);
    }
    return -1;
}
}

void
ScintillaQuickTest::syntaxHighlightingLifecycle()
{
    ScintillaEditorBackend editor;
    const QByteArray source("let value = 42;\n");
    editor.setText(QString::fromUtf8(source));
    editor.setLanguage(QStringLiteral("rust"));
    const auto keyword = expectedSyntaxColor(source, "rust", false, 0);
    QTRY_COMPARE(editor.syntaxName(), QStringLiteral("Rust"));
    QTRY_COMPARE(syntaxColor(editor, 0), keyword);
    QVERIFY(editor.languageRecognized());
    QVERIFY(!editor.canUndo());
    QVERIFY(!editor.sendMessage(SCI_GETMODIFY));
    editor.setFontPointSize(18);
    QCOMPARE(syntaxColor(editor, 0), keyword);
    editor.sendMessage(SCI_SETEMPTYSELECTION, 0);
    key(editor, Qt::Key_Slash, QStringLiteral("// "));
    const auto comment = expectedSyntaxColor("// " + source, "rust", false, 3);
    QTRY_COMPARE(syntaxColor(editor, 3), comment);
    editor.undo();
    QTRY_COMPARE(syntaxColor(editor, 0), keyword);
    QCOMPARE(editor.text().toUtf8(), source);
    QVERIFY(!editor.canUndo());
    editor.setDarkTheme(true);
    QTRY_COMPARE(syntaxColor(editor, 0), expectedSyntaxColor(source, "rust", true, 0));
    editor.setLanguage(QStringLiteral("text"));
    QTRY_COMPARE(editor.sendMessage(SCI_GETSTYLEINDEXAT, 0), 0);
    QTRY_COMPARE(editor.syntaxName(), QStringLiteral("Plain Text"));
    editor.setLanguage({});
    editor.setFilePath(QStringLiteral("/project/core/Cargo.lock"));
    editor.setText(QStringLiteral("version = 4\n"));
    QTRY_COMPARE(editor.syntaxName(), QStringLiteral("TOML"));
    QTRY_COMPARE(syntaxColor(editor, 10), expectedSyntaxColor("version = 4\n", "toml", true, 10));
}

void
ScintillaQuickTest::syntaxHighlightingDuringComposition()
{
    ScintillaEditorBackend editor;
    editor.setLanguage(QStringLiteral("rust"));
    editor.setText(QStringLiteral("let value = 1;\n"));
    const auto keyword = expectedSyntaxColor("let value = 1;\n", "rust", false, 0);
    QTRY_COMPARE(syntaxColor(editor, 0), keyword);
    editor.sendMessage(SCI_SETEMPTYSELECTION, 0);
    compose(editor, QString::fromUtf8("中文😀"));
    editor.setDarkTheme(true);
    QTest::qWait(20);
    QCOMPARE(editor.text(), QStringLiteral("let value = 1;\n"));
    compose(editor, {});
    QTRY_COMPARE(syntaxColor(editor, 0), expectedSyntaxColor("let value = 1;\n", "rust", true, 0));
    QVERIFY(!editor.canUndo());
    compose(editor, QString::fromUtf8("候选"));
    compose(editor, {}, QStringLiteral("// "));
    QTRY_COMPARE(syntaxColor(editor, 3), expectedSyntaxColor("// let value = 1;\n", "rust", true, 3));
    editor.undo();
    QTRY_COMPARE(syntaxColor(editor, 0), expectedSyntaxColor("let value = 1;\n", "rust", true, 0));
    QCOMPARE(editor.text(), QStringLiteral("let value = 1;\n"));
}

void
ScintillaQuickTest::syntaxHighlightingAfterRapidReplacement()
{
    ScintillaEditorBackend editor;
    for (int i = 0; i < 15; ++i) {
        editor.setLanguage(QStringLiteral("cpp"));
        editor.setText(QStringLiteral("/* unterminated\n").repeated(1000));
        editor.setLanguage(QStringLiteral("rust"));
        editor.setText(QStringLiteral("let value = 42;\n"));
    }
    QTRY_COMPARE(editor.syntaxName(), QStringLiteral("Rust"));
    QTRY_COMPARE(syntaxColor(editor, 0), expectedSyntaxColor("let value = 42;\n", "rust", false, 0));
    QCOMPARE(editor.text(), QStringLiteral("let value = 42;\n"));
    editor.setLanguage(QStringLiteral("unknown-language"));
    QTRY_COMPARE(editor.syntaxName(), QStringLiteral("Plain Text"));
    QTRY_COMPARE(editor.sendMessage(SCI_GETSTYLEINDEXAT, 0), 0);
    QVERIFY(!editor.languageRecognized());
}

void
ScintillaQuickTest::syntaxHighlightingRendersAndReplacesDocuments()
{
    QQuickWindow window;
    window.resize(640, 220);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(640, 220));
    editor.setShowLineNumbers(true);
    editor.setLanguage(QStringLiteral("rust"));
    editor.setText(QStringLiteral("fn main() {\n    let greeting = \"Hello, 世界\";\n    // Syntax highlighting in Qt "
                                  "Quick\n    println!(\"{}\", greeting);\n}\n"));
    const auto keyword = expectedSyntaxColor("fn main() {}", "rust", false, 0);
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QTRY_COMPARE(syntaxColor(editor, 0), keyword);
    const auto hasKeywordPixel = [&](const QImage& image) {
        const QColor target(keyword & 255, (keyword >> 8) & 255, (keyword >> 16) & 255);
        for (int y = 0; y < image.height(); ++y) {
            for (int x = 0; x < image.width(); ++x) {
                if (image.pixelColor(x, y) == target)
                    return true;
            }
        }
        return false;
    };
    QTRY_VERIFY(hasKeywordPixel(window.grabWindow()));
    const QString output = qEnvironmentVariable("CRAFTWARD_EDITOR_TEST_IMAGE");
    if (!output.isEmpty())
        QVERIFY(window.grabWindow().save(output));
    const auto replacement = editor.sendMessage(SCI_CREATEDOCUMENT);
    editor.sendMessage(SCI_SETDOCPOINTER, 0, replacement);
    editor.sendMessage(SCI_RELEASEDOCUMENT, 0, replacement);
    editor.sendMessage(SCI_ADDTEXT, 11, reinterpret_cast<qintptr>("let x = 1;\n"));
    QTRY_COMPARE(syntaxColor(editor, 0), expectedSyntaxColor("let x = 1;\n", "rust", false, 0));
    QCOMPARE(editor.text(), QStringLiteral("let x = 1;\n"));
}

void
ScintillaQuickTest::syntaxHighlightingRejectsSharedDocuments()
{
    ScintillaEditorBackend first;
    ScintillaEditorBackend second;
    const QString firstText = QStringLiteral("let value = 42;\n");
    const QString secondText = QStringLiteral("// comment\nlet value = 42;\n");
    for (auto* editor : { &first, &second }) {
        editor->setSize(QSizeF(480, 160));
        editor->setLanguage(QStringLiteral("rust"));
    }
    first.setText(firstText);
    second.setText(secondText);
    QTRY_VERIFY(first.highlightingReady() && second.highlightingReady());
    const auto keyword = expectedSyntaxColor(firstText.toUtf8(), "rust", false, 0);
    QCOMPARE(syntaxColor(first, 0), keyword);
    QCOMPARE(syntaxColor(second, 11), keyword);
    const auto firstDocument = first.sendMessage(SCI_GETDOCPOINTER);
    const auto secondDocument = second.sendMessage(SCI_GETDOCPOINTER);
    second.sendMessage(SCI_SETSEL, 3, 8);
    QSignalSpy textChanges(&second, &ScintillaEditorBackend::textChanged);
    QSignalSpy readyChanges(&second, &ScintillaEditorBackend::highlightingReadyChanged);

    second.sendMessage(SCI_SETDOCPOINTER, 0, firstDocument);
    QCOMPARE(second.sendMessage(SCI_GETSTATUS), SC_STATUS_FAILURE);
    QCOMPARE(second.sendMessage(SCI_GETDOCPOINTER), secondDocument);
    QCOMPARE(first.sendMessage(SCI_GETDOCPOINTER), firstDocument);
    QCOMPARE(second.text(), secondText);
    QCOMPARE(second.sendMessage(SCI_GETANCHOR), 3);
    QCOMPARE(second.sendMessage(SCI_GETCURRENTPOS), 8);
    QVERIFY(first.highlightingReady() && second.highlightingReady());
    QTest::qWait(20);
    QCOMPARE(textChanges.count(), 0);
    QCOMPARE(readyChanges.count(), 0);
    QCOMPARE(syntaxColor(first, 0), keyword);
    QCOMPARE(syntaxColor(second, 11), keyword);

    first.sendMessage(SCI_INSERTTEXT, 0, reinterpret_cast<qintptr>("// "));
    QTRY_COMPARE(syntaxColor(first, 3), expectedSyntaxColor("// let value = 42;\n", "rust", false, 3));
    QCOMPARE(second.text(), secondText);
    QCOMPARE(syntaxColor(second, 11), keyword);
}

void
ScintillaQuickTest::syntaxHighlightingTransfersDetachedDocuments_data()
{
    QTest::addColumn<bool>("destroyOwner");
    QTest::newRow("detach") << false;
    QTest::newRow("destroy") << true;
}

void
ScintillaQuickTest::syntaxHighlightingTransfersDetachedDocuments()
{
    QFETCH(bool, destroyOwner);
    ScintillaEditorBackend receiver;
    auto owner = std::make_unique<ScintillaEditorBackend>();
    const QString source = QStringLiteral("let value = 42;\n");
    owner->setLanguage(QStringLiteral("rust"));
    owner->setText(source);
    receiver.setLanguage(QStringLiteral("rust"));
    receiver.setText(QStringLiteral("// comment\n") + source);
    QTRY_VERIFY(owner->highlightingReady() && receiver.highlightingReady());
    const auto document = owner->sendMessage(SCI_GETDOCPOINTER);
    owner->sendMessage(SCI_ADDREFDOCUMENT, 0, document);
    const auto releaseDocument = qScopeGuard([&] { receiver.sendMessage(SCI_RELEASEDOCUMENT, 0, document); });
    if (destroyOwner)
        owner.reset();
    else {
        owner->sendMessage(SCI_SETDOCPOINTER);
        QCOMPARE(owner->text(), QString());
    }

    receiver.sendMessage(SCI_SETDOCPOINTER, 0, document);
    QCOMPARE(receiver.sendMessage(SCI_GETSTATUS), SC_STATUS_OK);
    QCOMPARE(receiver.sendMessage(SCI_GETDOCPOINTER), document);
    QCOMPARE(receiver.text(), source);
    QTRY_VERIFY(receiver.highlightingReady());
    QCOMPARE(syntaxColor(receiver, 0), expectedSyntaxColor(source.toUtf8(), "rust", false, 0));
    receiver.sendMessage(SCI_INSERTTEXT, 0, reinterpret_cast<qintptr>("// "));
    QTRY_COMPARE(syntaxColor(receiver, 3), expectedSyntaxColor("// let value = 42;\n", "rust", false, 3));
    if (owner)
        QCOMPARE(owner->text(), QString());
}

void
ScintillaQuickTest::documentSelfAttachmentIsNoOp()
{
    ScintillaEditorBackend editor;
    editor.setLanguage(QStringLiteral("rust"));
    editor.setText(QStringLiteral("let value = 42;\n"));
    QTRY_VERIFY(editor.highlightingReady());
    editor.sendMessage(SCI_INSERTTEXT, 0, reinterpret_cast<qintptr>("// "));
    QTRY_COMPARE(syntaxColor(editor, 3), expectedSyntaxColor("// let value = 42;\n", "rust", false, 3));
    editor.sendMessage(SCI_SETSEL, 3, 8);
    const auto document = editor.sendMessage(SCI_GETDOCPOINTER);
    QSignalSpy readyChanges(&editor, &ScintillaEditorBackend::highlightingReadyChanged);
    editor.sendMessage(SCI_SETDOCPOINTER, 0, document);
    QCOMPARE(editor.sendMessage(SCI_GETSTATUS), SC_STATUS_OK);
    QCOMPARE(editor.sendMessage(SCI_GETDOCPOINTER), document);
    QCOMPARE(editor.text(), QStringLiteral("// let value = 42;\n"));
    QCOMPARE(editor.sendMessage(SCI_GETANCHOR), 3);
    QCOMPARE(editor.sendMessage(SCI_GETCURRENTPOS), 8);
    QVERIFY(editor.highlightingReady());
    QCOMPARE(readyChanges.count(), 0);
    QVERIFY(editor.canUndo());
    editor.undo();
    QTRY_COMPARE(syntaxColor(editor, 0), expectedSyntaxColor("let value = 42;\n", "rust", false, 0));
}

void
ScintillaQuickTest::syntaxHighlightingRecoversAfterAnObsoleteError()
{
    ScintillaEditorBackend editor;
    editor.setLanguage(QStringLiteral("rust"));
    editor.setText(QStringLiteral("let x = 1;\n"));
    QTRY_COMPARE(syntaxColor(editor, 0), expectedSyntaxColor("let x = 1;\n", "rust", false, 0));
    const char invalid[] = { char(0xff), 0 };
    editor.sendMessage(SCI_INSERTTEXT, 0, reinterpret_cast<qintptr>(invalid));
    editor.sendMessage(SCI_DELETERANGE, 0, 1);
    editor.sendMessage(SCI_INSERTTEXT, 0, reinterpret_cast<qintptr>("// "));
    QTRY_COMPARE(syntaxColor(editor, 3), expectedSyntaxColor("// let x = 1;\n", "rust", false, 3));
    QCOMPARE(editor.text(), QStringLiteral("// let x = 1;\n"));
}

void
ScintillaQuickTest::syntaxHighlightingRecoversWithPendingConfiguration_data()
{
    QTest::addColumn<QString>("change");
    QTest::newRow("theme") << QStringLiteral("theme");
    QTest::newRow("path") << QStringLiteral("path");
    QTest::newRow("language") << QStringLiteral("language");
    QTest::newRow("replacement") << QStringLiteral("replacement");
}

void
ScintillaQuickTest::syntaxHighlightingRecoversWithPendingConfiguration()
{
    QFETCH(QString, change);
    ScintillaEditorBackend editor;
    editor.setSize(QSizeF(480, 160));
    editor.setLanguage(QStringLiteral("rust"));
    editor.setFilePath(QStringLiteral("main.rs"));
    editor.setText(QStringLiteral("let value = 42;\n"));
    QTRY_COMPARE(syntaxColor(editor, 0), expectedSyntaxColor("let value = 42;\n", "rust", false, 0));
    auto* document = editor.findChild<craftward::highlighting::SyntaxHighlightingDocument*>();
    QVERIFY(document);
    QSignalSpy failures(document, &craftward::highlighting::SyntaxHighlightingDocument::failed);
    BeforeQueuedCall beforeFailure;
    beforeFailure.callback = [&] {
        if (change == QStringLiteral("theme"))
            editor.setDarkTheme(true);
        else if (change == QStringLiteral("path"))
            editor.setFilePath(QStringLiteral("renamed.rs"));
        else if (change == QStringLiteral("language"))
            editor.setLanguage(QStringLiteral("cpp"));
        else
            editor.setText(QStringLiteral("let replacement = 42;\n"));
    };
    // Queue the change immediately before delivery of the real worker failure,
    // without letting the coalescing timer dispatch it first.
    document->installEventFilter(&beforeFailure);
    const char invalid[] = { char(0xff), 0 };
    editor.sendMessage(SCI_INSERTTEXT, 0, reinterpret_cast<qintptr>(invalid));
    QTRY_VERIFY(!failures.isEmpty());
    QVERIFY(!beforeFailure.callback);
    if (change == QStringLiteral("replacement")) {
        QTRY_VERIFY(editor.highlightingReady());
        QCOMPARE(editor.syntaxName(), QStringLiteral("Rust"));
        QCOMPARE(editor.text(), QStringLiteral("let replacement = 42;\n"));
        QCOMPARE(syntaxColor(editor, 0), expectedSyntaxColor(editor.text().toUtf8(), "rust", false, 0));
        return;
    }
    QTRY_COMPARE(editor.syntaxName(), QStringLiteral("Plain Text"));
    QTRY_VERIFY(editor.highlightingReady());
    QCOMPARE(editor.sendMessage(SCI_GETSTYLEINDEXAT, 1), 0);
    editor.sendMessage(SCI_DELETERANGE, 0, 1);
    editor.sendMessage(SCI_INSERTTEXT, 0, reinterpret_cast<qintptr>("// "));
    const QByteArray language = change == QStringLiteral("language") ? "cpp" : "rust";
    QTRY_VERIFY(editor.highlightingReady());
    QTRY_COMPARE(editor.syntaxName(), language == "cpp" ? QStringLiteral("C++") : QStringLiteral("Rust"));
    QTRY_COMPARE(syntaxColor(editor, 3),
                 expectedSyntaxColor("// let value = 42;\n", language, change == QStringLiteral("theme"), 3));
    QCOMPARE(editor.text(), QStringLiteral("// let value = 42;\n"));
}

void
ScintillaQuickTest::selectionPreservesSyntaxColors_data()
{
    QTest::addColumn<bool>("dark");
    QTest::addColumn<bool>("focused");
    QTest::addColumn<bool>("additional");
    for (bool dark : { false, true })
        for (bool focused : { false, true })
            for (bool additional : { false, true })
                QTest::newRow(qPrintable(QStringLiteral("%1-%2-%3").arg(dark).arg(focused).arg(additional)))
                  << dark << focused << additional;
}

void
ScintillaQuickTest::selectionPreservesSyntaxColors()
{
    QFETCH(bool, dark);
    QFETCH(bool, focused);
    QFETCH(bool, additional);
    QQuickWindow window;
    window.resize(480, 100);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(480, 100));
    editor.setFontPointSize(20);
    editor.setBackgroundColor(dark ? QColor("#282c34") : QColor("#fafafa"));
    editor.setForegroundColor(dark ? QColor("#abb2bf") : QColor("#383a42"));
    editor.setSelectionBackgroundColor(dark ? QColor("#35455e") : QColor("#dce8fa"));
    editor.setDarkTheme(dark);
    editor.setLanguage(QStringLiteral("rust"));
    editor.setText(QStringLiteral("let value = 42;\n"));
    const auto keyword = expectedSyntaxColor("let value = 42;\n", "rust", dark, 0);
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QTRY_COMPARE(syntaxColor(editor, 0), keyword);
    if (additional) {
        editor.sendMessage(SCI_SETMULTIPLESELECTION, 1);
        editor.sendMessage(SCI_SETSEL, 12, 14);
        editor.sendMessage(SCI_ADDSELECTION, 3, 0);
        editor.sendMessage(SCI_SETMAINSELECTION, 0);
    } else {
        editor.sendMessage(SCI_SETSEL, 0, 3);
    }
    editor.sendMessage(SCI_SETFOCUS, focused);
    const QColor target(keyword & 255, (keyword >> 8) & 255, (keyword >> 16) & 255);
    const auto rendersKeyword = [&] {
        const QImage image = window.grabWindow();
        for (int y = 0; y < image.height(); ++y)
            for (int x = 0; x < image.width(); ++x)
                if (image.pixelColor(x, y) == target)
                    return true;
        return false;
    };
    QTRY_VERIFY(rendersKeyword());
    const QString output = qEnvironmentVariable("CRAFTWARD_EDITOR_TEST_IMAGE");
    if (!output.isEmpty())
        QVERIFY(window.grabWindow().save(output));
}

void
ScintillaQuickTest::selectionForegroundOverrideCanBeReset()
{
    ScintillaEditorBackend editor;
    const int elements[] = { SC_ELEMENT_SELECTION_TEXT,
                             SC_ELEMENT_SELECTION_ADDITIONAL_TEXT,
                             SC_ELEMENT_SELECTION_SECONDARY_TEXT,
                             SC_ELEMENT_SELECTION_INACTIVE_TEXT,
                             SC_ELEMENT_SELECTION_INACTIVE_ADDITIONAL_TEXT };
    editor.setSelectionForegroundColor(QColor(Qt::red));
    for (int element : elements)
        QVERIFY(editor.sendMessage(SCI_GETELEMENTISSET, element));
    editor.resetSelectionForegroundColor();
    editor.setSelectionBackgroundColor(QColor(Qt::blue));
    editor.setDarkTheme(true);
    for (int element : elements)
        QVERIFY(!editor.sendMessage(SCI_GETELEMENTISSET, element));
}

void
ScintillaQuickTest::initialTextWaitsForVisibleStyles_data()
{
    QTest::addColumn<bool>("wrapped");
    QTest::addColumn<int>("startLine");
    QTest::newRow("top") << false << 0;
    QTest::newRow("requested-line") << false << 400;
    QTest::newRow("wrapped") << true << 0;
    QTest::newRow("wrapped-requested-line") << true << 400;
}

void
ScintillaQuickTest::initialTextWaitsForVisibleStyles()
{
    QFETCH(bool, wrapped);
    QFETCH(int, startLine);
    QQuickWindow window;
    PaintedEditor editor(window.contentItem());
    editor.setSize(QSizeF(480, 160));
    editor.setWordWrap(wrapped);
    editor.setLanguage(QStringLiteral("rust"));
    editor.setText(QStringLiteral("// An italic comment that wraps around a narrow editor viewport\nlet value = 42;\n")
                     .repeated(5000));
    editor.revealLocation(startLine);
    QVERIFY(!editor.highlightingReady());
    bool presented = false;
    qintptr tailStyleAtPresentation = -1;
    qintptr visibleStyleAtPresentation = -1;
    connect(&editor, &ScintillaEditorBackend::highlightingReadyChanged, &editor, [&] {
        if (editor.highlightingReady()) {
            presented = true;
            tailStyleAtPresentation = editor.sendMessage(SCI_GETSTYLEINDEXAT, editor.sendMessage(SCI_GETLENGTH) - 2);
            const auto position = editor.sendMessage(SCI_POSITIONFROMPOINT, 470, 150);
            visibleStyleAtPresentation = editor.sendMessage(SCI_GETSTYLEINDEXAT, position);
        }
    });
    const QImage initial = editor.renderImage();
    for (int y = 0; y < initial.height(); ++y)
        for (int x = 0; x < initial.width(); ++x)
            QCOMPARE(initial.pixelColor(x, y), editor.backgroundColor());
    QTRY_VERIFY(presented);
    QCOMPARE(tailStyleAtPresentation, 0);
    QVERIFY(visibleStyleAtPresentation > 0);
    const auto commentStyle = editor.sendMessage(SCI_GETSTYLEINDEXAT, 0);
    QVERIFY(editor.sendMessage(SCI_STYLEGETITALIC, commentStyle));
    const QImage shown = editor.renderImage();
    QVERIFY(shown != initial);
    editor.sendMessage(SCI_SETEMPTYSELECTION, 0);
    key(editor, Qt::Key_Space, QStringLiteral(" "));
    QVERIFY(editor.highlightingReady());
}

void
ScintillaQuickTest::initialPresentationRecoversAfterReplacementAndFailure()
{
    QQuickWindow window;
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(480, 160));
    for (int i = 0; i < 5; ++i) {
        editor.setFilePath(QStringLiteral("/project/main.cpp"));
        editor.setText(QStringLiteral("/* unfinished\n").repeated(1000));
        editor.setFilePath(QStringLiteral("/project/main.qml"));
        editor.setText(QStringLiteral("Item {\n    focusPolicy: Qt.StrongFocus\n}\n"));
    }
    QVERIFY(!editor.highlightingReady());
    QTRY_VERIFY(editor.highlightingReady());
    QCOMPARE(editor.syntaxName(), QStringLiteral("QML"));
    QVERIFY(editor.languageRecognized());
    editor.setText({});
    QVERIFY(editor.highlightingReady());
    editor.setLanguage(QStringLiteral("unknown-language"));
    editor.setText(QStringLiteral("unrecognized input"));
    QTRY_VERIFY(editor.highlightingReady());
    QCOMPARE(editor.syntaxName(), QStringLiteral("Plain Text"));
    editor.setLanguage(QStringLiteral("rust"));
    editor.setText(QStringLiteral("let value = 1;\n"));
    const char invalid[] = { char(0xff), 0 };
    QTest::ignoreMessage(QtWarningMsg, QRegularExpression(QStringLiteral("Syntax highlighting:.*")));
    editor.sendMessage(SCI_INSERTTEXT, 0, reinterpret_cast<qintptr>(invalid));
    QTRY_VERIFY(editor.highlightingReady());
    QCOMPARE(editor.syntaxName(), QStringLiteral("Plain Text"));
    editor.setText(QStringLiteral("let recovered = 42;\n"));
    QTRY_VERIFY(editor.highlightingReady());
    QCOMPARE(editor.syntaxName(), QStringLiteral("Rust"));
}

void
ScintillaQuickTest::textAndReadOnly()
{
    ScintillaEditorBackend editor;
    const QString text = QString::fromUtf8("hello 中文 😀\nsecond") + QChar(0) + QStringLiteral("tail");
    editor.setText(text);
    QCOMPARE(editor.text(), text);
    QCOMPARE(editor.sendMessage(SCI_GETLENGTH), text.toUtf8().size());
    QVERIFY(!editor.canUndo());
    editor.setReadOnly(true);
    key(editor, Qt::Key_A, QStringLiteral("a"));
    QGuiApplication::clipboard()->setText(QStringLiteral("paste"));
    editor.paste();
    QCOMPARE(editor.text(), text);
    editor.setText(QStringLiteral("replacement"));
    QCOMPARE(editor.text(), QStringLiteral("replacement"));
    QVERIFY(editor.isReadOnly());
}

void
ScintillaQuickTest::readOnlyPreservesViewport()
{
    QQuickWindow window;
    window.resize(360, 180);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    editor.setText((QStringLiteral("wide text ").repeated(100) + '\n').repeated(30));
    editor.sendMessage(SCI_SETMARGINWIDTHN, 0, 30);
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QTRY_VERIFY(editor.horizontalSize() < 1);
    editor.setHorizontalPosition(1);
    editor.setVerticalPosition(0.5);
    const auto offset = editor.sendMessage(SCI_GETXOFFSET);
    const auto line = editor.sendMessage(SCI_GETFIRSTVISIBLELINE);
    const auto scrollWidth = editor.sendMessage(SCI_GETSCROLLWIDTH);
    QVERIFY(offset > 0);
    QVERIFY(line > 0);
    for (const bool readOnly : { true, false }) {
        editor.setReadOnly(readOnly);
        QTest::qWait(50);
        QCOMPARE(editor.sendMessage(SCI_GETREADONLY), readOnly);
        QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), offset);
        QCOMPARE(editor.sendMessage(SCI_GETFIRSTVISIBLELINE), line);
        QCOMPARE(editor.sendMessage(SCI_GETSCROLLWIDTH), scrollWidth);
        QCOMPARE(editor.sendMessage(SCI_GETMARGINWIDTHN, 0), 30);
    }
}

void
ScintillaQuickTest::editingAndUndo()
{
    ScintillaEditorBackend editor;
    editor.setText(QStringLiteral("hello"));
    editor.sendMessage(SCI_SETEMPTYSELECTION, 5);
    key(editor, Qt::Key_Return);
    key(editor, Qt::Key_Tab);
    key(editor, Qt::Key_unknown, QString::fromUtf8("世界😀"));
    QVERIFY(editor.text().startsWith(QStringLiteral("hello\n\t")));
    QVERIFY(editor.text().endsWith(QString::fromUtf8("世界😀")));
    QVERIFY(editor.canUndo());
    const QString changed = editor.text();
    editor.undo();
    QVERIFY(editor.text() != changed);
    editor.redo();
    QCOMPARE(editor.text(), changed);
    editor.selectAll();
    editor.copy();
    QCOMPARE(QGuiApplication::clipboard()->text(), changed);
}

void
ScintillaQuickTest::multipleSelections()
{
    ScintillaEditorBackend editor;
    editor.setText(QStringLiteral("ab\ncd"));
    editor.sendMessage(SCI_SETSELECTION, 1, 1);
    editor.sendMessage(SCI_ADDSELECTION, 4, 4);
    key(editor, Qt::Key_X, QStringLiteral("x"));
    QCOMPARE(editor.text(), QStringLiteral("axb\ncxd"));
    editor.undo();
    QCOMPARE(editor.text(), QStringLiteral("ab\ncd"));
}

void
ScintillaQuickTest::imeUtf16AndUndo()
{
    ScintillaEditorBackend editor;
    editor.setText(QString::fromUtf8("a😀z"));
    editor.sendMessage(SCI_SETEMPTYSELECTION, 5);
    QCOMPARE(editor.inputMethodQuery(Qt::ImCursorPosition).toInt(), 3);
    QCOMPARE(editor.inputMethodQuery(Qt::ImSurroundingText).toString(), editor.text());
    compose(editor, QStringLiteral("ni"));
    QCOMPARE(editor.text(), QString::fromUtf8("a😀z"));
    QCOMPARE(editor.inputMethodQuery(Qt::ImAbsolutePosition).toInt(), 3);
    compose(editor, QString::fromUtf8("你"));
    QCOMPARE(editor.text(), QString::fromUtf8("a😀z"));
    QCOMPARE(editor.inputMethodQuery(Qt::ImCursorPosition).toInt(), 3);
    compose(editor, {}, QString::fromUtf8("你"));
    QCOMPARE(editor.text(), QString::fromUtf8("a😀你z"));
    editor.undo();
    QCOMPARE(editor.text(), QString::fromUtf8("a😀z"));
    editor.redo();
    QCOMPARE(editor.text(), QString::fromUtf8("a😀你z"));
    compose(editor, {}, QString::fromUtf8("文"), -1, 1);
    QCOMPARE(editor.text(), QString::fromUtf8("a😀文z"));
}

void
ScintillaQuickTest::imeInsertionOffset_data()
{
    QTest::addColumn<QString>("initial");
    QTest::addColumn<int>("cursor");
    QTest::addColumn<int>("offset");
    QTest::addColumn<QString>("preedit");
    QTest::addColumn<QString>("expected");
    QTest::newRow("before-cursor") << QStringLiteral("ab") << 2 << -1 << QString() << QStringLiteral("aXb");
    QTest::newRow("after-cursor") << QStringLiteral("ab") << 0 << 1 << QString() << QStringLiteral("aXb");
    QTest::newRow("before-surrogate-pair")
      << QString::fromUtf8("a😀z") << 5 << -2 << QString() << QString::fromUtf8("aX😀z");
    QTest::newRow("after-surrogate-pair")
      << QString::fromUtf8("a😀z") << 1 << 2 << QString() << QString::fromUtf8("a😀Xz");
    QTest::newRow("before-preedit") << QStringLiteral("ab") << 2 << -1 << QStringLiteral("candidate")
                                    << QStringLiteral("aXb");
    QTest::newRow("after-preedit") << QStringLiteral("ab") << 0 << 1 << QStringLiteral("candidate")
                                   << QStringLiteral("aXb");
}

void
ScintillaQuickTest::imeInsertionOffset()
{
    QFETCH(QString, initial);
    QFETCH(int, cursor);
    QFETCH(int, offset);
    QFETCH(QString, preedit);
    QFETCH(QString, expected);
    ScintillaEditorBackend editor;
    editor.setText(initial);
    editor.sendMessage(SCI_SETEMPTYSELECTION, cursor);
    if (!preedit.isEmpty())
        compose(editor, preedit);
    compose(editor, {}, QStringLiteral("X"), offset, 0);
    QCOMPARE(editor.text(), expected);
    editor.undo();
    QCOMPARE(editor.text(), initial);
    QVERIFY(!editor.canUndo());
    editor.redo();
    QCOMPARE(editor.text(), expected);
}

void
ScintillaQuickTest::imeCommitWithNextPreedit()
{
    ScintillaEditorBackend editor;
    editor.setText(QStringLiteral("a"));
    editor.sendMessage(SCI_SETEMPTYSELECTION, 1);
    QSignalSpy changes(&editor, &ScintillaEditorBackend::textChanged);
    compose(editor, QStringLiteral("ni"));
    QCoreApplication::processEvents();
    QCOMPARE(changes.count(), 0);
    compose(editor, QStringLiteral("hao"), QString::fromUtf8("你"));
    QTRY_COMPARE(changes.count(), 1);
    QCOMPARE(editor.text(), QString::fromUtf8("a你"));
    compose(editor, {}, QString::fromUtf8("好"));
    QTRY_COMPARE(changes.count(), 2);
    QCOMPARE(editor.text(), QString::fromUtf8("a你好"));
}

void
ScintillaQuickTest::cancelledCompositionRestoresSelection()
{
    ScintillaEditorBackend editor;
    editor.setText(QStringLiteral("selected"));
    editor.selectAll();
    compose(editor, QStringLiteral("candidate"));
    compose(editor, {});
    QCOMPARE(editor.text(), QStringLiteral("selected"));
    QVERIFY(editor.hasSelection());
    QCOMPARE(editor.sendMessage(SCI_GETSELECTIONSTART), 0);
    QCOMPARE(editor.sendMessage(SCI_GETSELECTIONEND), 8);
    QVERIFY(!editor.canUndo());
}

void
ScintillaQuickTest::imeCommitReplacesSelection_data()
{
    QTest::addColumn<int>("start");
    QTest::addColumn<int>("length");
    QTest::addColumn<QString>("commit");
    QTest::addColumn<bool>("preedit");
    QTest::addColumn<bool>("reverseSelection");
    QTest::addColumn<bool>("multiple");
    QTest::addColumn<QString>("expected");
    for (const bool preedit : { false, true }) {
        const QByteArray suffix = preedit ? "-preedit" : "-direct";
        QTest::newRow("selection" + suffix)
          << 0 << 0 << QStringLiteral("X") << preedit << false << false << QStringLiteral("aXe");
        QTest::newRow("replace-next" + suffix)
          << 0 << 1 << QStringLiteral("X") << preedit << false << false << QStringLiteral("aX");
        QTest::newRow("insert-after" + suffix)
          << 1 << 0 << QStringLiteral("X") << preedit << false << false << QStringLiteral("aeX");
        QTest::newRow("replace-before" + suffix)
          << -1 << 1 << QStringLiteral("X") << preedit << false << false << QStringLiteral("Xe");
        QTest::newRow("delete-next" + suffix)
          << 0 << 1 << QString() << preedit << false << false << QStringLiteral("a");
        QTest::newRow("reverse-selection" + suffix)
          << 0 << 1 << QStringLiteral("X") << preedit << true << false << QStringLiteral("aX");
        QTest::newRow("multiple-replace" + suffix)
          << 0 << 1 << QStringLiteral("X") << preedit << false << true << QStringLiteral("aX\naX");
        QTest::newRow("multiple-offset" + suffix)
          << 1 << 0 << QStringLiteral("X") << preedit << false << true << QStringLiteral("aeX\naeX");
    }
}

void
ScintillaQuickTest::imeCommitReplacesSelection()
{
    QFETCH(int, start);
    QFETCH(int, length);
    QFETCH(QString, commit);
    QFETCH(bool, preedit);
    QFETCH(bool, reverseSelection);
    QFETCH(bool, multiple);
    QFETCH(QString, expected);
    ScintillaEditorBackend editor;
    const QString original = multiple ? QStringLiteral("abcde\nabcde") : QStringLiteral("abcde");
    editor.setText(original);
    editor.sendMessage(SCI_SETSEL, reverseSelection ? 4 : 1, reverseSelection ? 1 : 4);
    if (multiple)
        editor.sendMessage(SCI_ADDSELECTION, 10, 7);
    if (preedit)
        compose(editor, QStringLiteral("candidate"));
    compose(editor, {}, commit, start, length);
    QCOMPARE(editor.text(), expected);
    editor.undo();
    QCOMPARE(editor.text(), original);
    QVERIFY(!editor.canUndo());
    editor.redo();
    QCOMPARE(editor.text(), expected);
}

void
ScintillaQuickTest::imePreeditFormats_data()
{
    QTest::addColumn<bool>("multiple");
    QTest::addColumn<bool>("wrap");
    QTest::newRow("single") << false << false;
    QTest::newRow("multiple") << true << false;
    QTest::newRow("wrapped") << false << true;
}

void
ScintillaQuickTest::imePreeditFormats()
{
    QFETCH(bool, multiple);
    QFETCH(bool, wrap);
    QQuickWindow window;
    window.resize(360, 180);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    editor.setWordWrap(wrap);
    const QString original =
      multiple ? QStringLiteral("prefix suffix\nprefix suffix") : QStringLiteral("prefix suffix");
    editor.setText(original);
    editor.sendMessage(SCI_SETSELECTION, 7, 7);
    if (multiple)
        editor.sendMessage(SCI_ADDSELECTION, 21, 21);
    editor.sendMessage(SCI_SETCARETPERIOD, 0);
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    const QString preedit = QString::fromUtf8("AB😀CD") + (wrap ? QStringLiteral("wide ").repeated(15) : QString());
    const auto sendFormats = [&](QColor background, bool underline) {
        QTextCharFormat first;
        first.setBackground(Qt::blue);
        first.setForeground(Qt::white);
        QTextCharFormat rest;
        rest.setBackground(background);
        rest.setForeground(Qt::green);
        rest.setFontUnderline(underline);
        rest.setUnderlineColor(Qt::magenta);
        QInputMethodEvent event(
          preedit,
          { { QInputMethodEvent::TextFormat, 0, 2, QVariant::fromValue(QTextFormat(first)) },
            { QInputMethodEvent::TextFormat, 2, int(preedit.size()) - 2, QVariant::fromValue(QTextFormat(rest)) } });
        QCoreApplication::sendEvent(&editor, &event);
        QTest::qWait(50);
        return window.grabWindow();
    };
    const auto countColour = [](const QImage& image, QColor colour, QRect bounds = {}) {
        int count = 0;
        if (bounds.isNull())
            bounds = image.rect();
        for (int y = bounds.top(); y <= bounds.bottom(); ++y)
            for (int x = bounds.left(); x <= bounds.right(); ++x)
                count += image.pixelColor(x, y) == colour;
        return count;
    };
    const auto countUnderline = [](const QImage& image) {
        int count = 0;
        for (int y = 0; y < image.height(); ++y) {
            for (int x = 0; x < image.width(); ++x) {
                const QColor pixel = image.pixelColor(x, y);
                // A one-pixel magenta underline blends with its background at fractional scales.
                count += pixel.red() - pixel.green() > 50 && pixel.blue() - pixel.green() > 50;
            }
        }
        return count;
    };
    const QImage red = sendFormats(Qt::red, true);
    QVERIFY(!red.isNull());
    QVERIFY(countColour(red, Qt::blue) > 0);
    QVERIFY(countColour(red, Qt::red) > 0);
    QVERIFY(countColour(red, Qt::green) > 0);
    QVERIFY(countUnderline(red) > 0);
    if (multiple || wrap) {
        const int lineHeight = editor.sendMessage(SCI_TEXTHEIGHT, 0);
        const int y = qCeil(lineHeight * qreal(red.height()) / window.height());
        QVERIFY(countColour(red, Qt::red, QRect(0, y, red.width(), red.height() - y)) > 0);
    }
    if (wrap)
        QVERIFY(editor.sendMessage(SCI_WRAPCOUNT, 0) > 1);
    QCOMPARE(editor.text(), original);
    const QImage yellow = sendFormats(Qt::yellow, false);
    QVERIFY(countColour(yellow, Qt::blue) > 0);
    QVERIFY(countColour(yellow, Qt::yellow) > 0);
    QCOMPARE(countColour(yellow, Qt::red), 0);
    QCOMPARE(countUnderline(yellow), 0);
    compose(editor, {});
    QTest::qWait(50);
    const QImage cancelled = window.grabWindow();
    QCOMPARE(countColour(cancelled, Qt::blue), 0);
    QCOMPARE(countColour(cancelled, Qt::yellow), 0);
    QCOMPARE(editor.text(), original);
    QVERIFY(!editor.canUndo());
    sendFormats(Qt::red, true);
    compose(editor, {}, preedit);
    QTest::qWait(50);
    const QImage committed = window.grabWindow();
    QCOMPARE(countColour(committed, Qt::blue), 0);
    QCOMPARE(countColour(committed, Qt::red), 0);
    QCOMPARE(countColour(committed, Qt::green), 0);
    QCOMPARE(countUnderline(committed), 0);
    editor.undo();
    QCOMPARE(editor.text(), original);
    QVERIFY(!editor.canUndo());
}

void
ScintillaQuickTest::focusAfterVisibility_data()
{
    QTest::addColumn<bool>("hideParent");
    QTest::newRow("editor") << false;
    QTest::newRow("ancestor") << true;
}

void
ScintillaQuickTest::focusAfterVisibility()
{
    QFETCH(bool, hideParent);
    QQuickWindow window;
    window.resize(360, 180);
    QQuickItem parent(window.contentItem());
    ScintillaEditorBackend editor(&parent);
    editor.setSize(QSizeF(360, 180));
    QQuickItem other(window.contentItem());
    window.show();
    window.requestActivate();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    editor.forceActiveFocus();
    QTRY_VERIFY(editor.hasActiveFocus());
    QCOMPARE(editor.sendMessage(SCI_GETFOCUS), 1);
    QQuickItem* hidden = hideParent ? &parent : &editor;
    hidden->setVisible(false);
    QVERIFY(editor.hasActiveFocus());
    QCOMPARE(editor.sendMessage(SCI_GETFOCUS), 0);
    hidden->setVisible(true);
    QVERIFY(editor.hasActiveFocus());
    QCOMPARE(editor.sendMessage(SCI_GETFOCUS), 1);

    other.forceActiveFocus();
    QVERIFY(other.hasActiveFocus());
    hidden->setVisible(false);
    hidden->setVisible(true);
    QVERIFY(other.hasActiveFocus());
    QVERIFY(!editor.hasActiveFocus());
    QCOMPARE(editor.sendMessage(SCI_GETFOCUS), 0);
}

void
ScintillaQuickTest::completionUsesQuickItems()
{
    QQuickWindow window;
    window.resize(360, 180);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    editor.setText(QStringLiteral("f"));
    editor.sendMessage(SCI_SETEMPTYSELECTION, 1);
    const auto windowCount = QGuiApplication::allWindows().size();
    const QByteArray words("foo fun");
    editor.sendMessage(SCI_AUTOCSHOW, 1, reinterpret_cast<qintptr>(words.constData()));
    QVERIFY(editor.sendMessage(SCI_AUTOCACTIVE));
    QCOMPARE(QGuiApplication::allWindows().size(), windowCount);
    key(editor, Qt::Key_Down);
    key(editor, Qt::Key_Return);
    QCOMPARE(editor.text(), QStringLiteral("fun"));
    QVERIFY(!editor.sendMessage(SCI_AUTOCACTIVE));

    const QByteArray tip("fun(argument)");
    editor.sendMessage(SCI_CALLTIPSHOW, 0, reinterpret_cast<qintptr>(tip.constData()));
    QVERIFY(editor.sendMessage(SCI_CALLTIPACTIVE));
    QCOMPARE(QGuiApplication::allWindows().size(), windowCount);
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QVERIFY(!window.grabWindow().isNull());
    editor.sendMessage(SCI_CALLTIPCANCEL);
    QVERIFY(!editor.sendMessage(SCI_CALLTIPACTIVE));
}

void
ScintillaQuickTest::completionScrolling_data()
{
    QTest::addColumn<bool>("usePixels");
    QTest::addColumn<bool>("includeAngles");
    QTest::addColumn<int>("steps");
    QTest::addColumn<bool>("restartGesture");
    QTest::newRow("no-scroll") << false << false << 0 << false;
    QTest::newRow("coarse-angle") << false << false << 1 << false;
    QTest::newRow("fine-angle") << false << false << 5 << false;
    QTest::newRow("small-angle") << false << false << 10 << false;
    QTest::newRow("pixels") << true << false << 1 << false;
    QTest::newRow("small-pixels") << true << false << 5 << false;
    QTest::newRow("pixels-take-precedence") << true << true << 5 << false;
    QTest::newRow("new-angle-gesture") << false << false << 5 << true;
    QTest::newRow("new-pixel-gesture") << true << false << 5 << true;
}

void
ScintillaQuickTest::completionScrolling()
{
    QFETCH(bool, usePixels);
    QFETCH(bool, includeAngles);
    QFETCH(int, steps);
    QFETCH(bool, restartGesture);
    QQuickWindow window;
    window.resize(360, 180);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    editor.forceActiveFocus();
    editor.sendMessage(SCI_AUTOCSETMAXHEIGHT, 5);
    const QByteArray words("a0 a1 a2 a3 a4 a5 a6 a7 a8 a9");
    editor.sendMessage(SCI_AUTOCSHOW, 0, reinterpret_cast<qintptr>(words.constData()));
    QQuickItem* popup = nullptr;
    for (auto* child : window.contentItem()->childItems())
        if (child != &editor && child->isVisible())
            popup = child;
    QVERIFY(popup);
    const QPointF point(10, 10);
    const auto scroll = [&](QPoint pixels, QPoint angles, Qt::ScrollPhase phase) {
        QWheelEvent event(point, popup->mapToGlobal(point), pixels, angles, Qt::NoButton, Qt::NoModifier, phase, false);
        QCoreApplication::sendEvent(popup, &event);
    };
    const int distance = usePixels ? editor.sendMessage(SCI_TEXTHEIGHT, 0) : 120;
    for (int i = 0; i < steps; ++i) {
        const bool restart = restartGesture && i == steps - 1;
        if (restart)
            scroll({}, {}, Qt::ScrollEnd);
        const int delta = distance * i / steps - distance * (i + 1) / steps;
        scroll(usePixels ? QPoint(0, delta) : QPoint(),
               usePixels ? (includeAngles ? QPoint(0, -120) : QPoint()) : QPoint(0, delta),
               i == 0 || restart ? Qt::ScrollBegin : Qt::ScrollUpdate);
    }
    scroll({}, {}, Qt::ScrollEnd);
    QMouseEvent click(
      QEvent::MouseButtonPress, point, popup->mapToGlobal(point), Qt::LeftButton, Qt::LeftButton, Qt::NoModifier);
    QCoreApplication::sendEvent(popup, &click);
    key(editor, Qt::Key_Return);
    QCOMPARE(editor.text(), steps && !restartGesture ? QStringLiteral("a1") : QStringLiteral("a0"));
}

void
ScintillaQuickTest::textDropAndReadOnly()
{
    QQuickWindow window;
    window.resize(360, 180);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    editor.setText(QStringLiteral("hello"));
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QMimeData mime;
    mime.setText(QString::fromUtf8("世界😀"));
    const QPoint point(200, 5);
    QDragEnterEvent enter(point, Qt::CopyAction, &mime, Qt::LeftButton, Qt::NoModifier);
    QCoreApplication::sendEvent(&editor, &enter);
    QVERIFY(enter.isAccepted());
    QDropEvent drop(point, Qt::CopyAction, &mime, Qt::LeftButton, Qt::NoModifier);
    QCoreApplication::sendEvent(&editor, &drop);
    QVERIFY(drop.isAccepted());
    QCOMPARE(editor.text(), QString::fromUtf8("hello世界😀"));
    editor.undo();
    QCOMPARE(editor.text(), QStringLiteral("hello"));
    editor.setReadOnly(true);
    QDropEvent rejected(point, Qt::CopyAction, &mime, Qt::LeftButton, Qt::NoModifier);
    QCoreApplication::sendEvent(&editor, &rejected);
    QVERIFY(!rejected.isAccepted());
    QCOMPARE(editor.text(), QStringLiteral("hello"));
}

void
ScintillaQuickTest::textDropDuringComposition_data()
{
    QTest::addColumn<QString>("resolution");
    QTest::addColumn<bool>("selectedAndWrapped");
    for (const char* resolution : { "commit", "cancel", "reset", "fallback" }) {
        for (const bool selectedAndWrapped : { false, true }) {
            const QByteArray suffix = selectedAndWrapped ? "-selection-wrapped" : "-caret";
            QTest::newRow(QByteArray(resolution) + suffix) << QString::fromLatin1(resolution) << selectedAndWrapped;
        }
    }
}

void
ScintillaQuickTest::textDropDuringComposition()
{
    QFETCH(QString, resolution);
    QFETCH(bool, selectedAndWrapped);
    QQuickWindow window;
    window.resize(360, 180);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    editor.setWordWrap(selectedAndWrapped);
    const QString original = QStringLiteral("abc def");
    editor.setText(original);
    editor.sendMessage(SCI_SETSEL, selectedAndWrapped ? 4 : 3, selectedAndWrapped ? 7 : 3);
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    editor.forceActiveFocus();
    QVERIFY(editor.hasActiveFocus());

    // Model the platform's response to commit/reset without depending on an installed IME.
    TestInputContext context;
    bool pending = true;
    int commitRequests = 0;
    int resetRequests = 0;
    context.onCommit = [&] {
        ++commitRequests;
        if (!pending || resolution == QStringLiteral("reset") || resolution == QStringLiteral("fallback"))
            return;
        pending = false;
        compose(editor, {}, resolution == QStringLiteral("commit") ? QString::fromUtf8("你") : QString());
    };
    context.onReset = [&] {
        ++resetRequests;
        if (!std::exchange(pending, false))
            return;
        if (resolution != QStringLiteral("fallback"))
            compose(editor, {});
    };
    const QScopedValueRollback<QPlatformInputContext*> inputContext(
      QInputMethodPrivate::get(QGuiApplication::inputMethod())->testContext, &context);
    compose(editor, QString::fromUtf8("ni😀 ").repeated(20));
    QCOMPARE(editor.text(), original);
    QSignalSpy changes(&editor, &ScintillaEditorBackend::textChanged);

    QMimeData mime;
    mime.setText(QStringLiteral("DROP"));
    const QPoint point(200, 5);
    QDragEnterEvent enter(point, Qt::CopyAction, &mime, Qt::LeftButton, Qt::NoModifier);
    QCoreApplication::sendEvent(&window, &enter);
    QVERIFY(enter.isAccepted());
    QDropEvent drop(point, Qt::CopyAction, &mime, Qt::LeftButton, Qt::NoModifier);
    QCoreApplication::sendEvent(&window, &drop);
    QVERIFY(drop.isAccepted());
    QVERIFY(editor.hasActiveFocus());
    QVERIFY(!pending);
    QCOMPARE(commitRequests, 1);
    QCOMPARE(resetRequests, resolution == QStringLiteral("reset") || resolution == QStringLiteral("fallback") ? 1 : 0);
    const QString resolved = resolution == QStringLiteral("commit")
                               ? (selectedAndWrapped ? QString::fromUtf8("abc 你") : QString::fromUtf8("abc你 def"))
                               : original;
    const QString dropped = resolved + QStringLiteral("DROP");
    QTRY_VERIFY(!changes.isEmpty());
    QCOMPARE(editor.text(), dropped);

    // Finishing the input context again must not affect an already accepted drop.
    QGuiApplication::inputMethod()->commit();
    QGuiApplication::inputMethod()->reset();
    QCOMPARE(editor.text(), dropped);
    editor.undo();
    QCOMPARE(editor.text(), resolved);
    editor.redo();
    QCOMPARE(editor.text(), dropped);
    editor.undo();
    if (resolution == QStringLiteral("commit")) {
        editor.undo();
        QCOMPARE(editor.text(), original);
    }
    QVERIFY(!editor.canUndo());
}

void
ScintillaQuickTest::wrappingAndHorizontalScroll()
{
    QQuickWindow window;
    window.resize(360, 180);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    editor.setText(QStringLiteral("a long line of text ").repeated(30));
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QTRY_VERIFY(editor.horizontalSize() < 1);
    editor.setHorizontalPosition(1 - editor.horizontalSize());
    QVERIFY(editor.horizontalPosition() > 0);
    editor.setWordWrap(true);
    QTRY_COMPARE(editor.horizontalSize(), 1);
    QTRY_VERIFY(editor.sendMessage(SCI_WRAPCOUNT, 0) > 1);
    editor.setWidth(180);
    QTRY_VERIFY(editor.verticalSize() < 1);
    editor.setWordWrap(false);
    QTRY_VERIFY(editor.horizontalSize() < 1);
}

void
ScintillaQuickTest::horizontalScrollInputs_data()
{
    QTest::addColumn<QString>("input");
    for (const char* input : { "pixels", "angles", "shift-wheel", "scrollbar", "line-message", "offset-message" })
        QTest::newRow(input) << QString::fromLatin1(input);
}

void
ScintillaQuickTest::horizontalScrollInputs()
{
    QFETCH(QString, input);
    QQuickWindow window;
    window.resize(360, 180);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    editor.setText(QStringLiteral("short"));
    editor.sendMessage(SCI_SETSCROLLWIDTHTRACKING, 0);
    editor.sendMessage(SCI_SETSCROLLWIDTH, 1000);
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    editor.setHorizontalPosition(1);
    const auto rightEdge = editor.sendMessage(SCI_GETXOFFSET);
    QVERIFY(rightEdge > 0);
    editor.setHorizontalPosition(0);
    const auto scroll = [&](int direction) {
        const int distance = direction * 100000;
        if (input == QStringLiteral("scrollbar")) {
            editor.setHorizontalPosition(direction * 2.0);
        } else if (input == QStringLiteral("line-message")) {
            editor.sendMessage(SCI_LINESCROLL, quintptr(distance));
        } else if (input == QStringLiteral("offset-message")) {
            editor.sendMessage(SCI_SETXOFFSET, quintptr(distance));
        } else {
            const bool pixels = input == QStringLiteral("pixels");
            const bool shift = input == QStringLiteral("shift-wheel");
            const QPoint angles = pixels ? QPoint() : (shift ? QPoint(0, -distance) : QPoint(-distance, 0));
            const QPointF point(20, 20);
            QWheelEvent event(point,
                              editor.mapToGlobal(point),
                              pixels ? QPoint(-distance, 0) : QPoint(),
                              angles,
                              Qt::NoButton,
                              shift ? Qt::ShiftModifier : Qt::NoModifier,
                              Qt::NoScrollPhase,
                              false);
            QCoreApplication::sendEvent(&editor, &event);
        }
    };
    for (int i = 0; i < 3; ++i) {
        scroll(1);
        QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), rightEdge);
        QCOMPARE(editor.sendMessage(SCI_GETSCROLLWIDTH), 1000);
        QVERIFY(qAbs(editor.horizontalPosition() + editor.horizontalSize() - 1) < 1e-9);
    }
    scroll(-1);
    QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), 0);
    QCOMPARE(editor.horizontalPosition(), 0);

    editor.setWordWrap(true);
    scroll(1);
    QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), 0);
    QCOMPARE(editor.horizontalPosition(), 0);
    QCOMPARE(editor.horizontalSize(), 1);
}

void
ScintillaQuickTest::horizontalScrollRangeChanges()
{
    QQuickWindow window;
    window.resize(600, 180);
    PaintedEditor editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    editor.setText(QStringLiteral("short"));
    editor.sendMessage(SCI_SETSCROLLWIDTHTRACKING, 0);
    editor.sendMessage(SCI_SETSCROLLWIDTH, 1000);
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    editor.setHorizontalPosition(1);
    const auto originalOffset = editor.sendMessage(SCI_GETXOFFSET);
    editor.setWidth(560);
    QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), originalOffset - 200);
    QVERIFY(qAbs(editor.horizontalPosition() + editor.horizontalSize() - 1) < 1e-9);
    editor.setWidth(180);
    QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), originalOffset - 200);

    editor.setHorizontalPosition(1);
    const auto beforeMargin = editor.sendMessage(SCI_GETXOFFSET);
    editor.sendMessage(SCI_SETMARGINWIDTHN, 0, 40);
    QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), beforeMargin);
    editor.setHorizontalPosition(1);
    QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), beforeMargin + 40);
    editor.sendMessage(SCI_SETMARGINWIDTHN, 0, 0);
    // Margin changes take effect when Scintilla refreshes its layout.
    QTRY_COMPARE(editor.sendMessage(SCI_GETXOFFSET), beforeMargin);

    QSignalSpy changes(&editor, &ScintillaEditorBackend::scrollChanged);
    editor.painted.clear();
    editor.sendMessage(SCI_SETSCROLLWIDTH, 300);
    QVERIFY(qAbs(editor.horizontalPosition() + editor.horizontalSize() - 1) < 1e-9);
    QTRY_VERIFY(!changes.isEmpty());
    QTRY_VERIFY(!editor.painted.isEmpty());
    editor.sendMessage(SCI_SETSCROLLWIDTH, 50);
    QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), 0);
    QCOMPARE(editor.horizontalPosition(), 0);
    QCOMPARE(editor.horizontalSize(), 1);
    editor.setHorizontalPosition(1);
    QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), 0);

    editor.sendMessage(SCI_SETSCROLLWIDTH, 1000);
    editor.setShowLineNumbers(true);
    editor.setHorizontalPosition(1);
    const auto numberedOffset = editor.sendMessage(SCI_GETXOFFSET);
    const auto marginWidth = editor.sendMessage(SCI_GETMARGINWIDTHN, 0);
    editor.setShowLineNumbers(false);
    QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), numberedOffset - marginWidth);
    QVERIFY(qAbs(editor.horizontalPosition() + editor.horizontalSize() - 1) < 1e-9);
}

void
ScintillaQuickTest::horizontalRangeAfterContentShrinks_data()
{
    QTest::addColumn<QString>("change");
    QTest::newRow("replace-document") << QStringLiteral("replace");
    QTest::newRow("delete-text") << QStringLiteral("delete");
    QTest::newRow("smaller-font") << QStringLiteral("font");
}

void
ScintillaQuickTest::horizontalRangeAfterContentShrinks()
{
    QFETCH(QString, change);
    QQuickWindow window;
    window.resize(360, 180);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    editor.setShowLineNumbers(true);
    editor.setText(change == QStringLiteral("font") ? QStringLiteral("wide ").repeated(4)
                                                    : QStringLiteral("wide ").repeated(100));
    if (change == QStringLiteral("font"))
        editor.setFontPointSize(52);
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QTRY_VERIFY(editor.horizontalSize() < 1);
    editor.setHorizontalPosition(1);
    QVERIFY(editor.sendMessage(SCI_GETXOFFSET) > 0);
    if (change == QStringLiteral("replace")) {
        editor.setText(QStringLiteral("short"));
    } else if (change == QStringLiteral("delete")) {
        editor.selectAll();
        key(editor, Qt::Key_S, QStringLiteral("short"));
    } else {
        editor.setFontPointSize(13);
    }
    QTRY_COMPARE_WITH_TIMEOUT(editor.horizontalSize(), 1.0, 1000);
    editor.setHorizontalPosition(1);
    QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), 0);
    QVERIFY(editor.sendMessage(SCI_POINTXFROMPOSITION, 0, editor.sendMessage(SCI_GETLENGTH)) >
            editor.sendMessage(SCI_GETMARGINWIDTHN, 0));
}

void
ScintillaQuickTest::horizontalExtentIncludesOffscreenLines()
{
    QQuickWindow window;
    window.resize(360, 180);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    const QString widest = QStringLiteral("x").repeated(200);
    const QString narrower = QStringLiteral("x").repeated(100);
    editor.setText(QStringLiteral("short\n").repeated(100) + widest + '\n' + narrower);
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    const QByteArray sample = widest.toUtf8();
    const auto expected =
      editor.sendMessage(SCI_TEXTWIDTH, STYLE_DEFAULT, reinterpret_cast<qintptr>(sample.constData()));
    QTRY_VERIFY(editor.sendMessage(SCI_GETSCROLLWIDTH) >= expected);
    const auto originalWidth = editor.sendMessage(SCI_GETSCROLLWIDTH);
    key(editor, Qt::Key_A, QStringLiteral("a"));
    QCoreApplication::processEvents();
    QCOMPARE(editor.sendMessage(SCI_GETSCROLLWIDTH), originalWidth);
    editor.sendMessage(
      SCI_SETSEL, editor.sendMessage(SCI_POSITIONFROMLINE, 100), editor.sendMessage(SCI_POSITIONFROMLINE, 101));
    editor.deleteSelection();
    QTRY_VERIFY(editor.sendMessage(SCI_GETSCROLLWIDTH) < originalWidth * 0.75);
    const auto narrowerWidth = editor.sendMessage(SCI_GETSCROLLWIDTH);
    QVERIFY(narrowerWidth > originalWidth * 0.45);
    editor.undo();
    QTRY_COMPARE(editor.sendMessage(SCI_GETSCROLLWIDTH), originalWidth);
    editor.redo();
    QTRY_COMPARE(editor.sendMessage(SCI_GETSCROLLWIDTH), narrowerWidth);
}

void
ScintillaQuickTest::cargoLockScrollRange()
{
    QFile source(QFINDTESTDATA("../../../../core/Cargo.lock"));
    QVERIFY(source.open(QIODevice::ReadOnly));
    const QString text = QString::fromUtf8(source.readAll());
    QQuickWindow window;
    window.resize(960, 640);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(960, 640));
    editor.setShowLineNumbers(true);
    editor.setText(QStringLiteral("wide ").repeated(500));
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QTRY_VERIFY(editor.horizontalSize() < 1);
    editor.setText(text);
    window.grabWindow();
    qintptr maximum = 0;
    for (const QString& line : text.split('\n')) {
        const QByteArray bytes = line.toUtf8();
        maximum =
          qMax(maximum, editor.sendMessage(SCI_TEXTWIDTH, STYLE_DEFAULT, reinterpret_cast<qintptr>(bytes.constData())));
    }
    editor.setHorizontalPosition(1);
    QVERIFY2(
      editor.sendMessage(SCI_GETXOFFSET) < maximum,
      qPrintable(
        QStringLiteral("Offset %1 exceeds the widest line (%2)").arg(editor.sendMessage(SCI_GETXOFFSET)).arg(maximum)));
}

void
ScintillaQuickTest::editorScrollBars_data()
{
    QTest::addColumn<bool>("vertical");
    QTest::addColumn<bool>("minimumThumb");
    QTest::addColumn<bool>("hover");
    QTest::newRow("vertical-wheel") << true << false << false;
    QTest::newRow("horizontal-wheel") << false << false << false;
    QTest::newRow("vertical-large-document") << true << true << false;
    QTest::newRow("horizontal-long-line") << false << true << false;
    QTest::newRow("vertical-hover") << true << false << true;
    QTest::newRow("horizontal-hover") << false << false << true;
}

void
ScintillaQuickTest::editorScrollBars()
{
    QFETCH(bool, vertical);
    QFETCH(bool, minimumThumb);
    QFETCH(bool, hover);
    QQmlEngine engine;
    QQmlComponent component(&engine);
    component.setData(R"(
        import QtQuick
        import Craftward.Editor
        CodeEditor { width: 360; height: 180; showLineNumbers: true }
    )",
                      QUrl());
    QScopedPointer<QObject> root(component.create());
    QVERIFY2(root, qPrintable(component.errorString()));
    auto* item = qobject_cast<QQuickItem*>(root.data());
    auto* editor = root->findChild<ScintillaEditorBackend*>();
    QVERIFY(editor);
    editor->setText(vertical ? QStringLiteral("line\n").repeated(minimumThumb ? 10000 : 80)
                             : QStringLiteral("x").repeated(minimumThumb ? 10000 : 200));
    QQuickWindow window;
    window.resize(360, 180);
    item->setParentItem(window.contentItem());
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QQuickItem* bar = nullptr;
    for (auto* child : root->findChildren<QQuickItem*>()) {
        if (child->inherits("QQuickScrollBar") &&
            child->property("orientation").toInt() == int(vertical ? Qt::Vertical : Qt::Horizontal))
            bar = child;
    }
    QVERIFY(bar);
    QTRY_VERIFY(bar->isVisible());
    auto* thumb = qvariant_cast<QQuickItem*>(bar->property("contentItem"));
    QVERIFY(thumb);
    if (minimumThumb) {
        editor->forceActiveFocus();
        QTRY_VERIFY(thumb->opacity() > 0.5);
        QVERIFY2((vertical ? thumb->height() : thumb->width()) >= 18,
                 qPrintable(
                   QStringLiteral("Thumb is only %1 logical pixels").arg(vertical ? thumb->height() : thumb->width())));
        const QPoint start = thumb->mapToScene(thumb->boundingRect().center()).toPoint();
        const QPoint end = bar
                             ->mapToScene(vertical ? QPointF(bar->width() / 2, bar->height() - 3)
                                                   : QPointF(bar->width() - 3, bar->height() / 2))
                             .toPoint();
        QTest::mousePress(&window, Qt::LeftButton, {}, start);
        QTest::mouseMove(&window, end);
        QTest::mouseRelease(&window, Qt::LeftButton, {}, end);
        QTRY_VERIFY(vertical ? editor->verticalPosition() > 0.9 : editor->horizontalPosition() > 0.9);
    } else if (hover) {
        window.contentItem()->forceActiveFocus();
        QTest::mouseMove(&window, bar->mapToScene(bar->boundingRect().center()).toPoint());
        QTRY_VERIFY(thumb->opacity() > 0.5);
    } else {
        window.contentItem()->forceActiveFocus();
        QVERIFY(!editor->hasActiveFocus());
        const QPointF position(100, 80);
        QWheelEvent wheel(position,
                          editor->mapToGlobal(position),
                          vertical ? QPoint(0, -60) : QPoint(-120, 0),
                          QPoint(),
                          Qt::NoButton,
                          Qt::NoModifier,
                          Qt::ScrollUpdate,
                          false);
        QCoreApplication::sendEvent(editor, &wheel);
        QVERIFY(vertical ? editor->verticalPosition() > 0 : editor->horizontalPosition() > 0);
        QTRY_VERIFY_WITH_TIMEOUT(thumb->opacity() > 0.5, 1000);
    }
    const QString output = qEnvironmentVariable("CRAFTWARD_EDITOR_SCROLLBAR_IMAGE");
    if (vertical && hover && !output.isEmpty())
        QVERIFY(window.grabWindow().save(output));
    item->setParentItem(nullptr);
}

void
ScintillaQuickTest::horizontalScrollCaretExpansion_data()
{
    QTest::addColumn<QString>("action");
    QTest::newRow("scroll-caret") << QStringLiteral("scroll-caret");
    QTest::newRow("typing") << QStringLiteral("typing");
    QTest::newRow("virtual-space") << QStringLiteral("virtual-space");
}

void
ScintillaQuickTest::horizontalScrollCaretExpansion()
{
    QFETCH(QString, action);
    QQuickWindow window;
    window.resize(360, 180);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    editor.setText(action == QStringLiteral("virtual-space") ? QStringLiteral("x") : QStringLiteral("x").repeated(200));
    editor.sendMessage(SCI_SETSCROLLWIDTHTRACKING, 0);
    editor.sendMessage(SCI_SETSCROLLWIDTH, 1);
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QCOMPARE(editor.horizontalSize(), 1);
    const auto end = editor.sendMessage(SCI_GETLENGTH);
    editor.sendMessage(SCI_SETEMPTYSELECTION, end);
    if (action == QStringLiteral("virtual-space")) {
        editor.sendMessage(SCI_SETVIRTUALSPACEOPTIONS, SCVS_USERACCESSIBLE);
        editor.sendMessage(SCI_SETSELECTIONNCARETVIRTUALSPACE, 0, 200);
        editor.sendMessage(SCI_SETSELECTIONNANCHORVIRTUALSPACE, 0, 200);
    }
    if (action == QStringLiteral("typing"))
        key(editor, Qt::Key_X, QStringLiteral("x"));
    else
        editor.sendMessage(SCI_SCROLLCARET);
    QVERIFY(editor.sendMessage(SCI_GETSCROLLWIDTH) > 1);
    QVERIFY(editor.sendMessage(SCI_GETXOFFSET) > 0);
    QVERIFY(editor.horizontalPosition() > 0);
    QVERIFY(editor.horizontalPosition() + editor.horizontalSize() <= 1 + 1e-9);
    if (action != QStringLiteral("virtual-space")) {
        const auto caret = editor.sendMessage(SCI_GETCURRENTPOS);
        const auto x = editor.sendMessage(SCI_POINTXFROMPOSITION, 0, caret);
        QVERIFY(x >= 0 && x < editor.width());
    } else {
        QCOMPARE(editor.sendMessage(SCI_GETSELECTIONNCARETVIRTUALSPACE, 0), 200);
    }
}

void
ScintillaQuickTest::horizontalRangeWithVirtualCaret_data()
{
    QTest::addColumn<QString>("change");
    QTest::addColumn<bool>("lineNumbers");
    for (const bool lineNumbers : { false, true }) {
        const QByteArray suffix = lineNumbers ? "-line-numbers" : "-no-margin";
        for (const auto* change : { "color", "weight", "edit", "smaller-font" })
            QTest::newRow(QByteArray(change) + suffix) << QString::fromLatin1(change) << lineNumbers;
    }
}

void
ScintillaQuickTest::horizontalRangeWithVirtualCaret()
{
    QFETCH(QString, change);
    QFETCH(bool, lineNumbers);
    QQuickWindow window;
    window.resize(360, 180);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    editor.setShowLineNumbers(lineNumbers);
    editor.setText(QStringLiteral("x\nsecond"));
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    editor.forceActiveFocus();
    QTest::qWait(50);
    QCOMPARE(editor.sendMessage(SCI_GETSCROLLWIDTHTRACKING), 1);
    editor.sendMessage(SCI_SETEMPTYSELECTION, 1);
    editor.sendMessage(SCI_SETVIRTUALSPACEOPTIONS, SCVS_USERACCESSIBLE);
    editor.sendMessage(SCI_SETSELECTIONNCARETVIRTUALSPACE, 0, 200);
    editor.sendMessage(SCI_SETSELECTIONNANCHORVIRTUALSPACE, 0, 200);
    editor.sendMessage(SCI_SCROLLCARET);
    const auto caretX = [&] {
        const QByteArray spaces(200, ' ');
        return editor.sendMessage(SCI_POINTXFROMPOSITION, 0, 1) +
               editor.sendMessage(SCI_TEXTWIDTH, STYLE_DEFAULT, reinterpret_cast<qintptr>(spaces.constData()));
    };
    QTest::qWait(50);
    QVERIFY(caretX() >= editor.sendMessage(SCI_GETMARGINWIDTHN, 0) && caretX() < editor.width());
    const auto previousOffset = editor.sendMessage(SCI_GETXOFFSET);
    if (change == QStringLiteral("color"))
        editor.setForegroundColor(Qt::red);
    else if (change == QStringLiteral("weight"))
        editor.setFontWeight(SC_WEIGHT_BOLD);
    else if (change == QStringLiteral("edit"))
        editor.sendMessage(SCI_APPENDTEXT, 1, reinterpret_cast<qintptr>("!"));
    else
        editor.setFontPointSize(11);
    // Let queued width measurement finish before checking that the caret is still visible.
    QTest::qWait(100);
    QVERIFY(!window.grabWindow().isNull());
    QCOMPARE(editor.sendMessage(SCI_GETSELECTIONNCARETVIRTUALSPACE, 0), 200);
    QVERIFY(caretX() >= editor.sendMessage(SCI_GETMARGINWIDTHN, 0) && caretX() < editor.width());
    if (change == QStringLiteral("color") || change == QStringLiteral("edit"))
        QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), previousOffset);
    editor.sendMessage(SCI_SETEMPTYSELECTION, 1);
    QTRY_COMPARE(editor.horizontalSize(), 1.0);
    QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), 0);
}

void
ScintillaQuickTest::horizontalRangeWithVirtualSelections()
{
    QQuickWindow window;
    window.resize(360, 180);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    editor.setShowLineNumbers(true);
    editor.setText(QStringLiteral("x\nsecond"));
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QTest::qWait(50);
    editor.sendMessage(SCI_SETEMPTYSELECTION, 1);
    const auto end = editor.sendMessage(SCI_GETLENGTH);
    editor.sendMessage(SCI_ADDSELECTION, end, end);
    editor.sendMessage(SCI_SETMAINSELECTION, 0);
    editor.sendMessage(SCI_SETVIRTUALSPACEOPTIONS, SCVS_USERACCESSIBLE);
    editor.sendMessage(SCI_SETSELECTIONNCARETVIRTUALSPACE, 1, 120);
    editor.sendMessage(SCI_SETSELECTIONNANCHORVIRTUALSPACE, 1, 200);
    // The secondary anchor uses the line-end style's space width, not STYLE_DEFAULT.
    editor.sendMessage(SCI_STYLESETSIZEFRACTIONAL, 1, 20 * SC_FONT_SIZE_MULTIPLIER);
    editor.sendMessage(SCI_STARTSTYLING, editor.sendMessage(SCI_POSITIONFROMLINE, 1));
    editor.sendMessage(SCI_SETSTYLING, 6, 1);
    const QByteArray spaces(200, ' ');
    const auto required = editor.sendMessage(SCI_TEXTWIDTH, 1, reinterpret_cast<qintptr>(spaces.constData()));
    QTRY_VERIFY(editor.sendMessage(SCI_GETSCROLLWIDTH) > required);
    editor.setHorizontalPosition(1);
    const auto anchorX = editor.sendMessage(SCI_POINTXFROMPOSITION, 0, end) + required;
    QVERIFY(anchorX >= editor.sendMessage(SCI_GETMARGINWIDTHN, 0) && anchorX < editor.width());
    editor.sendMessage(SCI_SETEMPTYSELECTION, 1);
    QTRY_COMPARE(editor.horizontalSize(), 1.0);
    QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), 0);
}

void
ScintillaQuickTest::horizontalRangeWithOffscreenVirtualSelections_data()
{
    QTest::addColumn<int>("selectionIndex");
    QTest::addColumn<unsigned int>("virtualSpaceMessage");
    for (const int selectionIndex : { 0, 1 }) {
        const QByteArray prefix = selectionIndex ? "secondary-" : "main-";
        QTest::newRow(prefix + "caret") << selectionIndex << unsigned(SCI_SETSELECTIONNCARETVIRTUALSPACE);
        QTest::newRow(prefix + "anchor") << selectionIndex << unsigned(SCI_SETSELECTIONNANCHORVIRTUALSPACE);
    }
}

void
ScintillaQuickTest::horizontalRangeWithOffscreenVirtualSelections()
{
    QFETCH(int, selectionIndex);
    QFETCH(unsigned int, virtualSpaceMessage);
    QQuickWindow window;
    window.resize(360, 180);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    editor.setShowLineNumbers(true);
    editor.setText(QStringLiteral("short\n").repeated(100) + QStringLiteral("last"));
    const auto end = editor.sendMessage(SCI_GETLENGTH);
    editor.sendMessage(SCI_SETEMPTYSELECTION, selectionIndex ? 1 : end);
    if (selectionIndex) {
        editor.sendMessage(SCI_ADDSELECTION, end, end);
        editor.sendMessage(SCI_SETMAINSELECTION, 0);
    }
    editor.sendMessage(SCI_SETVIRTUALSPACEOPTIONS, SCVS_USERACCESSIBLE);
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    // Leave the editor unfocused so caret blinking cannot trigger a compensating repaint.
    window.contentItem()->forceActiveFocus();
    QTest::qWait(100);
    QVERIFY(!editor.hasActiveFocus());
    QCOMPARE(editor.sendMessage(SCI_GETFIRSTVISIBLELINE), 0);
    QVERIFY(editor.sendMessage(SCI_POINTYFROMPOSITION, 0, end) >= editor.height());
    QCOMPARE(editor.horizontalSize(), 1.0);
    const auto originalWidth = editor.sendMessage(SCI_GETSCROLLWIDTH);
    QSignalSpy scrollChanges(&editor, &ScintillaEditorBackend::scrollChanged);

    // Both expansion and contraction must work without repainting the offscreen selection.
    editor.sendMessage(virtualSpaceMessage, selectionIndex, 200);
    QTRY_VERIFY_WITH_TIMEOUT(editor.horizontalSize() < 1, 1000);
    QTRY_VERIFY(!scrollChanges.isEmpty());
    editor.setHorizontalPosition(1);
    QTest::qWait(100);
    QVERIFY(editor.sendMessage(SCI_GETXOFFSET) > 0);
    scrollChanges.clear();
    editor.sendMessage(virtualSpaceMessage, selectionIndex, 0);
    QTRY_COMPARE_WITH_TIMEOUT(editor.horizontalSize(), 1.0, 1000);
    QTRY_VERIFY(!scrollChanges.isEmpty());
    QCOMPARE(editor.sendMessage(SCI_GETSCROLLWIDTH), originalWidth);
    QCOMPARE(editor.sendMessage(SCI_GETXOFFSET), 0);
    QCOMPARE(editor.sendMessage(SCI_GETFIRSTVISIBLELINE), 0);
}

void
ScintillaQuickTest::lineNumberWidth_data()
{
    QTest::addColumn<int>("lineCount");
    QTest::newRow("9-to-10") << 9;
    QTest::newRow("99-to-100") << 99;
}

void
ScintillaQuickTest::lineNumberWidth()
{
    QFETCH(int, lineCount);
    ScintillaEditorBackend editor;
    QVERIFY(!editor.showLineNumbers());
    QCOMPARE(editor.sendMessage(SCI_GETMARGINWIDTHN, 0), 0);
    const QString original = QStringLiteral("line\n").repeated(lineCount - 1) + QStringLiteral("last");
    editor.setText(original);
    editor.setShowLineNumbers(true);
    const auto originalWidth = editor.sendMessage(SCI_GETMARGINWIDTHN, 0);
    QVERIFY(originalWidth > 0);
    editor.sendMessage(SCI_SETEMPTYSELECTION, editor.sendMessage(SCI_GETLENGTH));
    key(editor, Qt::Key_Return, QStringLiteral("\n"));
    QTRY_VERIFY(editor.sendMessage(SCI_GETMARGINWIDTHN, 0) > originalWidth);
    const auto wider = editor.sendMessage(SCI_GETMARGINWIDTHN, 0);
    editor.undo();
    QTRY_COMPARE(editor.sendMessage(SCI_GETMARGINWIDTHN, 0), originalWidth);
    QCOMPARE(editor.text(), original);
    editor.redo();
    QTRY_COMPARE(editor.sendMessage(SCI_GETMARGINWIDTHN, 0), wider);
    key(editor, Qt::Key_Backspace);
    QTRY_COMPARE(editor.sendMessage(SCI_GETMARGINWIDTHN, 0), originalWidth);

    editor.setShowLineNumbers(false);
    editor.setText(original + '\n');
    QCOMPARE(editor.sendMessage(SCI_GETMARGINWIDTHN, 0), 0);
    editor.setShowLineNumbers(true);
    QCOMPARE(editor.sendMessage(SCI_GETMARGINWIDTHN, 0), wider);
    editor.setText(original);
    QCOMPARE(editor.sendMessage(SCI_GETMARGINWIDTHN, 0), originalWidth);
    editor.setText(QString());
    QCOMPARE(editor.sendMessage(SCI_GETLINECOUNT), 1);
    QVERIFY(editor.sendMessage(SCI_GETMARGINWIDTHN, 0) > 0);
    QVERIFY(editor.sendMessage(SCI_GETMARGINWIDTHN, 0) <= originalWidth);
}

void
ScintillaQuickTest::lineNumberFontAndZoom()
{
    ScintillaEditorBackend editor;
    editor.setText(QStringLiteral("line\n").repeated(100));
    editor.setShowLineNumbers(true);
    editor.setLineNumberColor(QColor("#336699"));
    const auto originalWidth = editor.sendMessage(SCI_GETMARGINWIDTHN, 0);
    const auto originalSize = editor.fontPointSize();
    editor.setFontPointSize(originalSize * 2);
    QVERIFY(editor.sendMessage(SCI_GETMARGINWIDTHN, 0) > originalWidth);
    editor.setBackgroundColor(QColor("#102030"));
    editor.setFontWeight(SC_WEIGHT_BOLD);
    QCOMPARE(editor.sendMessage(SCI_STYLEGETFORE, STYLE_LINENUMBER), 0x996633);
    QCOMPARE(editor.sendMessage(SCI_STYLEGETBACK, STYLE_LINENUMBER), 0x302010);
    editor.setFontWeight(SC_WEIGHT_NORMAL);
    editor.setFontPointSize(originalSize);
    QCOMPARE(editor.sendMessage(SCI_GETMARGINWIDTHN, 0), originalWidth);
    editor.sendMessage(SCI_SETZOOM, 8);
    QTRY_VERIFY(editor.sendMessage(SCI_GETMARGINWIDTHN, 0) > originalWidth);
    editor.sendMessage(SCI_SETZOOM, 0);
    QTRY_COMPARE(editor.sendMessage(SCI_GETMARGINWIDTHN, 0), originalWidth);
}

void
ScintillaQuickTest::lineNumbersDuringComposition()
{
    QQuickWindow window;
    window.resize(360, 180);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    const QString original = QStringLiteral("line\n").repeated(8) + QStringLiteral("last");
    editor.setText(original);
    editor.setShowLineNumbers(true);
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    const auto originalWidth = editor.sendMessage(SCI_GETMARGINWIDTHN, 0);
    editor.sendMessage(SCI_SETEMPTYSELECTION, editor.sendMessage(SCI_GETLENGTH));
    compose(editor, QStringLiteral("\n候选"));
    QTRY_VERIFY(editor.sendMessage(SCI_GETMARGINWIDTHN, 0) > originalWidth);
    QCOMPARE(editor.text(), original);
    compose(editor, QString());
    QTRY_COMPARE(editor.sendMessage(SCI_GETMARGINWIDTHN, 0), originalWidth);
    QCOMPARE(editor.text(), original);
    QVERIFY(!editor.canUndo());
}

void
ScintillaQuickTest::lineNumbersAndWrappedSelection()
{
    QQuickWindow window;
    window.resize(240, 360);
    ScintillaEditorBackend editor(window.contentItem());
    editor.setSize(QSizeF(240, 360));
    editor.setShowLineNumbers(true);
    editor.setWordWrap(true);
    editor.setReadOnly(true);
    editor.setText(QStringLiteral("wrapped text ").repeated(8) + QStringLiteral("\nsecond\nthird\nfourth"));
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QTRY_VERIFY(editor.sendMessage(SCI_WRAPCOUNT, 0) > 1);
    const int wraps = editor.sendMessage(SCI_WRAPCOUNT, 0);
    const int height = editor.sendMessage(SCI_TEXTHEIGHT, 0);
    const int margin = editor.sendMessage(SCI_GETMARGINWIDTHN, 0);
    const QImage image = window.grabWindow();
    QVERIFY(!image.isNull());
    const qreal ratio = qreal(image.width()) / window.width();
    const auto hasNumber = [&](int displayLine) {
        for (int y = qCeil((displayLine * height + 2) * ratio); y < qFloor(((displayLine + 1) * height - 2) * ratio);
             ++y)
            for (int x = 0; x < qFloor(margin * ratio); ++x)
                if (image.pixelColor(x, y) != editor.backgroundColor())
                    return true;
        return false;
    };
    QVERIFY(hasNumber(0));
    for (int line = 1; line < wraps; ++line)
        QVERIFY(!hasNumber(line));
    QVERIFY(hasNumber(wraps));

    QTest::mouseClick(&window, Qt::LeftButton, {}, QPoint(margin / 2, (wraps + 0.5) * height));
    QCOMPARE(editor.sendMessage(SCI_GETSELECTIONSTART), editor.sendMessage(SCI_POSITIONFROMLINE, 1));
    QCOMPARE(editor.sendMessage(SCI_GETSELECTIONEND), editor.sendMessage(SCI_POSITIONFROMLINE, 2));
    QTest::mousePress(&window, Qt::LeftButton, {}, QPoint(margin / 2, height * 1.5));
    QCOMPARE(editor.sendMessage(SCI_GETSELECTIONSTART), 0);
    QCOMPARE(editor.sendMessage(SCI_GETSELECTIONEND), editor.sendMessage(SCI_POSITIONFROMLINE, 1));
    const QPoint end(margin / 2, (wraps + 1.5) * height);
    QTest::mouseMove(&window, end);
    QTest::mouseRelease(&window, Qt::LeftButton, {}, end);
    QCOMPARE(editor.sendMessage(SCI_GETSELECTIONSTART), 0);
    QCOMPARE(editor.sendMessage(SCI_GETSELECTIONEND), editor.sendMessage(SCI_POSITIONFROMLINE, 3));
}

void
ScintillaQuickTest::lineNumbersInViews_data()
{
    QTest::addColumn<bool>("fileView");
    QTest::newRow("file") << true;
    QTest::newRow("legal") << false;
}

void
ScintillaQuickTest::lineNumbersInViews()
{
    QFETCH(bool, fileView);
    QQmlEngine engine;
    const QString path = fileView ? QStringLiteral(":/editor-tests/Pages/FileContentView.qml")
                                  : QStringLiteral(":/editor-tests/Features/Legal/LegalTextView.qml");
    QFile source(path);
    QVERIFY(source.open(QIODevice::ReadOnly));
    QQmlComponent component(&engine);
    // These standalone views only need the editor plugin, not the other page-module dependencies.
    component.setData(source.readAll(), QUrl());
    const QString text = QStringLiteral("// Scintilla line numbers\n\nint main()\n{\n    return 0;\n}\n\n") +
                         QStringLiteral("// Another line\n").repeated(20);
    QVariantMap properties;
    if (fileView)
        properties.insert(QStringLiteral("file"),
                          QVariantMap{ { QStringLiteral("text"), text },
                                       { QStringLiteral("error"), QString() },
                                       { QStringLiteral("path"), QString() } });
    else
        properties.insert(QStringLiteral("text"), text);
    QScopedPointer<QObject> root(component.createWithInitialProperties(properties));
    QVERIFY2(root, qPrintable(component.errorString()));
    auto* editor = root->findChild<ScintillaEditorBackend*>();
    QVERIFY(editor);
    QCOMPARE(editor->showLineNumbers(), fileView);
    QCOMPARE(editor->isReadOnly(), !fileView);
    QCOMPARE(editor->text(), text);
    QQuickWindow window;
    window.resize(480, 320);
    auto* item = qobject_cast<QQuickItem*>(root.data());
    item->setSize(QSizeF(480, 320));
    item->setParentItem(window.contentItem());
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    const QImage image = window.grabWindow();
    QVERIFY(!image.isNull());
    QCOMPARE(editor->sendMessage(SCI_GETMARGINWIDTHN, 0) > 0, fileView);
    const QString output = qEnvironmentVariable("CRAFTWARD_EDITOR_LINE_NUMBERS_IMAGE");
    if (fileView && !output.isEmpty())
        QVERIFY(image.save(output));
    if (fileView) {
        const QPointF origin = editor->mapToItem(item, QPointF());
        QVERIFY(origin.x() >= 8);
        QVERIFY(origin.y() >= 8);
        const qreal ratio = qreal(image.width()) / window.width();
        int numberRight = -1;
        const qreal marginRight = origin.x() + editor->sendMessage(SCI_GETMARGINWIDTHN, 0);
        for (int y = qCeil(origin.y() * ratio);
             y < qFloor((origin.y() + editor->sendMessage(SCI_TEXTHEIGHT, 0)) * ratio);
             ++y)
            for (int x = qCeil(origin.x() * ratio); x < qFloor(marginRight * ratio); ++x)
                if (image.pixelColor(x, y) != editor->backgroundColor())
                    numberRight = qMax(numberRight, x);
        QVERIFY(numberRight >= 0);
        const qreal textStart = origin.x() + editor->sendMessage(SCI_POINTXFROMPOSITION, 0, 0);
        QVERIFY(textStart - (numberRight + 1) / ratio >= 8);
    }
    item->setParentItem(nullptr);
}

void
ScintillaQuickTest::rendersInsideQuickScene()
{
    QQmlEngine engine;
    QQmlComponent component(&engine);
    component.setData(R"(
        import QtQuick
        import Craftward.TestEditor 1.0
        Rectangle {
            width: 240; height: 160; color: "#ff00ff"
            Item {
                x: 10; y: 10; width: 130; height: 100; clip: true
                Editor { objectName: "editor"; width: 200; height: 130; text: "Quick editor"; backgroundColor: "white" }
            }
            Rectangle { x: 40; y: 40; width: 30; height: 30; color: "#0000ff" }
        }
    )",
                      QUrl());
    QScopedPointer<QObject> root(component.create());
    QVERIFY2(root, qPrintable(component.errorString()));
    QQuickWindow window;
    window.resize(240, 160);
    auto* item = qobject_cast<QQuickItem*>(root.data());
    item->setParentItem(window.contentItem());
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QImage image;
    QTRY_VERIFY(!(image = window.grabWindow()).isNull());
    image = window.grabWindow();
    const qreal ratio = image.width() / 240.0;
    QCOMPARE(image.pixelColor(qRound(50 * ratio), qRound(50 * ratio)), QColor(Qt::blue));
    QCOMPARE(image.pixelColor(qRound(150 * ratio), qRound(85 * ratio)), QColor(Qt::magenta));
    QCOMPARE(image.pixelColor(qRound(30 * ratio), qRound(80 * ratio)), QColor(Qt::white));
    const QString output = qEnvironmentVariable("CRAFTWARD_EDITOR_TEST_IMAGE");
    if (!output.isEmpty())
        QVERIFY(image.save(output));
    auto* editor = root->findChild<ScintillaEditorBackend*>(QStringLiteral("editor"));
    QVERIFY(editor);
    QTest::mouseClick(&window, Qt::LeftButton, {}, QPoint(30, 30));
    QTRY_VERIFY(editor->hasActiveFocus());
    item->setParentItem(nullptr);
}

void
ScintillaQuickTest::partialPaintAndScrollReuse_data()
{
    QTest::addColumn<bool>("showLineNumbers");
    QTest::newRow("text-only") << false;
    QTest::newRow("line-numbers") << true;
}

void
ScintillaQuickTest::partialPaintAndScrollReuse()
{
    QFETCH(bool, showLineNumbers);
    QQuickWindow window;
    window.resize(360, 180);
    PaintedEditor editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    editor.setReadOnly(true);
    editor.setShowLineNumbers(showLineNumbers);
    QString lines;
    for (int i = 0; i < 80; ++i)
        lines += QStringLiteral("line %1: some text\n").arg(i);
    editor.setText(lines);
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QTRY_VERIFY(!editor.painted.isEmpty());
    QTRY_VERIFY(editor.verticalSize() < 1);
    editor.painted.clear();
    editor.sendMessage(SCI_SETSEL, 0, 4);
    QTRY_VERIFY(!editor.painted.isEmpty());
    QVERIFY(
      std::any_of(editor.painted.begin(), editor.painted.end(), [](const QRect& rect) { return rect.height() < 180; }));
    window.grabWindow();
    editor.painted.clear();
    editor.sendMessage(SCI_LINESCROLL, 0, 2);
    QTRY_VERIFY(!editor.painted.isEmpty());
    const QImage scrolled = window.grabWindow();
    editor.painted.clear();
    editor.invalidateImage();
    QTRY_VERIFY(!editor.painted.isEmpty());
    const QImage repainted = window.grabWindow();
    if (repainted != scrolled) {
        const QString output = qEnvironmentVariable("CRAFTWARD_EDITOR_TEST_IMAGE");
        if (!output.isEmpty()) {
            scrolled.save(output + QStringLiteral(".scrolled.png"));
            repainted.save(output + QStringLiteral(".repainted.png"));
        }
        QRect differences;
        for (int y = 0; y < repainted.height(); ++y)
            for (int x = 0; x < repainted.width(); ++x)
                if (repainted.pixel(x, y) != scrolled.pixel(x, y))
                    differences = differences.united(QRect(x, y, 1, 1));
        qWarning() << "Scroll repaint mismatch" << differences << "line height"
                   << editor.sendMessage(SCI_TEXTHEIGHT, 0);
    }
    QCOMPARE(repainted, scrolled);
    editor.setVerticalPosition(1 - editor.verticalSize());
    QTRY_VERIFY(editor.verticalPosition() > 0.5);
}

void
ScintillaQuickTest::publicQmlControl()
{
    QQmlEngine engine;
    QQmlComponent component(&engine);
    component.setData(R"(
        import QtQuick
        import Craftward.Editor
        CodeEditor { width: 360; height: 180; text: "original"; wordWrap: true }
    )",
                      QUrl());
    QScopedPointer<QObject> root(component.create());
    QVERIFY2(root, qPrintable(component.errorString()));
    QQuickWindow window;
    window.resize(360, 180);
    auto* item = qobject_cast<QQuickItem*>(root.data());
    item->setParentItem(window.contentItem());
    window.show();
    QVERIFY(QTest::qWaitForWindowExposed(&window));
    QTest::mouseClick(&window, Qt::LeftButton, {}, QPoint(25, 15));
    QTest::keyClick(&window, Qt::Key_A, Qt::ControlModifier);
    QTest::keyClick(&window, Qt::Key_B);
    QTRY_COMPARE(root->property("text").toString(), QStringLiteral("b"));
    QVERIFY(QMetaObject::invokeMethod(root.data(), "undo"));
    QTRY_COMPARE(root->property("text").toString(), QStringLiteral("original"));
    root->setProperty("readOnly", true);
    QTest::keyClick(&window, Qt::Key_X);
    QCOMPARE(root->property("text").toString(), QStringLiteral("original"));
    item->setParentItem(nullptr);
}

QTEST_MAIN(ScintillaQuickTest)
#include "scintillaquicktest.moc"
