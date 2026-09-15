// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "scintillaeditorbackend.h"
#include "scintillaquickadapter_p.h"

#include <QClipboard>
#include <QDragEnterEvent>
#include <QFontDatabase>
#include <QGuiApplication>
#include <QInputMethod>
#include <QMimeData>
#include <QQuickWindow>
#include <QScopedValueRollback>
#include <QThread>
#include <QTimer>
#include <QtMath>

using namespace Scintilla;

namespace {
qintptr
colorValue(const QColor& color)
{
    return quint32(color.red()) | (quint32(color.green()) << 8) | (quint32(color.blue()) << 16) |
           (quint32(color.alpha()) << 24);
}
}

class ScintillaEditorBackendPrivate
{
  public:
    explicit ScintillaEditorBackendPrivate(ScintillaEditorBackend* owner)
      : q(owner)
      , editor(owner)
    {
        editor.notification = [this](const NotificationData& data) {
            if (!updatingText && data.nmhdr.code == Notification::Modified &&
                (int(data.modificationType) & (SC_MOD_INSERTTEXT | SC_MOD_DELETETEXT))) {
                textDirty = true;
                pendingText = true;
            }
            scheduleUpdate();
        };
        editor.scrollChanged = [this] { scheduleUpdate(); };
        editor.host.showMenu = [this](QPointF position, const QVariantList& entries) {
            emit q->contextMenuRequested(position, entries);
        };
    }
    void scheduleUpdate()
    {
        if (updateQueued)
            return;
        updateQueued = true;
        QTimer::singleShot(0, q, [this] {
            updateQueued = false;
            const bool changed = std::exchange(pendingText, false);
            if (changed)
                emit q->textChanged();
            emit q->scrollChanged();
            emit q->editorStateChanged();
            if (q->hasActiveFocus())
                QGuiApplication::inputMethod()->update(Qt::ImQueryInput);
        });
    }
    void initializeEditor()
    {
        editor.WndProc(Message::SetReadOnly, readOnly);
        editor.WndProc(Message::SetWrapMode, wordWrap ? SC_WRAP_WORD : SC_WRAP_NONE);
        editor.WndProc(Message::SetHScrollBar, !wordWrap);
        editor.WndProc(Message::SetScrollWidth, 1);
        editor.WndProc(Message::SetScrollWidthTracking, 1);
        editor.WndProc(Message::SetCaretWidth, readOnly ? 0 : 1);
        for (int margin = 0; margin < 5; ++margin)
            editor.WndProc(Message::SetMarginWidthN, margin, 0);
        scheduleUpdate();
    }
    void applyDefaultStyle()
    {
        const QByteArray family = fontFamily.toUtf8();
        editor.WndProc(Message::StyleSetFont, STYLE_DEFAULT, reinterpret_cast<sptr_t>(family.constData()));
        editor.WndProc(Message::StyleSetSizeFractional, STYLE_DEFAULT, qRound(fontPointSize * SC_FONT_SIZE_MULTIPLIER));
        editor.WndProc(Message::StyleSetWeight, STYLE_DEFAULT, fontWeight);
        editor.WndProc(Message::StyleSetFore, STYLE_DEFAULT, colorValue(foregroundColor));
        editor.WndProc(Message::StyleSetBack, STYLE_DEFAULT, colorValue(backgroundColor));
        editor.WndProc(Message::StyleClearAll);
        editor.WndProc(Message::SetCaretFore, colorValue(foregroundColor));
        editor.WndProc(Message::SetExtraAscent, 0);
        editor.WndProc(Message::SetExtraDescent, 0);
        const int base = editor.WndProc(Message::TextHeight, 0);
        const int extra =
          qMax(0, qCeil(fontPointSize * editor.host.measurementDevice.logicalDpiY() / 72.0 * lineHeightScale) - base);
        editor.WndProc(Message::SetExtraAscent, extra / 2);
        editor.WndProc(Message::SetExtraDescent, extra - extra / 2);
    }
    void applySelectionStyle()
    {
        const int textElements[] = { SC_ELEMENT_SELECTION_TEXT,
                                     SC_ELEMENT_SELECTION_ADDITIONAL_TEXT,
                                     SC_ELEMENT_SELECTION_SECONDARY_TEXT,
                                     SC_ELEMENT_SELECTION_INACTIVE_TEXT,
                                     SC_ELEMENT_SELECTION_INACTIVE_ADDITIONAL_TEXT };
        const int backElements[] = { SC_ELEMENT_SELECTION_BACK,
                                     SC_ELEMENT_SELECTION_ADDITIONAL_BACK,
                                     SC_ELEMENT_SELECTION_SECONDARY_BACK,
                                     SC_ELEMENT_SELECTION_INACTIVE_BACK,
                                     SC_ELEMENT_SELECTION_INACTIVE_ADDITIONAL_BACK };
        for (int element : textElements)
            editor.WndProc(Message::SetElementColour, element, colorValue(selectionForegroundColor));
        for (int element : backElements)
            editor.WndProc(Message::SetElementColour, element, colorValue(selectionBackgroundColor));
        editor.WndProc(Message::SetSelectionLayer, SC_LAYER_BASE);
    }
    ScintillaEditorBackend* q;
    ScintillaQuickAdapter editor;
    mutable QString text;
    mutable bool textDirty = false;
    bool pendingText = false, updateQueued = false, updatingText = false;
    bool readOnly = false, wordWrap = false;
    QString fontFamily = QFontDatabase::systemFont(QFontDatabase::FixedFont).family();
    qreal fontPointSize = 13;
    int fontWeight = SC_WEIGHT_NORMAL;
    qreal lineHeightScale = 1;
    QColor foregroundColor = Qt::black, backgroundColor = Qt::white;
    QColor selectionForegroundColor = Qt::white, selectionBackgroundColor = QColor(0, 122, 255);
};

ScintillaEditorBackend::ScintillaEditorBackend(QQuickItem* parent)
  : ScintillaImageItem(parent)
  , d(std::make_unique<ScintillaEditorBackendPrivate>(this))
{
    setAcceptedMouseButtons(Qt::LeftButton | Qt::RightButton);
    setAcceptHoverEvents(true);
    setActiveFocusOnTab(true);
    setFlag(ItemAcceptsInputMethod);
    setFlag(ItemAcceptsDrops);
    setClip(true);
    d->initializeEditor();
    d->applyDefaultStyle();
    d->applySelectionStyle();
    connect(QGuiApplication::clipboard(), &QClipboard::dataChanged, this, [this] { d->scheduleUpdate(); });
}
ScintillaEditorBackend::~ScintillaEditorBackend() = default;

qintptr
ScintillaEditorBackend::sendMessage(unsigned int message, quintptr wParam, qintptr lParam)
{
    Q_ASSERT(QThread::currentThread() == thread());
    return d->editor.WndProc(static_cast<Message>(message), wParam, lParam);
}
QString
ScintillaEditorBackend::text() const
{
    if (d->textDirty) {
        d->text = d->editor.documentText();
        d->textDirty = false;
    }
    return d->text;
}
void
ScintillaEditorBackend::setText(const QString& value)
{
    if (text() == value)
        return;
    d->editor.cancelComposition();
    const QScopedValueRollback guard(d->updatingText, true);
    d->editor.WndProc(Message::SetReadOnly, 0);
    d->editor.WndProc(Message::ClearAll);
    const QByteArray bytes = value.toUtf8();
    d->editor.WndProc(Message::AddText, bytes.size(), reinterpret_cast<sptr_t>(bytes.constData()));
    d->editor.WndProc(Message::EmptyUndoBuffer);
    d->editor.WndProc(Message::SetSavePoint);
    d->editor.WndProc(Message::SetEmptySelection, 0);
    d->editor.WndProc(Message::SetFirstVisibleLine, 0);
    d->editor.WndProc(Message::SetXOffset, 0);
    d->editor.WndProc(Message::SetReadOnly, d->readOnly);
    d->text = value;
    d->textDirty = d->pendingText = false;
    emit textChanged();
    d->scheduleUpdate();
}
void
ScintillaEditorBackend::revealLocation(int startLine, int endLine)
{
    if (startLine <= 0)
        return;
    const int count = d->editor.WndProc(Message::GetLineCount);
    const int first = qBound(0, startLine - 1, qMax(0, count - 1));
    const int last = qBound(first, endLine > 0 ? endLine - 1 : first, qMax(0, count - 1));
    const auto start = d->editor.WndProc(Message::PositionFromLine, first);
    const auto end = d->editor.WndProc(Message::GetLineEndPosition, last);
    d->editor.WndProc(Message::SetSel, start, end);
    d->editor.WndProc(Message::ScrollCaret);
    d->scheduleUpdate();
}

bool
ScintillaEditorBackend::isReadOnly() const
{
    return d->readOnly;
}

void
ScintillaEditorBackend::setReadOnly(bool readOnly)
{
    if (d->readOnly == readOnly)
        return;

    d->editor.cancelComposition();
    d->readOnly = readOnly;
    d->editor.WndProc(Message::SetReadOnly, readOnly);
    d->editor.WndProc(Message::SetCaretWidth, readOnly ? 0 : 1);
    d->scheduleUpdate();
    emit readOnlyChanged();
    setFlag(ItemAcceptsInputMethod, !readOnly);
    QGuiApplication::inputMethod()->update(Qt::ImEnabled);
}

bool
ScintillaEditorBackend::wordWrap() const
{
    return d->wordWrap;
}

void
ScintillaEditorBackend::setWordWrap(bool wordWrap)
{
    if (d->wordWrap == wordWrap)
        return;

    d->wordWrap = wordWrap;
    d->editor.WndProc(Message::SetWrapMode, wordWrap ? SC_WRAP_WORD : SC_WRAP_NONE);
    d->editor.WndProc(Message::SetHScrollBar, !wordWrap);
    d->scheduleUpdate();
    emit wordWrapChanged();
}

QString
ScintillaEditorBackend::fontFamily() const
{
    return d->fontFamily;
}

void
ScintillaEditorBackend::setFontFamily(const QString& fontFamily)
{
    if (d->fontFamily == fontFamily || fontFamily.isEmpty())
        return;

    d->fontFamily = fontFamily;
    d->applyDefaultStyle();
    emit fontFamilyChanged();
}

qreal
ScintillaEditorBackend::fontPointSize() const
{
    return d->fontPointSize;
}

void
ScintillaEditorBackend::setFontPointSize(qreal fontPointSize)
{
    if (fontPointSize <= 0 || qFuzzyCompare(d->fontPointSize, fontPointSize))
        return;

    d->fontPointSize = fontPointSize;
    d->applyDefaultStyle();
    emit fontPointSizeChanged();
}

int
ScintillaEditorBackend::fontWeight() const
{
    return d->fontWeight;
}

void
ScintillaEditorBackend::setFontWeight(int fontWeight)
{
    if (fontWeight <= 0 || fontWeight > 1000 || d->fontWeight == fontWeight)
        return;

    d->fontWeight = fontWeight;
    d->applyDefaultStyle();
    emit fontWeightChanged();
}

qreal
ScintillaEditorBackend::lineHeightScale() const
{
    return d->lineHeightScale;
}

void
ScintillaEditorBackend::setLineHeightScale(qreal lineHeightScale)
{
    if (lineHeightScale < 1.0 || qFuzzyCompare(d->lineHeightScale, lineHeightScale))
        return;

    d->lineHeightScale = lineHeightScale;
    d->applyDefaultStyle();
    emit lineHeightScaleChanged();
}

QColor
ScintillaEditorBackend::foregroundColor() const
{
    return d->foregroundColor;
}

void
ScintillaEditorBackend::setForegroundColor(const QColor& foregroundColor)
{
    if (!foregroundColor.isValid() || d->foregroundColor == foregroundColor)
        return;

    d->foregroundColor = foregroundColor;
    d->applyDefaultStyle();
    emit foregroundColorChanged();
}

QColor
ScintillaEditorBackend::backgroundColor() const
{
    return d->backgroundColor;
}

void
ScintillaEditorBackend::setBackgroundColor(const QColor& backgroundColor)
{
    if (!backgroundColor.isValid() || d->backgroundColor == backgroundColor)
        return;

    d->backgroundColor = backgroundColor;
    d->applyDefaultStyle();
    emit backgroundColorChanged();
}

QColor
ScintillaEditorBackend::selectionForegroundColor() const
{
    return d->selectionForegroundColor;
}

void
ScintillaEditorBackend::setSelectionForegroundColor(const QColor& selectionForegroundColor)
{
    if (!selectionForegroundColor.isValid() || d->selectionForegroundColor == selectionForegroundColor) {
        return;
    }

    d->selectionForegroundColor = selectionForegroundColor;
    d->applySelectionStyle();
    emit selectionForegroundColorChanged();
}

QColor
ScintillaEditorBackend::selectionBackgroundColor() const
{
    return d->selectionBackgroundColor;
}

void
ScintillaEditorBackend::setSelectionBackgroundColor(const QColor& selectionBackgroundColor)
{
    if (!selectionBackgroundColor.isValid() || d->selectionBackgroundColor == selectionBackgroundColor) {
        return;
    }

    d->selectionBackgroundColor = selectionBackgroundColor;
    d->applySelectionStyle();
    emit selectionBackgroundColorChanged();
}

qreal
ScintillaEditorBackend::verticalSize() const
{
    return qreal(d->editor.verticalPage) / (d->editor.verticalPage + d->editor.verticalMaximum);
}
qreal
ScintillaEditorBackend::verticalPosition() const
{
    return qreal(d->editor.WndProc(Message::GetFirstVisibleLine)) /
           (d->editor.verticalPage + d->editor.verticalMaximum);
}
void
ScintillaEditorBackend::setVerticalPosition(qreal value)
{
    const int line =
      qBound(0, qRound(value * (d->editor.verticalPage + d->editor.verticalMaximum)), d->editor.verticalMaximum);
    d->editor.WndProc(Message::SetFirstVisibleLine, line);
    d->scheduleUpdate();
}
qreal
ScintillaEditorBackend::horizontalSize() const
{
    return qreal(d->editor.horizontalPage) / (d->editor.horizontalPage + d->editor.horizontalMaximum);
}
qreal
ScintillaEditorBackend::horizontalPosition() const
{
    return qreal(d->editor.WndProc(Message::GetXOffset)) / (d->editor.horizontalPage + d->editor.horizontalMaximum);
}
void
ScintillaEditorBackend::setHorizontalPosition(qreal value)
{
    d->editor.setHorizontalPosition(value);
}
bool
ScintillaEditorBackend::canUndo() const
{
    return d->editor.WndProc(Message::CanUndo);
}
bool
ScintillaEditorBackend::canRedo() const
{
    return d->editor.WndProc(Message::CanRedo);
}
bool
ScintillaEditorBackend::hasSelection() const
{
    return !d->editor.WndProc(Message::GetSelectionEmpty);
}
bool
ScintillaEditorBackend::canPaste() const
{
    return d->editor.WndProc(Message::CanPaste);
}
void
ScintillaEditorBackend::undo()
{
    d->editor.cancelComposition();
    d->editor.WndProc(Message::Undo);
}
void
ScintillaEditorBackend::redo()
{
    d->editor.cancelComposition();
    d->editor.WndProc(Message::Redo);
}
void
ScintillaEditorBackend::cut()
{
    d->editor.cancelComposition();
    d->editor.WndProc(Message::Cut);
}
void
ScintillaEditorBackend::copy()
{
    d->editor.WndProc(Message::Copy);
}
void
ScintillaEditorBackend::paste()
{
    d->editor.cancelComposition();
    d->editor.WndProc(Message::Paste);
}
void
ScintillaEditorBackend::selectAll()
{
    d->editor.cancelComposition();
    d->editor.WndProc(Message::SelectAll);
}
void
ScintillaEditorBackend::deleteSelection()
{
    d->editor.cancelComposition();
    d->editor.WndProc(Message::Clear);
}

bool
ScintillaEditorBackend::paintImage(QPainter& painter, const QRect& rect)
{
    return d->editor.paint(painter, rect);
}
void
ScintillaEditorBackend::geometryChange(const QRectF& geometry, const QRectF& oldGeometry)
{
    ScintillaImageItem::geometryChange(geometry, oldGeometry);
    if (d && geometry.size() != oldGeometry.size())
        d->editor.resize();
}
void
ScintillaEditorBackend::itemChange(ItemChange change, const ItemChangeData& data)
{
    ScintillaImageItem::itemChange(change, data);
    if (!d)
        return;
    if (change == ItemDevicePixelRatioHasChanged || change == ItemSceneChange) {
        d->editor.updateMetrics();
        d->applyDefaultStyle();
        d->editor.resize();
    }
    if (change == ItemVisibleHasChanged)
        d->editor.focus(isVisible() && hasActiveFocus());
}
bool
ScintillaEditorBackend::event(QEvent* event)
{
    if (event->type() == QEvent::ShortcutOverride) {
        const auto* key = static_cast<QKeyEvent*>(event);
        if (ScintillaQuickAdapter::shortcutCommand(*key) || key->key() == Qt::Key_Tab ||
            key->key() == Qt::Key_Backtab) {
            event->accept();
            return true;
        }
    }
    if (event->type() == QEvent::KeyPress) {
        auto* key = static_cast<QKeyEvent*>(event);
        if (key->key() == Qt::Key_Tab || key->key() == Qt::Key_Backtab) {
            keyPressEvent(key);
            return key->isAccepted();
        }
    }
    return ScintillaImageItem::event(event);
}
void
ScintillaEditorBackend::keyPressEvent(QKeyEvent* event)
{
    event->setAccepted(d->editor.key(event));
}
void
ScintillaEditorBackend::mousePressEvent(QMouseEvent* event)
{
    QGuiApplication::inputMethod()->commit();
    forceActiveFocus(Qt::MouseFocusReason);
    d->editor.mouse(event);
    event->accept();
}
void
ScintillaEditorBackend::mouseMoveEvent(QMouseEvent* event)
{
    d->editor.mouse(event);
    event->accept();
}
void
ScintillaEditorBackend::mouseReleaseEvent(QMouseEvent* event)
{
    d->editor.mouse(event);
    event->accept();
}
void
ScintillaEditorBackend::mouseDoubleClickEvent(QMouseEvent* event)
{
    mousePressEvent(event);
}
void
ScintillaEditorBackend::mouseUngrabEvent()
{
    d->editor.releaseMouse();
}
void
ScintillaEditorBackend::hoverMoveEvent(QHoverEvent* event)
{
    d->editor.hover(event->position(), event->modifiers());
}
void
ScintillaEditorBackend::hoverLeaveEvent(QHoverEvent*)
{
    d->editor.leave();
}
void
ScintillaEditorBackend::wheelEvent(QWheelEvent* event)
{
    d->editor.wheel(event);
}
void
ScintillaEditorBackend::focusInEvent(QFocusEvent* event)
{
    ScintillaImageItem::focusInEvent(event);
    d->editor.focus(true);
}
void
ScintillaEditorBackend::focusOutEvent(QFocusEvent* event)
{
    QGuiApplication::inputMethod()->commit();
    d->editor.focus(false);
    ScintillaImageItem::focusOutEvent(event);
}
void
ScintillaEditorBackend::inputMethodEvent(QInputMethodEvent* event)
{
    d->editor.inputMethod(event);
}
QVariant
ScintillaEditorBackend::inputMethodQuery(Qt::InputMethodQuery query) const
{
    if (query == Qt::ImFont) {
        QFont font(d->fontFamily);
        font.setPointSizeF(d->fontPointSize);
        return font;
    }
    return d->editor.inputQuery(query);
}
void
ScintillaEditorBackend::dragEnterEvent(QDragEnterEvent* event)
{
    if (!isReadOnly() && event->mimeData()->hasText()) {
        d->editor.dragMove(event->position());
        event->acceptProposedAction();
    }
}
void
ScintillaEditorBackend::dragMoveEvent(QDragMoveEvent* event)
{
    if (!isReadOnly() && event->mimeData()->hasText()) {
        d->editor.dragMove(event->position());
        event->acceptProposedAction();
    }
}
void
ScintillaEditorBackend::dragLeaveEvent(QDragLeaveEvent* event)
{
    d->editor.dragLeave();
    event->accept();
}
void
ScintillaEditorBackend::dropEvent(QDropEvent* event)
{
    if (!isReadOnly() && event->mimeData()->hasText()) {
        d->editor.drop(
          event->position(), event->mimeData(), event->source() == this && event->proposedAction() == Qt::MoveAction);
        event->acceptProposedAction();
    }
}
