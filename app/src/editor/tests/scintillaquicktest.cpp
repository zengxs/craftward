// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "Scintilla.h"
#include "scintillaeditorbackend.h"

#include <QClipboard>
#include <QDragEnterEvent>
#include <QDropEvent>
#include <QGuiApplication>
#include <QInputMethod>
#include <QInputMethodEvent>
#include <QKeyEvent>
#include <QMimeData>
#include <QQmlComponent>
#include <QQmlEngine>
#include <QQmlExtensionPlugin>
#include <QQuickWindow>
#include <QScopedValueRollback>
#include <QSignalSpy>
#include <QTest>
#include <QTextCharFormat>
#include <QWheelEvent>
#include <QtGui/private/qinputmethod_p.h>

#include <functional>
#include <utility>

Q_IMPORT_QML_PLUGIN(Craftward_EditorPlugin)

class ScintillaQuickTest : public QObject
{
    Q_OBJECT
  private slots:
    void initTestCase();
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
    void horizontalScrollCaretExpansion_data();
    void horizontalScrollCaretExpansion();
    void rendersInsideQuickScene();
    void publicQmlControl();
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
}

void
ScintillaQuickTest::initTestCase()
{
    qmlRegisterType<ScintillaEditorBackend>("Craftward.TestEditor", 1, 0, "Editor");
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
ScintillaQuickTest::partialPaintAndScrollReuse()
{
    QQuickWindow window;
    window.resize(360, 180);
    PaintedEditor editor(window.contentItem());
    editor.setSize(QSizeF(360, 180));
    editor.setReadOnly(true);
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
