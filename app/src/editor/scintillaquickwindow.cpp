// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "scintillaquickplatform_p.h"

#include "XPM.h"

#include <QGuiApplication>
#include <QMouseEvent>
#include <QQuickWindow>
#include <QScreen>
#include <QStyleHints>
#include <QWheelEvent>

#include <cstdarg>
#include <cstdio>

namespace Scintilla::Internal {
namespace {
class PopupItem : public ScintillaImageItem
{
  public:
    explicit PopupItem(QQuickItem* parent)
      : ScintillaImageItem(parent)
    {
        setAcceptedMouseButtons(Qt::LeftButton);
        setZ(1000);
    }
    std::function<void(QPainter&)> paint;
    std::function<void(QPointF, bool)> click;
    std::function<void(const QWheelEvent&)> wheel;
    bool paintImage(QPainter& painter, const QRect&) override
    {
        if (paint)
            paint(painter);
        return true;
    }
    void mousePressEvent(QMouseEvent* event) override
    {
        if (click)
            click(event->position(), false);
        event->accept();
    }
    void mouseDoubleClickEvent(QMouseEvent* event) override
    {
        if (click)
            click(event->position(), true);
        event->accept();
    }
    void wheelEvent(QWheelEvent* event) override
    {
        if (wheel)
            wheel(*event);
        event->accept();
    }
};
}

QuickWindow*
CreateQuickPopup(QuickWindow& parent,
                 std::function<void(QPainter&)> paint,
                 std::function<void(QPointF, bool)> click,
                 std::function<void(const QWheelEvent&)> wheel)
{
    auto* container = parent.item && parent.item->window() ? parent.item->window()->contentItem() : parent.item.data();
    auto* popup = new PopupItem(container);
    popup->setVisible(false);
    popup->paint = std::move(paint);
    popup->click = std::move(click);
    popup->wheel = std::move(wheel);
    auto* result = new QuickWindow;
    result->item = popup;
    result->measurementDevice = parent.measurementDevice;
    result->devicePixelRatio = parent.devicePixelRatio;
    result->owned = true;
    return result;
}

Window::~Window() noexcept = default;
void
Window::Destroy() noexcept
{
    if (auto* host = window(wid); host && host->owned) {
        if (auto* item = static_cast<PopupItem*>(host->item.data())) {
            item->paint = {};
            item->click = {};
            item->wheel = {};
            item->setVisible(false);
            item->deleteLater();
        }
        delete host;
    }
    wid = nullptr;
}
PRectangle
Window::GetPosition() const
{
    const auto* host = window(wid);
    if (!host || !host->item)
        return PRectangle(0, 0, 1000, 1000);
    const auto* item = host->item.data();
    return PRectangle(item->x(), item->y(), item->x() + item->width(), item->y() + item->height());
}
PRectangle
Window::GetClientPosition() const
{
    const auto rc = GetPosition();
    return PRectangle(0, 0, rc.Width(), rc.Height());
}
void
Window::SetPosition(PRectangle rc)
{
    if (const auto* host = window(wid); host && host->item) {
        host->item->setPosition(QPointF(rc.left, rc.top));
        host->item->setSize(QSizeF(rc.Width(), rc.Height()));
    }
}
void
Window::SetPositionRelative(PRectangle rc, const Window* relativeTo)
{
    const auto* host = window(wid);
    const auto* relative = window(relativeTo->GetID());
    if (!host || !host->item || !relative || !relative->item)
        return;
    auto* container = host->item->parentItem();
    const QPointF point = relative->item->mapToItem(container, relative->toItem(Point(rc.left, rc.top)));
    const qreal x = qBound(0.0, point.x(), qMax(0.0, container->width() - rc.Width()));
    const qreal y = qBound(0.0, point.y(), qMax(0.0, container->height() - rc.Height()));
    SetPosition(PRectangle(x, y, x + rc.Width(), y + rc.Height()));
}
void
Window::Show(bool show)
{
    if (const auto* host = window(wid); host && host->item) {
        host->item->setVisible(show);
        if (show)
            host->item->invalidateImage();
    }
}
void
Window::InvalidateAll()
{
    if (const auto* host = window(wid); host && host->item)
        host->item->invalidateImage();
}
void
Window::InvalidateRectangle(PRectangle rc)
{
    if (const auto* host = window(wid); host && host->item)
        host->item->invalidateImage(QRectFFromPRect(rc).translated(0, -host->scrollOffsetY));
}
void
Window::SetCursor(Cursor cursor)
{
    const auto* host = window(wid);
    if (!host || !host->item || cursor == cursorLast)
        return;
    Qt::CursorShape shape = Qt::ArrowCursor;
    switch (cursor) {
        case Cursor::text:
            shape = Qt::IBeamCursor;
            break;
        case Cursor::wait:
            shape = Qt::WaitCursor;
            break;
        case Cursor::horizontal:
            shape = Qt::SizeHorCursor;
            break;
        case Cursor::vertical:
            shape = Qt::SizeVerCursor;
            break;
        case Cursor::hand:
            shape = Qt::PointingHandCursor;
            break;
        default:
            break;
    }
    host->item->setCursor(QCursor(shape));
    cursorLast = cursor;
}
PRectangle
Window::GetMonitorRect(Point)
{
    const auto* host = window(wid);
    if (!host || !host->item || !host->item->window())
        return PRectangle(0, 0, 1000, 1000);
    auto* container = host->item->window()->contentItem();
    const Point origin = host->toContent(container->mapToItem(host->item, QPointF()));
    return PRectangle(origin.x, origin.y, origin.x + container->width(), origin.y + container->height());
}

namespace {
class QuickListBox : public ListBox
{
    QStringList items;
    QList<int> types;
    QHash<int, QImage> images;
    QFont font;
    IListBoxDelegate* delegate = nullptr;
    ListOptions options;
    int visibleRows = 5;
    int rowHeight = 20;
    int selected = -1;
    int first = 0;
    int averageWidth = 8;
    int pixelWheelRemainder = 0;
    int angleWheelRemainder = 0;

    void scroll(const QWheelEvent& event)
    {
        if (event.phase() == Qt::ScrollBegin)
            pixelWheelRemainder = angleWheelRemainder = 0;
        int rows = 0;
        const QPoint pixels = event.pixelDelta();
        if (!pixels.isNull()) {
            pixelWheelRemainder -= pixels.y();
            rows = pixelWheelRemainder / rowHeight;
            pixelWheelRemainder %= rowHeight;
            angleWheelRemainder = 0;
        } else if (event.angleDelta().y()) {
            angleWheelRemainder -= event.angleDelta().y();
            rows = angleWheelRemainder / 120;
            angleWheelRemainder %= 120;
            pixelWheelRemainder = 0;
        }
        if (event.phase() == Qt::ScrollEnd)
            pixelWheelRemainder = angleWheelRemainder = 0;
        const int position = qBound(0, first + rows, qMax(0, int(items.size()) - visibleRows));
        if (position != first) {
            first = position;
            InvalidateAll();
        }
    }

  public:
    void SetFont(const Font* value) override
    {
        font = FontForQuick(value);
        InvalidateAll();
    }
    void Create(Window& parent, int, Point, int height, bool, Technology) override
    {
        Destroy();
        pixelWheelRemainder = angleWheelRemainder = 0;
        rowHeight = qMax(1, height);
        wid = CreateQuickPopup(
          *window(parent.GetID()),
          [this](QPainter& painter) {
              const QRectF bounds = window(wid)->item->boundingRect();
              const QColor back = options.back ? QColorFromColourRGBA(*options.back) : QColor(Qt::white);
              const QColor fore = options.fore ? QColorFromColourRGBA(*options.fore) : QColor(Qt::black);
              painter.fillRect(bounds, back);
              painter.setFont(font);
              for (int row = first; row < qMin(first + visibleRows, items.size()); ++row) {
                  const QRectF rect(1, 1 + (row - first) * rowHeight, bounds.width() - 2, rowHeight);
                  if (row == selected) {
                      painter.fillRect(
                        rect, options.backSelected ? QColorFromColourRGBA(*options.backSelected) : QColor(0, 122, 255));
                      painter.setPen(options.foreSelected ? QColorFromColourRGBA(*options.foreSelected)
                                                          : QColor(Qt::white));
                  } else {
                      painter.setPen(fore);
                  }
                  qreal left = 4;
                  if (images.contains(types[row])) {
                      const QImage& image = images[types[row]];
                      const qreal edge = qMin(qreal(rowHeight), image.height() * options.imageScale);
                      painter.drawImage(QRectF(left, rect.top(), edge, edge), image);
                      left += edge + 4;
                  }
                  painter.drawText(rect.adjusted(left, 0, -4, 0), Qt::AlignVCenter | Qt::AlignLeft, items[row]);
              }
              painter.setPen(QColor(128, 128, 128));
              painter.drawRect(bounds.adjusted(0.5, 0.5, -0.5, -0.5));
          },
          [this](QPointF point, bool doubleClick) {
              Select(qBound(0, first + int(point.y() - 1) / rowHeight, int(items.size()) - 1));
              if (doubleClick && delegate) {
                  ListBoxEvent event(ListBoxEvent::EventType::doubleClick);
                  delegate->ListNotify(&event);
              }
          },
          [this](const QWheelEvent& event) { scroll(event); });
    }
    void SetAverageCharWidth(int width) override { averageWidth = width; }
    void SetVisibleRows(int rows) override { visibleRows = qMax(1, rows); }
    int GetVisibleRows() const override { return visibleRows; }
    PRectangle GetDesiredRect() override
    {
        qreal width = 12 * averageWidth;
        const QFontMetricsF metrics(font);
        for (const QString& item : items)
            width = qMax(width, metrics.horizontalAdvance(item) + rowHeight + 12);
        return PRectangle(0, 0, width, qMin(visibleRows, qMax(1, int(items.size()))) * rowHeight + 2);
    }
    int CaretFromEdge() override { return 4; }
    void Clear() noexcept override
    {
        items.clear();
        types.clear();
        selected = -1;
        first = 0;
        pixelWheelRemainder = angleWheelRemainder = 0;
    }
    void Append(char* text, int type) override
    {
        items.append(QString::fromUtf8(text));
        types.append(type);
    }
    int Length() override { return items.size(); }
    void Select(int index) override
    {
        index = index >= 0 && index < items.size() ? index : -1;
        if (selected == index)
            return;
        selected = index;
        if (index >= 0)
            first = qBound(qMax(0, index - visibleRows + 1), first, index);
        InvalidateAll();
        if (delegate && selected >= 0) {
            ListBoxEvent event(ListBoxEvent::EventType::selectionChange);
            delegate->ListNotify(&event);
        }
    }
    int GetSelection() override { return selected; }
    int Find(const char* prefix) override
    {
        for (int i = 0; i < items.size(); ++i)
            if (items[i].startsWith(QString::fromUtf8(prefix)))
                return i;
        return -1;
    }
    std::string GetValue(int index) override
    {
        return index >= 0 && index < items.size() ? items[index].toStdString() : std::string();
    }
    void RegisterImage(int type, const char* data) override
    {
        const XPM xpm(data);
        const RGBAImage image(xpm);
        RegisterRGBAImage(type, image.GetWidth(), image.GetHeight(), image.Pixels());
    }
    void RegisterRGBAImage(int type, int width, int height, const unsigned char* pixels) override
    {
        images.insert(type, QImage(pixels, width, height, QImage::Format_RGBA8888).copy());
    }
    void ClearRegisteredImages() override { images.clear(); }
    void SetDelegate(IListBoxDelegate* value) override { delegate = value; }
    void SetList(const char* list, char separator, char typeSeparator) override
    {
        Clear();
        for (const QByteArray& entry : QByteArray(list).split(separator)) {
            const auto split = entry.indexOf(typeSeparator);
            QByteArray label = split < 0 ? entry : entry.first(split);
            Append(label.data(), split < 0 ? -1 : entry.sliced(split + 1).toInt());
        }
        InvalidateAll();
    }
    void SetOptions(ListOptions value) override
    {
        options = value;
        InvalidateAll();
    }
};
}

ListBox::ListBox() noexcept = default;
ListBox::~ListBox() noexcept = default;
std::unique_ptr<ListBox>
ListBox::Allocate()
{
    return std::make_unique<QuickListBox>();
}

Menu::Menu() noexcept
  : mid(nullptr)
{
}
void
Menu::CreatePopUp()
{
    Destroy();
    mid = new QuickMenu;
}
void
Menu::Destroy() noexcept
{
    delete static_cast<QuickMenu*>(mid);
    mid = nullptr;
}
void
Menu::Show(Point point, const Window& owner)
{
    const auto* host = window(owner.GetID());
    if (host && host->showMenu && mid)
        host->showMenu(host->toItem(point), static_cast<QuickMenu*>(mid)->entries);
    Destroy();
}

ColourRGBA
Platform::Chrome()
{
    return ColourRGBA(128, 128, 128);
}
ColourRGBA
Platform::ChromeHighlight()
{
    return ColourRGBA(192, 192, 192);
}
const char*
Platform::DefaultFont()
{
    static const QByteArray family = QGuiApplication::font().family().toUtf8();
    return family.constData();
}
int
Platform::DefaultFontSize()
{
    return qMax(1, QGuiApplication::font().pointSize());
}
unsigned int
Platform::DoubleClickTime()
{
    return QGuiApplication::styleHints()->mouseDoubleClickInterval();
}
void
Platform::DebugDisplay(const char* text) noexcept
{
    qWarning("Scintilla: %s", text);
}
void
Platform::DebugPrintf(const char* format, ...) noexcept
{
    char buffer[2000];
    va_list arguments;
    va_start(arguments, format);
    vsnprintf(buffer, sizeof(buffer), format, arguments);
    va_end(arguments);
    DebugDisplay(buffer);
}
bool
Platform::ShowAssertionPopUps(bool) noexcept
{
    return false;
}
void
Platform::Assert(const char* condition, const char* file, int line) noexcept
{
    qWarning("Scintilla assertion [%s] at %s:%d", condition, file, line);
}
}
