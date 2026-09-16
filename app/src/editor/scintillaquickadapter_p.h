// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#ifndef CRAFTWARD_SCINTILLAQUICKADAPTER_P_H
#define CRAFTWARD_SCINTILLAQUICKADAPTER_P_H

#include <algorithm>
#include <cassert>
#include <cctype>
#include <cmath>
#include <cstddef>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <ctime>
#include <map>
#include <memory>
#include <new>
#include <optional>
#include <set>
#include <stdexcept>
#include <string>
#include <string_view>
#include <vector>

// Scintilla's internal headers require this include order.
// clang-format off
#include "ScintillaTypes.h"
#include "ScintillaMessages.h"
#include "ScintillaStructures.h"
#include "Scintilla.h"
#include "Debugging.h"
#include "Geometry.h"
#include "Platform.h"
#include "ILoader.h"
#include "ILexer.h"
#include "CharacterCategoryMap.h"
#include "Position.h"
#include "UniqueString.h"
#include "SplitVector.h"
#include "Partitioning.h"
#include "RunStyles.h"
#include "ContractionState.h"
#include "CellBuffer.h"
#include "CallTip.h"
#include "KeyMap.h"
#include "Indicator.h"
#include "LineMarker.h"
#include "Style.h"
#include "AutoComplete.h"
#include "ViewStyle.h"
#include "CharClassify.h"
#include "Decoration.h"
#include "CaseFolder.h"
#include "Document.h"
#include "Selection.h"
#include "PositionCache.h"
#include "EditModel.h"
#include "MarginView.h"
#include "EditView.h"
#include "Editor.h"
#include "ScintillaBase.h"
#include "CaseConvert.h"
// clang-format on

#include "scintillaquickplatform_p.h"
#include <QElapsedTimer>
#include <QInputMethodEvent>
#include <QKeyEvent>
#include <QObject>
#include <QTimer>
#include <array>
#include <functional>

class ScintillaQuickAdapter final
  : public QObject
  , public Scintilla::Internal::ScintillaBase
{
  public:
    static constexpr int lineNumberPadding = 8;
    explicit ScintillaQuickAdapter(ScintillaImageItem* owner);
    ~ScintillaQuickAdapter() override;
    std::function<void(const Scintilla::NotificationData&)> notification;
    std::function<void()> scrollChanged;
    Scintilla::Internal::QuickWindow host;
    int verticalMaximum = 0, verticalPage = 1;
    int horizontalMaximum = 0, horizontalPage = 1;
    qreal wheelRemainder = 0;

    Scintilla::sptr_t WndProc(Scintilla::Message message,
                              Scintilla::uptr_t wParam = 0,
                              Scintilla::sptr_t lParam = 0) override;
    bool paint(QPainter& painter, const QRect& rect);
    void resize();
    void updateMetrics();
    void resetHorizontalExtent();
    void updateHorizontalExtent();
    void focus(bool on);
    static std::optional<Scintilla::Message> shortcutCommand(const QKeyEvent& event);
    bool key(QKeyEvent* event);
    void mouse(QMouseEvent* event);
    void hover(QPointF position, Qt::KeyboardModifiers modifiers);
    void leave();
    void releaseMouse();
    void wheel(QWheelEvent* event);
    void setHorizontalPosition(qreal position);
    void inputMethod(QInputMethodEvent* event);
    QVariant inputQuery(Qt::InputMethodQuery query);
    void cancelComposition();
    bool composing() const;
    QString documentText() const;
    QString selectionText() const;
    void dragMove(QPointF point);
    void dragLeave();
    void drop(QPointF point, const QMimeData* mime, bool move);

  private:
    std::array<int, 5> timers{};
    QTimer idleTimer;
    QTimer widthTimer;
    // Scintilla only grows its observed width; current per-line widths also allow the range to shrink.
    std::vector<int> lineWidths;
    std::map<int, Sci::Line> measuredLineWidths;
    Sci::Line nextWidthLine = 0;
    Sci::Line widthEndLine = 0;
    int lastVirtualSpaceWidth = 0;
    QElapsedTimer clock;
    bool captured = false;
    bool workQueued = false;
    bool tentativeUpdate = false;
    Scintilla::Position preeditPosition = -1;
    int compositionCursor = 0;
    QString compositionText;
    std::string compositionSelection;
    unsigned int timestamp() const;
    void queueUpdate();
    bool synchronizeHorizontalScroll();
    void invalidateHorizontalExtent();
    void invalidateLineWidths(Sci::Line first, Sci::Line last);
    void insertQString(const QString& text, Scintilla::CharacterSource source);
    void applyPreeditFormats(const QInputMethodEvent& event);

    bool ValidCodePage(int codePage) const override;
    std::string UTF8FromEncoded(std::string_view text) const override;
    std::string EncodedFromUTF8(std::string_view text) const override;
    std::unique_ptr<Scintilla::Internal::CaseFolder> CaseFolderForEncoding() override;
    std::string CaseMapString(const std::string& text, CaseMapping mapping) override;
    void ScrollText(Sci::Line delta) override;
    void SetVerticalScrollPos() override;
    void SetHorizontalScrollPos() override;
    bool ModifyScrollBars(Sci::Line maximum, Sci::Line page) override;
    void ReconfigureScrollBars() override;
    void Copy() override;
    void CopyToClipboard(const Scintilla::Internal::SelectionText& text) override;
    void Paste() override;
    bool CanPaste() override;
    void ClaimSelection() override;
    void NotifyChange() override;
    void NotifyModified(Scintilla::Internal::Document* document,
                        Scintilla::Internal::DocModification modification,
                        void* userData) override;
    void NotifyParent(Scintilla::NotificationData data) override;
    bool FineTickerRunning(TickReason reason) override;
    void FineTickerStart(TickReason reason, int millis, int tolerance) override;
    void FineTickerCancel(TickReason reason) override;
    bool SetIdle(bool on) override;
    void QueueIdleWork(Scintilla::Internal::WorkItems items, Sci::Position upTo) override;
    void timerEvent(QTimerEvent* event) override;
    void SetMouseCapture(bool on) override;
    bool HaveMouseCapture() override;
    bool DragThreshold(Scintilla::Internal::Point start, Scintilla::Internal::Point current) override;
    void StartDrag() override;
    void CreateCallTipWindow(Scintilla::Internal::PRectangle rect) override;
    void AddToPopUp(const char* label, int command, bool enabled) override;
    Scintilla::sptr_t DefWndProc(Scintilla::Message, Scintilla::uptr_t, Scintilla::sptr_t) override;
};

#endif
