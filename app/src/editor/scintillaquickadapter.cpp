// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "scintillaquickadapter_p.h"

#include <QClipboard>
#include <QDrag>
#include <QGuiApplication>
#include <QInputMethod>
#include <QKeySequence>
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
Point
point(QPointF value)
{
    return Point(value.x(), value.y());
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
}

ScintillaQuickAdapter::~ScintillaQuickAdapter()
{
    notification = {};
    scrollChanged = {};
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
        if (message == Message::GrabFocus) {
            host.item->forceActiveFocus();
            return 0;
        }
        return ScintillaBase::WndProc(message, wParam, lParam);
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
    rcPaint = PRectFromQRect(rect);
    paintState = PaintState::painting;
    paintingAllText = rcPaint.Contains(GetClientRectangle());
    AutoSurface surface(painter.device(), this);
    Paint(surface, rcPaint);
    const bool complete = paintState != PaintState::abandoned;
    paintState = PaintState::notPainting;
    queueUpdate();
    return complete;
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
        consumed = true;
    }
    queueUpdate();
    return consumed;
}

void
ScintillaQuickAdapter::mouse(QMouseEvent* event)
{
    const Point pt = point(event->position());
    const KeyMod mods = modifiers(event->modifiers());
    if (event->type() == QEvent::MouseMove) {
        ButtonMoveWithModifiers(pt, timestamp(), mods);
    } else if (event->button() == Qt::LeftButton) {
        if (event->type() == QEvent::MouseButtonRelease)
            ButtonUpWithModifiers(pt, timestamp(), mods);
        else {
            cancelComposition();
            ButtonDownWithModifiers(pt, timestamp(), mods);
        }
    } else if (event->button() == Qt::RightButton && event->type() == QEvent::MouseButtonPress) {
        cancelComposition();
        RightButtonDownWithModifiers(pt, timestamp(), mods);
        ContextMenu(pt);
    }
    queueUpdate();
}
void
ScintillaQuickAdapter::hover(QPointF position, Qt::KeyboardModifiers mods)
{
    ButtonMoveWithModifiers(point(position), timestamp(), modifiers(mods));
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
    const QPoint pixels = event->pixelDelta();
    if (!pixels.isNull()) {
        wheelRemainder -= pixels.y();
        const int lines = int(wheelRemainder / qMax(1, vs.lineHeight));
        wheelRemainder -= lines * vs.lineHeight;
        ScrollTo(topLine + lines);
        if (!Wrapping())
            HorizontalScrollTo(xOffset - pixels.x());
    } else {
        const QPoint delta = event->angleDelta();
        const int lines = -delta.y() * QGuiApplication::styleHints()->wheelScrollLines() / 120;
        if (event->modifiers() & Qt::ShiftModifier) {
            if (!Wrapping())
                HorizontalScrollTo(xOffset + lines * int(vs.aveCharWidth));
        } else {
            ScrollTo(topLine + lines);
            if (!Wrapping())
                HorizontalScrollTo(xOffset - delta.x() * int(vs.aveCharWidth) / 120);
        }
    }
    event->accept();
    queueUpdate();
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
        return QRectF(pt.x, pt.y, qMax(1, vs.caret.width), qMax(1, vs.lineHeight));
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
    host.item->scrollImage(delta * vs.lineHeight);
}
void
ScintillaQuickAdapter::SetVerticalScrollPos()
{
    Editor::SetVerticalScrollPos();
    if (scrollChanged)
        scrollChanged();
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
ScintillaQuickAdapter::ModifyScrollBars(Sci::Line maximum, Sci::Line page)
{
    const int vPage = qMax<Sci::Line>(1, page);
    const int vMax = qMax<Sci::Line>(0, maximum - vPage + 1);
    const bool horizontalChanged = synchronizeHorizontalScroll();
    const bool changed = vPage != verticalPage || vMax != verticalMaximum || horizontalChanged;
    verticalPage = vPage;
    verticalMaximum = vMax;
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
ScintillaQuickAdapter::NotifyParent(NotificationData data)
{
    // IME candidates live in Scintilla's tentative undo transaction, not the bound document text.
    if (tentativeUpdate && data.nmhdr.code == Notification::Modified &&
        (int(data.modificationType) & (SC_MOD_INSERTTEXT | SC_MOD_DELETETEXT)))
        return;
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
    SetDragPosition(SPositionFromLocation(point(position), false, false, UserVirtualSpace()));
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
    DropAt(SPositionFromLocation(point(position), false, false, UserVirtualSpace()),
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
              ct.MouseClick(point(pt));
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
