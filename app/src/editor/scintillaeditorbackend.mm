// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "scintillaeditorbackend.h"

#import <Cocoa/Cocoa.h>

#import "ScintillaView.h"

#include "Scintilla.h"

#include <QPointer>
#include <QtGlobal>

#include <array>
#include <utility>

class ScintillaEditorBackendPrivate;

@interface CraftwardScintillaNotificationBridge : NSObject<ScintillaNotificationProtocol> {
    ScintillaEditorBackendPrivate* backend;
}

- (instancetype)initWithBackend:(ScintillaEditorBackendPrivate*)backend;
- (void)invalidate;

@end

namespace {

NSColor*
nativeColor(const QColor& color)
{
    return [NSColor colorWithSRGBRed:color.redF() green:color.greenF() blue:color.blueF() alpha:color.alphaF()];
}

sptr_t
scintillaColor(const QColor& color)
{
    const quint32 rgba = static_cast<quint32>(color.red()) | (static_cast<quint32>(color.green()) << 8U) |
                         (static_cast<quint32>(color.blue()) << 16U) | (static_cast<quint32>(color.alpha()) << 24U);
    return static_cast<sptr_t>(rgba);
}

NSString*
nativeString(const QString& string)
{
    return [NSString stringWithCharacters:reinterpret_cast<const unichar*>(string.utf16()) length:string.size()];
}

QString
qtString(NSString* string)
{
    return QString::fromNSString(string);
}

} // namespace

class ScintillaEditorBackendPrivate
{
  public:
    explicit ScintillaEditorBackendPrivate(ScintillaEditorBackend* owner)
      : q(owner)
      , view([[ScintillaView alloc] initWithFrame:NSMakeRect(0, 0, 320, 240)])
      , notificationBridge([[CraftwardScintillaNotificationBridge alloc] initWithBackend:this])
    {
        view.delegate = notificationBridge;
        view.autoresizesSubviews = YES;
        view.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        view.scrollView.borderType = NSNoBorder;
        view.scrollView.scrollerStyle = NSScrollerStyleOverlay;

        foreignWindow = QWindow::fromWinId(WId(view));
        Q_ASSERT(foreignWindow);

        applyEditorBehavior();
        applyDefaultStyle();
        applySelectionStyle();
    }

    ~ScintillaEditorBackendPrivate()
    {
        view.delegate = nil;
        [notificationBridge invalidate];

        if (foreignWindow) {
            foreignWindow->setVisible(false);
            foreignWindow->setParent(nullptr);
            delete foreignWindow;
        }

        notificationBridge = nil;
        view = nil;
    }

    void applyEditorBehavior()
    {
        [view setEditable:!readOnly];
        [view message:SCI_SETCODEPAGE wParam:SC_CP_UTF8];
        [view message:SCI_SETWRAPMODE wParam:wordWrap ? SC_WRAP_WORD : SC_WRAP_NONE];
        [view message:SCI_SETHSCROLLBAR wParam:wordWrap ? 0 : 1];
        [view message:SCI_SETSCROLLWIDTHTRACKING wParam:1];
        [view message:SCI_SETCARETWIDTH wParam:readOnly ? 0 : 1];

        for (uptr_t margin = 0; margin < 5; ++margin)
            [view message:SCI_SETMARGINWIDTHN wParam:margin lParam:0];
    }

    void applyDefaultStyle()
    {
        [view setStringProperty:SCI_STYLESETFONT parameter:STYLE_DEFAULT value:nativeString(fontFamily)];
        [view setGeneralProperty:SCI_STYLESETSIZEFRACTIONAL
                       parameter:STYLE_DEFAULT
                           value:qRound(fontPointSize * SC_FONT_SIZE_MULTIPLIER)];
        [view setColorProperty:SCI_STYLESETFORE parameter:STYLE_DEFAULT value:nativeColor(foregroundColor)];
        [view setColorProperty:SCI_STYLESETBACK parameter:STYLE_DEFAULT value:nativeColor(backgroundColor)];
        [view setGeneralProperty:SCI_STYLECLEARALL parameter:0 value:0];
        [view setColorProperty:SCI_SETCARETFORE parameter:0 value:nativeColor(foregroundColor)];

        view.scrollView.drawsBackground = YES;
        view.scrollView.backgroundColor = nativeColor(backgroundColor);
    }

    void applySelectionStyle()
    {
        static constexpr std::array<uptr_t, 5> textElements = {
            SC_ELEMENT_SELECTION_TEXT,
            SC_ELEMENT_SELECTION_ADDITIONAL_TEXT,
            SC_ELEMENT_SELECTION_SECONDARY_TEXT,
            SC_ELEMENT_SELECTION_INACTIVE_TEXT,
            SC_ELEMENT_SELECTION_INACTIVE_ADDITIONAL_TEXT,
        };
        static constexpr std::array<uptr_t, 5> backgroundElements = {
            SC_ELEMENT_SELECTION_BACK,
            SC_ELEMENT_SELECTION_ADDITIONAL_BACK,
            SC_ELEMENT_SELECTION_SECONDARY_BACK,
            SC_ELEMENT_SELECTION_INACTIVE_BACK,
            SC_ELEMENT_SELECTION_INACTIVE_ADDITIONAL_BACK,
        };

        for (const uptr_t element : textElements)
            [view message:SCI_SETELEMENTCOLOUR wParam:element lParam:scintillaColor(selectionForegroundColor)];
        for (const uptr_t element : backgroundElements)
            [view message:SCI_SETELEMENTCOLOUR wParam:element lParam:scintillaColor(selectionBackgroundColor)];

        [view message:SCI_SETSELECTIONLAYER wParam:SC_LAYER_BASE];
    }

    void replaceText(const QString& newText)
    {
        const bool wasEditable = view.isEditable;
        updatingText = true;
        [view setEditable:YES];
        [view setString:nativeString(newText)];
        [view setEditable:wasEditable];
        [view message:SCI_EMPTYUNDOBUFFER];
        [view message:SCI_SETSAVEPOINT];
        [view message:SCI_SETEMPTYSELECTION wParam:0];
        [view message:SCI_SETFIRSTVISIBLELINE wParam:0];
        [view message:SCI_SETXOFFSET wParam:0];
        updatingText = false;
    }

    void handleNotification(SCNotification* notification)
    {
        if (updatingText || notification->nmhdr.code != SCN_MODIFIED)
            return;

        const int textChange = SC_MOD_INSERTTEXT | SC_MOD_DELETETEXT;
        if ((notification->modificationType & textChange) != 0)
            q->handleNativeTextChanged();
    }

    ScintillaEditorBackend* q;
    __strong ScintillaView* view;
    __strong CraftwardScintillaNotificationBridge* notificationBridge;
    QPointer<QWindow> foreignWindow;
    QString text;
    bool readOnly = false;
    bool wordWrap = false;
    QString fontFamily = QStringLiteral("Menlo");
    qreal fontPointSize = 13.0;
    QColor foregroundColor = Qt::black;
    QColor backgroundColor = Qt::white;
    QColor selectionForegroundColor = Qt::white;
    QColor selectionBackgroundColor = QColor(0, 122, 255);
    bool updatingText = false;
};

@implementation CraftwardScintillaNotificationBridge

- (instancetype)initWithBackend:(ScintillaEditorBackendPrivate*)newBackend
{
    self = [super init];
    if (self)
        backend = newBackend;
    return self;
}

- (void)invalidate
{
    backend = nullptr;
}

- (void)notification:(SCNotification*)notification
{
    if (backend)
        backend->handleNotification(notification);
}

@end

ScintillaEditorBackend::ScintillaEditorBackend(QObject* parent)
  : QObject(parent)
  , d(std::make_unique<ScintillaEditorBackendPrivate>(this))
{
}

ScintillaEditorBackend::~ScintillaEditorBackend() = default;

QWindow*
ScintillaEditorBackend::window() const
{
    return d->foreignWindow;
}

QString
ScintillaEditorBackend::text() const
{
    return d->text;
}

void
ScintillaEditorBackend::setText(const QString& text)
{
    if (d->text == text)
        return;

    d->text = text;
    d->replaceText(text);
    emit textChanged();
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

    d->readOnly = readOnly;
    d->applyEditorBehavior();
    emit readOnlyChanged();
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
    d->applyEditorBehavior();
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

void
ScintillaEditorBackend::handleNativeTextChanged()
{
    const QString currentText = qtString(d->view.string);
    if (d->text == currentText)
        return;

    d->text = currentText;
    emit textChanged();
}
