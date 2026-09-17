// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "scintillaquickadapter_p.h"

#include <QClipboard>
#include <QDrag>
#include <QGuiApplication>
#include <QInputMethod>
#include <QKeySequence>
#include <QList>
#include <QMimeData>
#include <QQuickWindow>
#include <QScopedValueRollback>
#include <QScreen>
#include <QStyleHints>
#include <QTextFormat>
#include <QTimerEvent>
#include <QWheelEvent>

using namespace Scintilla;
using namespace Scintilla::Internal;

namespace {
constexpr auto rectangularMime = "text/x-scintilla.utf16-plain-text.rectangular";
constexpr int imeTextIndicator = INDIC_IME;
constexpr int imeBackgroundIndicator = INDIC_IME + 1;
constexpr int imeUnderlineIndicator = INDIC_IME + 2;

QList<ScintillaQuickAdapter*>&
liveEditors()
{
    // Editors and document attachment changes are confined to the GUI thread.
    static QList<ScintillaQuickAdapter*> editors;
    return editors;
}

QColor
compositeColour(const QColor& foreground, const QColor& background)
{
    const int alpha = foreground.alpha();
    const auto channel = [alpha](int fore, int back) { return (fore * alpha + back * (255 - alpha) + 127) / 255; };
    return QColor(channel(foreground.red(), background.red()),
                  channel(foreground.green(), background.green()),
                  channel(foreground.blue(), background.blue()));
}

int
indicatorColour(const QColor& colour)
{
    // Indicator values carry RGB; the extra bit keeps black distinct from an unset range.
    return SC_INDICVALUEBIT | colour.red() | (colour.green() << 8) | (colour.blue() << 16);
}

KeyMod
modifiers(Qt::KeyboardModifiers value)
{
    int flags = 0;
    if (value & Qt::ShiftModifier)
        flags |= SCMOD_SHIFT;
    if (value & Qt::ControlModifier)
        flags |= SCMOD_CTRL;
    if (value & Qt::AltModifier)
        flags |= SCMOD_ALT;
    if (value & Qt::MetaModifier)
        flags |= SCMOD_META;
    return static_cast<KeyMod>(flags);
}
}

ScintillaQuickAdapter::ScintillaQuickAdapter(ScintillaImageItem* owner)
  : QObject(owner)
{
    host.item = owner;
    wMain = &host;
    clock.start();
    imeInteraction = IMEInteraction::Inline;
    view.bufferedDraw = false;
    vs.marginNumberPadding = lineNumberPadding;
    widthTimer.setSingleShot(true);
    connect(&widthTimer, &QTimer::timeout, this, &ScintillaQuickAdapter::updateHorizontalExtent);
    updateMetrics();
    WndProc(Message::SetCodePage, SC_CP_UTF8);
    WndProc(Message::SetLayoutCache, SC_CACHE_PAGE);
    WndProc(Message::SetMultipleSelection, 1);
    WndProc(Message::SetAdditionalSelectionTyping, 1);
    WndProc(Message::SetMultiPaste, SC_MULTIPASTE_EACH);
    const std::pair<int, int> imeIndicators[] = { { imeTextIndicator, INDIC_TEXTFORE },
                                                  { imeBackgroundIndicator, INDIC_FULLBOX },
                                                  { imeUnderlineIndicator, INDIC_PLAIN } };
    for (const auto& [indicator, style] : imeIndicators) {
        WndProc(Message::IndicSetStyle, indicator, style);
        WndProc(Message::IndicSetFlags, indicator, SC_INDICFLAG_VALUEFORE);
    }
    WndProc(Message::IndicSetUnder, imeBackgroundIndicator, 1);
    WndProc(Message::IndicSetAlpha, imeBackgroundIndicator, 255);
    WndProc(Message::IndicSetOutlineAlpha, imeBackgroundIndicator, 255);
    idleTimer.setInterval(0);
    connect(&idleTimer, &QTimer::timeout, this, [this] {
        if (!Idle())
            idleTimer.stop();
        queueUpdate();
    });
#ifdef Q_OS_MACOS
    // Qt's ControlModifier represents Command on macOS.
    kmap.AssignCmdKey(Keys::Left, KeyMod::Ctrl, Message::VCHome);
    kmap.AssignCmdKey(Keys::Right, KeyMod::Ctrl, Message::LineEnd);
    kmap.AssignCmdKey(Keys::Up, KeyMod::Ctrl, Message::DocumentStart);
    kmap.AssignCmdKey(Keys::Down, KeyMod::Ctrl, Message::DocumentEnd);
    kmap.AssignCmdKey(Keys::Left, KeyMod::Ctrl | KeyMod::Shift, Message::VCHomeExtend);
    kmap.AssignCmdKey(Keys::Right, KeyMod::Ctrl | KeyMod::Shift, Message::LineEndExtend);
    kmap.AssignCmdKey(Keys::Up, KeyMod::Ctrl | KeyMod::Shift, Message::DocumentStartExtend);
    kmap.AssignCmdKey(Keys::Down, KeyMod::Ctrl | KeyMod::Shift, Message::DocumentEndExtend);
    kmap.AssignCmdKey(Keys::Left, KeyMod::Alt, Message::WordLeft);
    kmap.AssignCmdKey(Keys::Right, KeyMod::Alt, Message::WordRight);
    kmap.AssignCmdKey(Keys::Left, KeyMod::Alt | KeyMod::Shift, Message::WordLeftExtend);
    kmap.AssignCmdKey(Keys::Right, KeyMod::Alt | KeyMod::Shift, Message::WordRightExtend);
    kmap.AssignCmdKey(Keys::Back, KeyMod::Alt, Message::DelWordLeft);
#endif
    liveEditors().append(this);
}

ScintillaQuickAdapter::~ScintillaQuickAdapter()
{
    notification = {};
    scrollChanged = {};
    documentChanged = {};
    textModified = {};
    compositionChanged = {};
    liveEditors().removeOne(this);
    ScintillaBase::Finalise();
    for (int timer : timers)
        if (timer)
            killTimer(timer);
    wMain = nullptr;
}

sptr_t
ScintillaQuickAdapter::WndProc(Message message, uptr_t wParam, sptr_t lParam)
{
    try {
        if (message == Message::SetDocPointer && lParam) {
            for (const auto* editor : liveEditors()) {
                if (reinterpret_cast<sptr_t>(editor->pdoc->AsDocumentEditable()) != lParam)
                    continue;
                // Style IDs are view-local, but style bytes belong to the document.
                // Reject another owner before Scintilla changes either view. Reattaching
                // our own document is a no-op, without releasing its only reference.
                if (editor != this)
                    errorStatus = Status::Failure;
                return 0;
            }
        }
        if (message == Message::GrabFocus) {
            host.item->forceActiveFocus();
            return 0;
        }
        switch (message) {
            case Message::SetFirstVisibleLine:
                RefreshStyleData();
                setVerticalOffset(qreal(static_cast<sptr_t>(wParam)) * vs.lineHeight);
                return 0;
            case Message::LineScroll:
                RefreshStyleData();
                setVerticalOffset(verticalOffset() + qreal(lParam) * vs.lineHeight);
                HorizontalScrollTo(xOffset + static_cast<int>(static_cast<int>(wParam) * vs.spaceWidth));
                return 1;
            case Message::PointYFromPosition: {
                if (lParam < 0)
                    return 0;
                const Point point = LocationFromPosition(lParam);
                return static_cast<sptr_t>(std::floor(host.toItem(point).y()));
            }
            case Message::PositionFromPoint:
            case Message::PositionFromPointClose:
            case Message::CharPositionFromPoint:
            case Message::CharPositionFromPointClose: {
                const bool close =
                  message == Message::PositionFromPointClose || message == Message::CharPositionFromPointClose;
                const bool character =
                  message == Message::CharPositionFromPoint || message == Message::CharPositionFromPointClose;
                return PositionFromLocation(
                  contentPoint(QPointF(static_cast<sptr_t>(wParam), lParam)), close, character);
            }
            case Message::SelectionFromPoint:
                return SelectionFromPoint(contentPoint(QPointF(static_cast<sptr_t>(wParam), lParam)));
            case Message::MoveCaretInsideView:
                moveCaretInsideViewport(true);
                return 0;
            case Message::ScrollVertical:
                RefreshStyleData();
                setVerticalOffset(qreal(topLine) * vs.lineHeight);
                break;
            default:
                break;
        }
        const bool stylesWereValid = stylesValid;
        const auto result = ScintillaBase::WndProc(message, wParam, lParam);
        if ((stylesWereValid && !stylesValid) || message == Message::SetScrollWidthTracking ||
            message == Message::SetTabWidth || message == Message::SetCodePage || message == Message::SetWrapMode)
            invalidateHorizontalExtent();
        if (message == Message::SetDocPointer) {
            setVerticalOffset(0);
            resetHorizontalExtent();
            if (documentChanged)
                documentChanged();
        }
        // Commands that reveal the caret must also expose a clipped part of its row.
        // Passive selection setters intentionally leave the viewport unchanged.
        switch (message) {
            case Message::ScrollCaret:
            case Message::GotoPos:
            case Message::GotoLine:
            case Message::SetSel:
            case Message::Paste:
            case Message::Cut:
            case Message::Clear:
            case Message::ReplaceSel:
            case Message::Undo:
            case Message::Redo:
                revealPosition(sel.RangeMain().caret);
                break;
            case Message::ScrollRange:
                revealPosition(SelectionPosition(wParam));
                break;
            default:
                break;
        }
        // Some selection messages set the pending notification without queuing idle work or a repaint.
        if (FlagSet(needUpdateUI, Update::Selection))
            queueUpdate();
        return result;
    } catch (const std::bad_alloc&) {
        errorStatus = Status::BadAlloc;
    } catch (const Failure& failure) {
        errorStatus = failure.status;
    } catch (...) {
        errorStatus = Status::Failure;
    }
    return 0;
}

bool
ScintillaQuickAdapter::paint(QPainter& painter, const QRect& rect)
{
    RefreshStyleData();
    synchronizeVerticalScroll();
    const qreal offset = host.scrollOffsetY;
    painter.save();
    painter.translate(0, -offset);
    rcPaint = PRectFromQRect(rect);
    rcPaint.Move(0, offset);
    paintState = PaintState::painting;
    paintingAllText = rcPaint.Contains(GetClientRectangle());
    AutoSurface surface(painter.device(), this);
    // A row bordering the dirty area can still contribute antialiased pixels
    // after translation. Draw its edge too, retaining the caller's damage clip.
    PRectangle area = rcPaint;
    const qreal edge = 1.0 / painter.device()->devicePixelRatioF();
    area.top -= edge;
    area.bottom += edge;
    Paint(surface, area);
    const bool complete = paintState != PaintState::abandoned && offset == host.scrollOffsetY;
    paintState = PaintState::notPainting;
    painter.restore();
    queueUpdate();
    return complete;
}

qsizetype
ScintillaQuickAdapter::visibleTextEnd()
{
    RefreshStyleData();
    WrapLines(WrapScope::wsVisible);
    synchronizeVerticalScroll();
    const PRectangle viewport = GetClientRectangle();
    if (viewport.Width() <= 0 || viewport.Height() <= 0)
        return pdoc->Length();
    // Include the character at the lower-right edge, including a partial row.
    // Hit testing uses the current styles, wrapping, and horizontal offset.
    const Sci::Position position = PositionFromLocation(Point(viewport.right, viewport.bottom), false, true);
    return position >= pdoc->Length() ? pdoc->Length() : WndProc(Message::PositionRelative, position, 1);
}

void
ScintillaQuickAdapter::updateMetrics()
{
    if (host.item && host.item->window()) {
        host.devicePixelRatio = host.item->window()->effectiveDevicePixelRatio();
        if (const auto* screen = host.item->window()->screen()) {
            host.measurementDevice.setDotsPerMeterX(qRound(screen->logicalDotsPerInchX() / 0.0254));
            host.measurementDevice.setDotsPerMeterY(qRound(screen->logicalDotsPerInchY() / 0.0254));
        }
    }
    InvalidateStyleRedraw();
    invalidateHorizontalExtent();
}

void
ScintillaQuickAdapter::invalidateHorizontalExtent()
{
    lineWidths.assign(size_t(pdoc->LinesTotal()), -1);
    measuredLineWidths.clear();
    nextWidthLine = 0;
    widthEndLine = pdoc->LinesTotal();
    if (trackLineWidth)
        widthTimer.start(0);
}

void
ScintillaQuickAdapter::resetHorizontalExtent()
{
    invalidateHorizontalExtent();
    if (trackLineWidth) {
        view.lineWidthMaxSeen = 0;
        WndProc(Message::SetScrollWidth, 1);
    }
}

void
ScintillaQuickAdapter::invalidateLineWidths(Sci::Line first, Sci::Line last)
{
    first = std::max<Sci::Line>(0, first);
    last = std::min<Sci::Line>(last, Sci::Line(lineWidths.size()) - 1);
    for (Sci::Line line = first; line <= last; ++line) {
        int& width = lineWidths[size_t(line)];
        if (width >= 0) {
            const auto count = measuredLineWidths.find(width);
            if (--count->second == 0)
                measuredLineWidths.erase(count);
        }
        width = -1;
    }
    if (nextWidthLine >= widthEndLine) {
        nextWidthLine = first;
        widthEndLine = last + 1;
    } else {
        nextWidthLine = std::min(nextWidthLine, first);
        widthEndLine = std::max(widthEndLine, last + 1);
    }
    if (trackLineWidth)
        widthTimer.start(0);
}

void
ScintillaQuickAdapter::updateHorizontalExtent()
{
    if (!trackLineWidth || Wrapping())
        return;
    RefreshStyleData();
    // Virtual space belongs to the live selections, not the cached document lines.
    int virtualSpaceWidth = 0;
    for (size_t i = 0; i < sel.Count(); ++i) {
        const auto& range = sel.Range(i);
        for (const auto position : { range.caret, range.anchor }) {
            if (position.VirtualSpace())
                virtualSpaceWidth = std::max(virtualSpaceWidth, XFromPosition(position) + qCeil(vs.aveCharWidth));
        }
    }
    if (nextWidthLine >= widthEndLine && virtualSpaceWidth == lastVirtualSpaceWidth)
        return;
    AutoSurface surface(this);
    if (!surface)
        return;
    lastVirtualSpaceWidth = virtualSpaceWidth;
    QElapsedTimer budget;
    budget.start();
    // Keep initial measurement and font changes out of a single long GUI-thread scan.
    while (nextWidthLine < widthEndLine && budget.elapsed() < 2) {
        const Sci::Line line = nextWidthLine++;
        int& width = lineWidths[size_t(line)];
        if (width >= 0)
            continue;
        // Background measurement must not evict the visible page's layout cache.
        LineLayout layout(line, int(pdoc->LineStart(line + 1) - pdoc->LineStart(line)));
        view.LayoutLine(*this, surface, vs, &layout, LineLayout::wrapWidthInfinite);
        width = qCeil(layout.positions[layout.numCharsInLine]);
        ++measuredLineWidths[width];
    }
    // Leave room for the caret at the end of the widest line.
    const int textWidth = measuredLineWidths.empty() ? 1 : measuredLineWidths.rbegin()->first + qCeil(vs.aveCharWidth);
    const int maximum = std::max({ 1, textWidth, virtualSpaceWidth });
    const bool complete = nextWidthLine >= widthEndLine;
    // Do not shrink the range while a wider line is still waiting to be measured.
    if (complete || maximum > scrollWidth) {
        view.lineWidthMaxSeen = maximum;
        const int oldOffset = xOffset;
        WndProc(Message::SetScrollWidth, maximum);
        if (xOffset != oldOffset)
            Redraw();
        if (scrollChanged)
            scrollChanged();
    }
    if (!complete)
        widthTimer.start(0);
}
void
ScintillaQuickAdapter::resize()
{
    ChangeSize();
    queueUpdate();
}
void
ScintillaQuickAdapter::focus(bool on)
{
    if (!on) {
        cancelComposition();
        CancelModes();
        captured = false;
    }
    SetFocusState(on);
    queueUpdate();
}
unsigned int
ScintillaQuickAdapter::timestamp() const
{
    return clock.elapsed() % 2'000'000'000;
}

void
ScintillaQuickAdapter::insertQString(const QString& text, CharacterSource source)
{
    for (qsizetype i = 0; i < text.size();) {
        const qsizetype units =
          text[i].isHighSurrogate() && i + 1 < text.size() && text[i + 1].isLowSurrogate() ? 2 : 1;
        const QByteArray bytes = text.mid(i, units).toUtf8();
        InsertCharacter(std::string_view(bytes.constData(), bytes.size()), source);
        i += units;
    }
}

std::optional<Message>
ScintillaQuickAdapter::shortcutCommand(const QKeyEvent& event)
{
    const std::pair<QKeySequence::StandardKey, Message> shortcuts[] = {
        { QKeySequence::Undo, Message::Undo },   { QKeySequence::Redo, Message::Redo },
        { QKeySequence::Cut, Message::Cut },     { QKeySequence::Copy, Message::Copy },
        { QKeySequence::Paste, Message::Paste }, { QKeySequence::SelectAll, Message::SelectAll }
    };
    for (const auto& [sequence, command] : shortcuts) {
        if (event.matches(sequence))
            return command;
    }
    return std::nullopt;
}

bool
ScintillaQuickAdapter::key(QKeyEvent* event)
{
    if (event->key() == Qt::Key_Shift || event->key() == Qt::Key_Control || event->key() == Qt::Key_Alt ||
        event->key() == Qt::Key_Meta)
        return false;
    cancelComposition();
    if (const auto command = shortcutCommand(*event)) {
        WndProc(*command);
        queueUpdate();
        return true;
    }
    int key = event->key();
    switch (key) {
        case Qt::Key_Down:
            key = SCK_DOWN;
            break;
        case Qt::Key_Up:
            key = SCK_UP;
            break;
        case Qt::Key_Left:
            key = SCK_LEFT;
            break;
        case Qt::Key_Right:
            key = SCK_RIGHT;
            break;
        case Qt::Key_Home:
            key = SCK_HOME;
            break;
        case Qt::Key_End:
            key = SCK_END;
            break;
        case Qt::Key_PageUp:
            key = SCK_PRIOR;
            break;
        case Qt::Key_PageDown:
            key = SCK_NEXT;
            break;
        case Qt::Key_Delete:
            key = SCK_DELETE;
            break;
        case Qt::Key_Backspace:
            key = SCK_BACK;
            break;
        case Qt::Key_Escape:
            key = SCK_ESCAPE;
            break;
        case Qt::Key_Backtab:
        case Qt::Key_Tab:
            key = SCK_TAB;
            break;
        case Qt::Key_Enter:
        case Qt::Key_Return:
            key = SCK_RETURN;
            break;
        default:
            break;
    }
    bool consumed = false;
    KeyDownWithModifiers(static_cast<Keys>(key), modifiers(event->modifiers()), &consumed);
    if (!consumed && !event->text().isEmpty() &&
        (event->text().front().isPrint() || event->text().front().isHighSurrogate()) &&
        !(event->modifiers() & (Qt::ControlModifier | Qt::MetaModifier))) {
        insertQString(event->text(), CharacterSource::DirectInput);
        revealPosition(sel.RangeMain().caret);
        consumed = true;
    }
    queueUpdate();
    return consumed;
}

void
ScintillaQuickAdapter::mouse(QMouseEvent* event)
{
    const KeyMod mods = modifiers(event->modifiers());
    if (event->type() == QEvent::MouseMove) {
        ButtonMoveWithModifiers(contentPoint(event->position()), timestamp(), mods);
    } else if (event->button() == Qt::LeftButton) {
        if (event->type() == QEvent::MouseButtonRelease)
            ButtonUpWithModifiers(contentPoint(event->position()), timestamp(), mods);
        else {
            cancelComposition();
            ButtonDownWithModifiers(contentPoint(event->position()), timestamp(), mods);
        }
    } else if (event->button() == Qt::RightButton && event->type() == QEvent::MouseButtonPress) {
        cancelComposition();
        RightButtonDownWithModifiers(contentPoint(event->position()), timestamp(), mods);
        ContextMenu(contentPoint(event->position()));
    }
    if (event->button() == Qt::LeftButton)
        revealPosition(sel.RangeMain().caret);
    else
        finishSelectionScroll();
    queueUpdate();
}
void
ScintillaQuickAdapter::hover(QPointF position, Qt::KeyboardModifiers mods)
{
    ButtonMoveWithModifiers(contentPoint(position), timestamp(), modifiers(mods));
}
void
ScintillaQuickAdapter::leave()
{
    MouseLeave();
}
void
ScintillaQuickAdapter::releaseMouse()
{
    SetMouseCapture(false);
    FineTickerCancel(TickReason::scroll);
}
void
ScintillaQuickAdapter::wheel(QWheelEvent* event)
{
    RefreshStyleData();
    synchronizeVerticalScroll();
    const QPoint pixels = event->pixelDelta();
    if (!pixels.isNull()) {
        setVerticalOffset(verticalOffset() - pixels.y());
        if (!Wrapping())
            HorizontalScrollTo(xOffset - pixels.x());
    } else {
        const QPoint delta = event->angleDelta();
        const qreal lines = -qreal(delta.y()) * QGuiApplication::styleHints()->wheelScrollLines() / 120;
        if (event->modifiers() & Qt::ShiftModifier) {
            if (!Wrapping())
                HorizontalScrollTo(xOffset + qRound(lines * vs.aveCharWidth));
        } else {
            setVerticalOffset(verticalOffset() + lines * vs.lineHeight);
            if (!Wrapping())
                HorizontalScrollTo(xOffset - delta.x() * int(vs.aveCharWidth) / 120);
        }
    }
    event->accept();
    queueUpdate();
}

PRectangle
ScintillaQuickAdapter::GetClientRectangle() const
{
    PRectangle rect = Editor::GetClientRectangle();
    rect.Move(0, host.scrollOffsetY);
    return rect;
}

Point
ScintillaQuickAdapter::contentPoint(QPointF point)
{
    RefreshStyleData();
    return host.toContent(point);
}

qreal
ScintillaQuickAdapter::verticalOffset() const
{
    return qreal(topLine) * qMax(1, vs.lineHeight) + host.scrollOffsetY;
}

bool
ScintillaQuickAdapter::moveVerticalOffset(qreal position, bool reuseImage)
{
    qreal next = qBound(0.0, position, verticalMaximum);
    const int height = qMax(1, vs.lineHeight);
    const qreal aligned = std::round(next / height) * height;
    if (qFuzzyCompare(next, aligned))
        next = qMin(aligned, verticalMaximum);
    const auto line = static_cast<Sci::Line>(std::floor(next / height));
    const qreal offset = next - qreal(line) * height;
    if (line == topLine && qFuzzyIsNull(offset - host.scrollOffsetY))
        return false;
    const qreal previous = verticalOffset();
    // Timed selection dragging reuses these points without another Qt mouse event.
    if (ptMouseLast.x != -1 || ptMouseLast.y != -1)
        ptMouseLast.y += offset - host.scrollOffsetY;
    lastClick.y += offset - host.scrollOffsetY;
    host.scrollOffsetY = offset;
    SetTopLine(line);
    ContainerNeedsUpdate(Update::VScroll);
    if (paintState != PaintState::notPainting)
        paintState = PaintState::abandoned;
    if (reuseImage)
        host.item->scrollImage(previous - next);
    else
        host.item->invalidateImage();
    return true;
}

bool
ScintillaQuickAdapter::synchronizeVerticalScroll()
{
    const int height = qMax(1, vs.lineHeight);
    const qreal page = qMax(1.0, Editor::GetClientRectangle().Height());
    const qreal maximum = qMax(0.0, qreal(pcs->LinesDisplayed()) * height - (endAtLastLine ? page : height));
    const bool changed = page != verticalPage || maximum != verticalMaximum || height != scrollLineHeight;
    verticalPage = page;
    verticalMaximum = maximum;
    // Preserve the top display row and the fraction of it hidden by a font/zoom change.
    const qreal position = qreal(topLine) * height + host.scrollOffsetY * height / scrollLineHeight;
    scrollLineHeight = height;
    const bool moved = moveVerticalOffset(position, false);
    if (moved)
        finishVerticalScroll();
    else if (changed && scrollChanged)
        scrollChanged();
    return changed || moved;
}

void
ScintillaQuickAdapter::finishVerticalScroll()
{
    // Popups refer to a document position; dismiss them when their viewport moves.
    AutoCompleteCancel();
    ct.CallTipCancel();
    if (scrollChanged)
        scrollChanged();
    queueUpdate();
}

void
ScintillaQuickAdapter::setVerticalOffset(qreal position)
{
    if (!std::isfinite(position))
        return;
    RefreshStyleData();
    synchronizeVerticalScroll();
    if (moveVerticalOffset(position, true)) {
        StyleAreaBounded(GetClientRectangle(), true);
        Editor::SetVerticalScrollPos();
        finishVerticalScroll();
    }
}

void
ScintillaQuickAdapter::setVerticalPosition(qreal position)
{
    if (!std::isfinite(position))
        return;
    RefreshStyleData();
    synchronizeVerticalScroll();
    setVerticalOffset(qBound(0.0, position, 1.0) * (verticalMaximum + verticalPage));
}

void
ScintillaQuickAdapter::moveCaretInsideViewport(bool scrollHorizontally)
{
    RefreshStyleData();
    synchronizeVerticalScroll();
    const PRectangle viewport = GetClientRectangle();
    const qreal height = vs.lineHeight;
    qreal firstRowY = std::ceil(viewport.top / height) * height;
    qreal lastRowY = (std::floor(viewport.bottom / height) - 1) * height;
    if (lastRowY < firstRowY) {
        // A very short viewport may contain only partial rows. Keep its position
        // and use the row at its centre instead of trying to reveal a whole row.
        firstRowY = lastRowY = std::floor((viewport.top + viewport.bottom) / (2 * height)) * height;
    }
    const Point caret = PointMainCaret();
    const qreal y = qBound(firstRowY, caret.y, lastRowY);
    if (y != caret.y) {
        SelectionPosition position =
          SPositionFromLocation(Point(lastXChosen - xOffset, y), false, false, UserVirtualSpace());
        // The end of a wrapped row belongs to the next row for caret placement.
        if (Wrapping() && position.Position() > 0 && LocationFromPosition(position).y > y)
            position = SelectionPosition(pdoc->MovePositionOutsideChar(position.Position() - 1, -1));
        MovePositionTo(position, Selection::SelTypes::none, false);
        if (scrollHorizontally)
            EnsureCaretVisible(true, false, true);
    }
}

void
ScintillaQuickAdapter::revealPosition(SelectionPosition position)
{
    RefreshStyleData();
    synchronizeVerticalScroll();
    const qreal y = LocationFromPosition(position).y - host.scrollOffsetY;
    // The core handles distant positions and caret policies. Complete its row-based
    // scrolling only when the target row is still clipped at a viewport edge.
    if (y < 0 && y + vs.lineHeight > 0)
        setVerticalOffset(verticalOffset() + y);
    else if (y >= 0 && y < verticalPage && y + vs.lineHeight > verticalPage)
        setVerticalOffset(verticalOffset() + y + qMin(qreal(vs.lineHeight), verticalPage) - verticalPage);
}

int
ScintillaQuickAdapter::KeyCommand(Message message)
{
    if (message == Message::LineScrollUp || message == Message::LineScrollDown) {
        RefreshStyleData();
        setVerticalOffset(verticalOffset() + (message == Message::LineScrollUp ? -vs.lineHeight : vs.lineHeight));
        moveCaretInsideViewport(false);
        return 0;
    }
    const SelectionPosition previous = sel.RangeMain().caret;
    const int result = ScintillaBase::KeyCommand(message);
    switch (message) {
        case Message::ScrollToStart:
        case Message::ScrollToEnd:
            // Absolute viewport commands use pixel endpoints and leave the caret in place.
            RefreshStyleData();
            synchronizeVerticalScroll();
            setVerticalOffset(message == Message::ScrollToStart ? 0 : verticalMaximum);
            break;
        case Message::Cancel:
        case Message::LineCopy:
            // Cancelling modes and copying preserve the viewport even when the caret row is clipped.
            break;
        case Message::ZoomIn:
        case Message::ZoomOut:
            // WndProc must observe the invalid styles before refreshing font metrics.
            // The refresh preserves the viewport's row fraction without revealing the caret.
            break;
        default:
            if (!ac.Active() || sel.RangeMain().caret != previous)
                revealPosition(sel.RangeMain().caret);
            break;
    }
    return result;
}

void
ScintillaQuickAdapter::finishSelectionScroll()
{
    const qreal y = host.toItem(ptMouseLast).y();
    if (HaveMouseCapture() && (y < 0 || y >= verticalPage))
        revealPosition(posDrag.IsValid() ? posDrag : sel.RangeMain().caret);
}

bool
ScintillaQuickAdapter::composing() const
{
    return pdoc->TentativeActive();
}
QString
ScintillaQuickAdapter::documentText() const
{
    if (composing())
        return compositionText;
    const std::string bytes = RangeText(0, pdoc->Length());
    return QString::fromUtf8(bytes.data(), bytes.size());
}
QString
ScintillaQuickAdapter::selectionText() const
{
    const std::string bytes = RangeText(sel.RangeMain().Start().Position(), sel.RangeMain().End().Position());
    return QString::fromUtf8(bytes.data(), bytes.size());
}
void
ScintillaQuickAdapter::cancelComposition()
{
    if (!composing())
        return;
    const QScopedValueRollback guard(tentativeUpdate, true);
    pdoc->TentativeUndo();
    if (compositionChanged)
        compositionChanged(false);
    SetSelectionFromSerialized(compositionSelection.c_str());
    preeditPosition = -1;
    view.imeCaretBlockOverride = false;
    Redraw();
    queueUpdate();
}

void
ScintillaQuickAdapter::applyPreeditFormats(const QInputMethodEvent& event)
{
    const QString& preedit = event.preeditString();
    QTextCharFormat defaultFormat;
    defaultFormat.setFontUnderline(true);
    QList<QTextCharFormat> formats(preedit.size(), defaultFormat);
    for (const auto& attribute : event.attributes()) {
        if (attribute.type != QInputMethodEvent::TextFormat)
            continue;
        const qsizetype start = qBound(qsizetype(0), qsizetype(attribute.start), preedit.size());
        const qsizetype end = qBound(start, qsizetype(attribute.start) + attribute.length, preedit.size());
        const auto format = attribute.value.value<QTextFormat>().toCharFormat();
        std::fill(formats.begin() + start, formats.begin() + end, format);
    }

    const int previousIndicator = pdoc->decorations->GetCurrentIndicator();
    const qsizetype byteLength = preedit.toUtf8().size();
    for (size_t r = 0; r < sel.Count(); ++r) {
        Sci::Position position = sel.Range(r).Start().Position() - byteLength;
        for (qsizetype i = 0; i < preedit.size();) {
            const qsizetype units =
              preedit[i].isHighSurrogate() && i + 1 < preedit.size() && preedit[i + 1].isLowSurrogate() ? 2 : 1;
            const qsizetype bytes = preedit.mid(i, units).toUtf8().size();
            const auto& format = formats[i];
            const auto& style = vs.styles[pdoc->StyleIndexAt(position)];
            QColor background = QColorFromColourRGBA(style.back);
            QColor foreground = QColorFromColourRGBA(style.fore);
            const auto fill = [&](int indicator, const QColor& colour) {
                pdoc->DecorationSetCurrentIndicator(indicator);
                pdoc->DecorationFillRange(position, indicatorColour(colour), bytes);
            };
            // Scintilla's per-range indicator colours are opaque RGB, so resolve alpha here.
            if (format.background().style() != Qt::NoBrush) {
                background = compositeColour(format.background().color(), background);
                fill(imeBackgroundIndicator, background);
            }
            if (format.foreground().style() != Qt::NoBrush) {
                foreground = compositeColour(format.foreground().color(), background);
                fill(imeTextIndicator, foreground);
            }
            if (format.underlineStyle() != QTextCharFormat::NoUnderline) {
                const QColor underline = format.underlineColor().isValid() ? format.underlineColor() : foreground;
                fill(imeUnderlineIndicator, compositeColour(underline, background));
            }
            position += bytes;
            i += units;
        }
    }
    pdoc->DecorationSetCurrentIndicator(previousIndicator);
}

void
ScintillaQuickAdapter::inputMethod(QInputMethodEvent* event)
{
    if (pdoc->IsReadOnly() || SelectionContainsProtected()) {
        event->ignore();
        return;
    }
    const bool wasComposing = composing();
    const bool commits = !event->commitString().isEmpty() || event->replacementLength();
    cancelComposition();
    if (commits) {
        UndoGroup undo(pdoc);
        ClearSelection();
        if (event->replacementStart() || event->replacementLength()) {
            std::vector<SelectionRange*> ranges;
            for (size_t r = 0; r < sel.Count(); ++r)
                ranges.push_back(&sel.Range(r));
            std::sort(ranges.begin(), ranges.end(), [](const auto* left, const auto* right) { return *left < *right; });
            // Match Scintilla's typing order so edits do not shift ranges still to be processed.
            for (auto it = ranges.rbegin(); it != ranges.rend(); ++it) {
                auto& range = **it;
                const auto start = pdoc->GetRelativePositionUTF16(range.caret.Position(), event->replacementStart());
                const auto end = pdoc->GetRelativePositionUTF16(start, event->replacementLength());
                if (start >= 0 && end >= start && end <= pdoc->Length()) {
                    pdoc->DeleteChars(start, end - start);
                    range = SelectionRange(start);
                }
            }
            sel.RemoveDuplicates();
        }
        insertQString(event->commitString(), CharacterSource::DirectInput);
    }
    if (!event->preeditString().isEmpty()) {
        const QScopedValueRollback guard(tentativeUpdate, true);
        if (!wasComposing || commits)
            compositionText = documentText();
        compositionSelection = sel.ToString();
        pdoc->TentativeStart();
        if (compositionChanged)
            compositionChanged(true);
        ClearBeforeTentativeStart();
        preeditPosition = CurrentPosition();
        if (!wasComposing || commits)
            compositionCursor = pdoc->CountUTF16(0, preeditPosition);
        insertQString(event->preeditString(), CharacterSource::TentativeInput);
        applyPreeditFormats(*event);
        int cursor = event->preeditString().size();
        for (const auto& attribute : event->attributes()) {
            if (attribute.type == QInputMethodEvent::Cursor)
                cursor = qBound(0, attribute.start, int(event->preeditString().size()));
        }
        const auto position = pdoc->GetRelativePositionUTF16(CurrentPosition(), cursor - event->preeditString().size());
        MoveImeCarets(position - CurrentPosition());
        EnsureCaretVisible();
    }
    ShowCaretAtCurrentPosition();
    revealPosition(sel.RangeMain().caret);
    queueUpdate();
    event->accept();
}

QVariant
ScintillaQuickAdapter::inputQuery(Qt::InputMethodQuery query)
{
    const auto position = preeditPosition >= 0 ? preeditPosition : CurrentPosition();
    if (query == Qt::ImEnabled)
        return !pdoc->IsReadOnly();
    if (query == Qt::ImHints)
        return int(Qt::ImhNoPredictiveText | Qt::ImhNoAutoUppercase);
    if (query == Qt::ImCursorRectangle || query == Qt::ImAnchorRectangle) {
        const auto pt =
          LocationFromPosition(query == Qt::ImAnchorRectangle ? sel.RangeMain().anchor.Position() : position);
        return QRectF(host.toItem(pt), QSizeF(qMax(1, vs.caret.width), qMax(1, vs.lineHeight)));
    }
    if (query == Qt::ImCurrentSelection)
        return selectionText();
    if (query == Qt::ImAbsolutePosition)
        return composing() ? compositionCursor : int(pdoc->CountUTF16(0, position));
    if (query == Qt::ImSurroundingText || query == Qt::ImCursorPosition || query == Qt::ImAnchorPosition ||
        query == Qt::ImTextBeforeCursor || query == Qt::ImTextAfterCursor) {
        // Bound normal IME queries even for documents with very long lines.
        const auto start = pdoc->MovePositionOutsideChar(qMax<Sci::Position>(0, position - 4096), -1);
        const auto end = pdoc->MovePositionOutsideChar(qMin<Sci::Position>(pdoc->Length(), position + 4096), 1);
        const auto raw = RangeText(start, end);
        QString surrounding = QString::fromUtf8(raw.data(), raw.size());
        int cursor = pdoc->CountUTF16(start, position);
        int anchor = pdoc->CountUTF16(start, std::clamp(sel.RangeMain().anchor.Position(), start, end));
        if (composing()) {
            const int begin = qMax(0, compositionCursor - 4096);
            surrounding = compositionText.mid(begin, 8192);
            cursor = qBound(0, compositionCursor - begin, int(surrounding.size()));
            anchor = cursor;
        }
        if (query == Qt::ImCursorPosition)
            return cursor;
        if (query == Qt::ImAnchorPosition)
            return anchor;
        if (query == Qt::ImTextBeforeCursor)
            return surrounding.first(cursor);
        if (query == Qt::ImTextAfterCursor)
            return surrounding.sliced(cursor);
        return surrounding;
    }
    return {};
}

void
ScintillaQuickAdapter::queueUpdate()
{
    if (workQueued)
        return;
    workQueued = true;
    QTimer::singleShot(0, this, [this] {
        workQueued = false;
        IdleWork();
        NotifyUpdateUI();
    });
}
bool
ScintillaQuickAdapter::ValidCodePage(int page) const
{
    return page == SC_CP_UTF8;
}
std::string
ScintillaQuickAdapter::UTF8FromEncoded(std::string_view text) const
{
    return std::string(text);
}
std::string
ScintillaQuickAdapter::EncodedFromUTF8(std::string_view text) const
{
    return std::string(text);
}
std::unique_ptr<CaseFolder>
ScintillaQuickAdapter::CaseFolderForEncoding()
{
    return std::make_unique<CaseFolderUnicode>();
}
std::string
ScintillaQuickAdapter::CaseMapString(const std::string& text, CaseMapping mapping)
{
    if (mapping == CaseMapping::same)
        return text;
    const QString value = QString::fromUtf8(text.data(), text.size());
    return (mapping == CaseMapping::upper ? value.toUpper() : value.toLower()).toStdString();
}
void
ScintillaQuickAdapter::ScrollText(Sci::Line delta)
{
    const qreal previous = verticalOffset() + qreal(delta) * vs.lineHeight;
    synchronizeVerticalScroll();
    host.item->scrollImage(previous - verticalOffset());
}
void
ScintillaQuickAdapter::SetVerticalScrollPos()
{
    Editor::SetVerticalScrollPos();
    synchronizeVerticalScroll();
    finishVerticalScroll();
}
void
ScintillaQuickAdapter::setHorizontalPosition(qreal position)
{
    if (!std::isfinite(position))
        return;
    RefreshStyleData();
    synchronizeHorizontalScroll();
    const qreal extent = qreal(horizontalPage) + horizontalMaximum;
    WndProc(Message::SetXOffset, qRound(qBound(0.0, position, 1.0) * extent));
}

void
ScintillaQuickAdapter::SetHorizontalScrollPos()
{
    synchronizeHorizontalScroll();
    if (scrollChanged)
        scrollChanged();
}
bool
ScintillaQuickAdapter::synchronizeHorizontalScroll()
{
    // Use the live core range: caret navigation may have expanded it since
    // the last scrollbar update.
    const int hPage = qMax(1, int(GetTextRectangle().Width()));
    const int hMax = Wrapping() ? 0 : qMax(0, scrollWidth - hPage);
    const int offset = qBound(0, xOffset, hMax);
    const bool changed = hPage != horizontalPage || hMax != horizontalMaximum || offset != xOffset;
    horizontalPage = hPage;
    horizontalMaximum = hMax;
    if (offset != xOffset) {
        xOffset = offset;
        ContainerNeedsUpdate(Update::HScroll);
    }
    return changed;
}
bool
ScintillaQuickAdapter::ModifyScrollBars(Sci::Line, Sci::Line)
{
    const bool verticalChanged = synchronizeVerticalScroll();
    const bool horizontalChanged = synchronizeHorizontalScroll();
    const bool changed = verticalChanged || horizontalChanged;
    if (changed && scrollChanged)
        scrollChanged();
    return changed;
}
void
ScintillaQuickAdapter::ReconfigureScrollBars()
{
    ModifyScrollBars(MaxScrollPos() + LinesOnScreen() - 1, LinesOnScreen());
}
void
ScintillaQuickAdapter::Copy()
{
    if (!sel.Empty()) {
        SelectionText text;
        CopySelectionRange(&text);
        CopyToClipboard(text);
    }
}
void
ScintillaQuickAdapter::CopyToClipboard(const SelectionText& text)
{
    const auto bytes = text.AsView();
    auto* mime = new QMimeData;
    mime->setText(QString::fromUtf8(bytes.data(), bytes.size()));
    if (text.rectangular)
        mime->setData(rectangularMime, QByteArray(bytes.data(), bytes.size()));
    QGuiApplication::clipboard()->setMimeData(mime);
}
bool
ScintillaQuickAdapter::CanPaste()
{
    const auto* mime = QGuiApplication::clipboard()->mimeData();
    return !pdoc->IsReadOnly() && mime && mime->hasText();
}
void
ScintillaQuickAdapter::Paste()
{
    if (!CanPaste())
        return;
    const auto* mime = QGuiApplication::clipboard()->mimeData();
    const QByteArray bytes = mime->text().toUtf8();
    UndoGroup undo(pdoc);
    ClearSelection(multiPasteMode == MultiPaste::Each);
    InsertPasteShape(std::string_view(bytes.constData(), bytes.size()),
                     mime->hasFormat(rectangularMime) ? PasteShape::rectangular : PasteShape::stream);
    EnsureCaretVisible();
}
void
ScintillaQuickAdapter::ClaimSelection()
{
}
void
ScintillaQuickAdapter::NotifyChange()
{
    queueUpdate();
}
void
ScintillaQuickAdapter::NotifyModified(Document* document, DocModification modification, void* userData)
{
    const auto changed = modification.modificationType;
    if (textModified && FlagSet(changed, ModificationFlags::InsertText | ModificationFlags::DeleteText)) {
        const bool inserted = FlagSet(changed, ModificationFlags::InsertText);
        textModified(modification.position,
                     inserted ? 0 : modification.length,
                     inserted ? QByteArray(modification.text, modification.length) : QByteArray{});
    }
    ScintillaBase::NotifyModified(document, modification, userData);
    const auto flags = modification.modificationType;
    if (FlagSet(flags, ModificationFlags::InsertText | ModificationFlags::DeleteText)) {
        const Sci::Line line = pdoc->SciLineFromPosition(modification.position);
        if (Sci::Line(lineWidths.size()) != pdoc->LinesTotal() - modification.linesAdded) {
            invalidateHorizontalExtent();
            return;
        }
        const Sci::Line removed = std::max<Sci::Line>(0, -modification.linesAdded);
        invalidateLineWidths(line, line + removed);
        if (removed)
            lineWidths.erase(lineWidths.begin() + line + 1, lineWidths.begin() + line + removed + 1);
        else if (modification.linesAdded > 0)
            lineWidths.insert(lineWidths.begin() + line + 1, size_t(modification.linesAdded), -1);
        widthEndLine += modification.linesAdded;
    } else if (FlagSet(flags, ModificationFlags::ChangeStyle)) {
        invalidateLineWidths(pdoc->SciLineFromPosition(modification.position),
                             pdoc->SciLineFromPosition(modification.position + modification.length));
    } else if (FlagSet(flags, ModificationFlags::ChangeTabStops)) {
        invalidateLineWidths(modification.line, modification.line);
    }
}
void
ScintillaQuickAdapter::NotifyParent(NotificationData data)
{
    // IME candidates live in Scintilla's tentative undo transaction, not the bound document text.
    if (tentativeUpdate && data.nmhdr.code == Notification::Modified &&
        (int(data.modificationType) & (SC_MOD_INSERTTEXT | SC_MOD_DELETETEXT)))
        return;
    // Recompute selection extents after the core finishes dispatching the notification.
    if (data.nmhdr.code == Notification::UpdateUI && FlagSet(data.updated, Update::Selection) && trackLineWidth)
        widthTimer.start(0);
    if ((data.nmhdr.code == Notification::DwellStart || data.nmhdr.code == Notification::DwellEnd) &&
        (data.x != -1 || data.y != -1))
        data.y = std::floor(data.y - host.scrollOffsetY);
    if (notification)
        notification(data);
}
bool
ScintillaQuickAdapter::FineTickerRunning(TickReason reason)
{
    return timers[size_t(reason)] != 0;
}
void
ScintillaQuickAdapter::FineTickerStart(TickReason reason, int millis, int)
{
    FineTickerCancel(reason);
    timers[size_t(reason)] = startTimer(millis);
}
void
ScintillaQuickAdapter::FineTickerCancel(TickReason reason)
{
    int& timer = timers[size_t(reason)];
    if (timer)
        killTimer(timer);
    timer = 0;
}
bool
ScintillaQuickAdapter::SetIdle(bool on)
{
    if (on)
        idleTimer.start();
    else
        idleTimer.stop();
    return true;
}
void
ScintillaQuickAdapter::QueueIdleWork(WorkItems items, Sci::Position upTo)
{
    Editor::QueueIdleWork(items, upTo);
    queueUpdate();
}
void
ScintillaQuickAdapter::timerEvent(QTimerEvent* event)
{
    for (size_t i = 0; i < timers.size(); ++i)
        if (timers[i] == event->timerId()) {
            TickFor(static_cast<TickReason>(i));
            if (static_cast<TickReason>(i) == TickReason::scroll)
                finishSelectionScroll();
            queueUpdate();
            break;
        }
}
void
ScintillaQuickAdapter::SetMouseCapture(bool on)
{
    captured = on && mouseDownCaptures;
    host.item->setKeepMouseGrab(captured);
}
bool
ScintillaQuickAdapter::HaveMouseCapture()
{
    return captured;
}
bool
ScintillaQuickAdapter::DragThreshold(Point start, Point current)
{
    return std::abs(start.x - current.x) + std::abs(start.y - current.y) >=
           QGuiApplication::styleHints()->startDragDistance();
}
void
ScintillaQuickAdapter::StartDrag()
{
    if (!drag.Length())
        return;
    inDragDrop = DragDrop::dragging;
    dropWentOutside = true;
    auto* mime = new QMimeData;
    const auto bytes = drag.AsView();
    mime->setText(QString::fromUtf8(bytes.data(), bytes.size()));
    if (drag.rectangular)
        mime->setData(rectangularMime, QByteArray(bytes.data(), bytes.size()));
    const QPointer<QDrag> operation = new QDrag(host.item);
    operation->setMimeData(mime);
    const QPointer<ScintillaQuickAdapter> guard(this);
    const auto result =
      operation->exec(pdoc->IsReadOnly() ? Qt::CopyAction : Qt::CopyAction | Qt::MoveAction, Qt::MoveAction);
    if (operation)
        operation->deleteLater();
    if (!guard)
        return;
    if (result == Qt::MoveAction && dropWentOutside)
        ClearSelection();
    inDragDrop = DragDrop::none;
    SetMouseCapture(false);
    dragLeave();
}
void
ScintillaQuickAdapter::dragMove(QPointF position)
{
    SetDragPosition(SPositionFromLocation(contentPoint(position), false, false, UserVirtualSpace()));
}
void
ScintillaQuickAdapter::dragLeave()
{
    SetDragPosition(SelectionPosition(Sci::invalidPosition));
}
void
ScintillaQuickAdapter::drop(QPointF position, const QMimeData* mime, bool move)
{
    if (pdoc->IsReadOnly() || !mime->hasText())
        return;
    if (composing()) {
        // Finish the input context before hit-testing: resolving preedit changes document positions.
        if (host.item->hasActiveFocus()) {
            auto* inputMethod = QGuiApplication::inputMethod();
            inputMethod->commit();
            if (composing())
                inputMethod->reset();
        }
        // A platform may not deliver a final IME event; keep the drop outside any tentative transaction.
        cancelComposition();
    }
    const auto bytes = mime->text().toUtf8();
    DropAt(SPositionFromLocation(contentPoint(position), false, false, UserVirtualSpace()),
           std::string_view(bytes.constData(), bytes.size()),
           move,
           mime->hasFormat(rectangularMime));
    queueUpdate();
}
void
ScintillaQuickAdapter::CreateCallTipWindow(PRectangle rect)
{
    if (!ct.wCallTip.Created()) {
        ct.wCallTip = CreateQuickPopup(
          host,
          [this](QPainter& painter) {
              AutoSurface surface(painter.device(), this);
              ct.PaintCT(surface);
          },
          [this](QPointF pt, bool) {
              ct.MouseClick(PointFromQPointF(pt));
              CallTipClick();
          });
    }
    ct.wCallTip.SetPositionRelative(rect, &wMain);
}
void
ScintillaQuickAdapter::AddToPopUp(const char* label, int command, bool enabled)
{
    auto* menu = static_cast<QuickMenu*>(popup.GetID());
    menu->entries.append(QVariantMap{ { QStringLiteral("text"), QString::fromUtf8(label) },
                                      { QStringLiteral("command"), command },
                                      { QStringLiteral("enabled"), enabled } });
}
sptr_t
ScintillaQuickAdapter::DefWndProc(Message, uptr_t, sptr_t)
{
    return 0;
}
