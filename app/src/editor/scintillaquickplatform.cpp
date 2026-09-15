// Adapted from Scintilla 5.6.6 qt/ScintillaEditBase/PlatQt.cpp.
// Copyright (c) 1990-2011, Scientific Toolworks, Inc.
// Author: Jason Haslam
// Additions Copyright (c) 2011 Archaeopteryx Software, Inc. d/b/a Wingware
// See ../../third_party/scintilla/License.txt for the upstream license.

#include "scintillaquickplatform_p.h"

#include "UniConversion.h"
#include <QFontMetricsF>
#include <QGuiApplication>
#include <QMouseEvent>
#include <QPaintEngine>
#include <QPainterPath>
#include <QQuickWindow>
#include <QScreen>
#include <QStyleHints>
#include <QTextLayout>
#include <QWheelEvent>
#include <QtMath>
#include <cstdarg>
#include <cstdio>

using namespace Scintilla;
namespace Scintilla::Internal {
static QFont::StyleStrategy
ChooseStrategy(FontQuality eff)
{
    switch (eff) {
        case FontQuality::QualityDefault:
            return QFont::PreferDefault;
        case FontQuality::QualityNonAntialiased:
            return QFont::NoAntialias;
        case FontQuality::QualityAntialiased:
            return QFont::PreferAntialias;
        case FontQuality::QualityLcdOptimized:
            return QFont::PreferAntialias;
        default:
            return QFont::PreferDefault;
    }
}

static QFont::Stretch
QStretchFromFontStretch(Scintilla::FontStretch stretch)
{
    switch (stretch) {
        case FontStretch::UltraCondensed:
            return QFont::Stretch::UltraCondensed;
        case FontStretch::ExtraCondensed:
            return QFont::Stretch::ExtraCondensed;
        case FontStretch::Condensed:
            return QFont::Stretch::Condensed;
        case FontStretch::SemiCondensed:
            return QFont::Stretch::SemiCondensed;
        case FontStretch::Normal:
            return QFont::Stretch::Unstretched;
        case FontStretch::SemiExpanded:
            return QFont::Stretch::SemiExpanded;
        case FontStretch::Expanded:
            return QFont::Stretch::Expanded;
        case FontStretch::ExtraExpanded:
            return QFont::Stretch::ExtraExpanded;
        case FontStretch::UltraExpanded:
            return QFont::Stretch::UltraExpanded;
        default:
            return QFont::Stretch::Unstretched;
    }
}

class FontAndCharacterSet : public Font
{
  public:
    CharacterSet characterSet = CharacterSet::Ansi;
    std::unique_ptr<QFont> pfont;
    explicit FontAndCharacterSet(const FontParameters& fp)
      : characterSet(fp.characterSet)
    {
        pfont = std::make_unique<QFont>();
        pfont->setStyleStrategy(ChooseStrategy(fp.extraFontFlag));
        pfont->setFamily(QString::fromUtf8(fp.faceName));
        pfont->setPointSizeF(fp.size);
        pfont->setWeight(static_cast<QFont::Weight>(std::clamp(static_cast<int>(fp.weight), 1, 1000)));
        pfont->setStretch(QStretchFromFontStretch(fp.stretch));
        pfont->setItalic(fp.italic);
    }
};

namespace {

const Supports SupportsQt[] = {
    Supports::LineDrawsFinal,
    Supports::FractionalStrokeWidth,
    Supports::TranslucentStroke,
    Supports::PixelModification,
};

const FontAndCharacterSet*
AsFontAndCharacterSet(const Font* f)
{
    return dynamic_cast<const FontAndCharacterSet*>(f);
}

QFont*
FontPointer(const Font* f)
{
    return AsFontAndCharacterSet(f)->pfont.get();
}

}

std::shared_ptr<Font>
Font::Allocate(const FontParameters& fp)
{
    return std::make_shared<FontAndCharacterSet>(fp);
}

SurfaceImpl::SurfaceImpl() = default;

SurfaceImpl::SurfaceImpl(int width, int height, SurfaceMode mode_, qreal scale_, const QPaintDevice* reference)
{
    if (width < 1)
        width = 1;
    if (height < 1)
        height = 1;
    deviceOwned = true;
    auto* image = new QImage(qCeil(width * scale_), qCeil(height * scale_), QImage::Format_ARGB32_Premultiplied);
    image->setDevicePixelRatio(scale_);
    image->setDotsPerMeterX(qRound(reference->logicalDpiX() / 0.0254));
    image->setDotsPerMeterY(qRound(reference->logicalDpiY() / 0.0254));
    image->fill(Qt::transparent);
    device = image;
    mode = mode_;
    scale = scale_;
}

SurfaceImpl::~SurfaceImpl()
{
    Clear();
}

void
SurfaceImpl::Clear()
{
    if (painterOwned && painter) {
        delete painter;
    }

    if (deviceOwned && device) {
        delete device;
    }
    device = nullptr;
    painter = nullptr;
    deviceOwned = false;
    painterOwned = false;
}

void
SurfaceImpl::Init(WindowID wid)
{
    Release();
    auto* host = window(wid);
    device = host ? &host->measurementDevice : nullptr;
    scale = host ? host->devicePixelRatio : 1.0;
}

void
SurfaceImpl::Init(SurfaceID sid, WindowID wid)
{
    Release();
    device = static_cast<QPaintDevice*>(sid);
    scale = wid ? window(wid)->devicePixelRatio : 1.0;
}

std::unique_ptr<Surface>
SurfaceImpl::AllocatePixMap(int width, int height)
{
    return std::make_unique<SurfaceImpl>(width, height, mode, scale, device);
}

void
SurfaceImpl::SetMode(SurfaceMode mode_)
{
    mode = mode_;
}

void
SurfaceImpl::Release() noexcept
{
    Clear();
}

int
SurfaceImpl::SupportsFeature(Supports feature) noexcept
{
    for (const Supports f : SupportsQt) {
        if (f == feature)
            return 1;
    }
    return 0;
}

bool
SurfaceImpl::Initialised()
{
    return device != nullptr;
}

void
SurfaceImpl::PenColour(ColourRGBA fore)
{
    QPen penOutline(QColorFromColourRGBA(fore));
    penOutline.setCapStyle(Qt::FlatCap);
    GetPainter()->setPen(penOutline);
}

void
SurfaceImpl::PenColourWidth(ColourRGBA fore, XYPOSITION strokeWidth)
{
    QPen penOutline(QColorFromColourRGBA(fore));
    penOutline.setCapStyle(Qt::FlatCap);
    penOutline.setJoinStyle(Qt::MiterJoin);
    penOutline.setWidthF(strokeWidth);
    GetPainter()->setPen(penOutline);
}

void
SurfaceImpl::BrushColour(ColourRGBA back)
{
    GetPainter()->setBrush(QBrush(QColorFromColourRGBA(back)));
}

void
SurfaceImpl::SetFont(const Font* font)
{
    const FontAndCharacterSet* pfacs = AsFontAndCharacterSet(font);
    if (pfacs && pfacs->pfont) {
        GetPainter()->setFont(*(pfacs->pfont));
    }
}

int
SurfaceImpl::LogPixelsY()
{
    return device->logicalDpiY();
}

int
SurfaceImpl::PixelDivisions()
{
    return std::max(1, qRound(scale));
}

int
SurfaceImpl::DeviceHeightFont(int points)
{
    return points;
}

void
SurfaceImpl::LineDraw(Point start, Point end, Stroke stroke)
{
    PenColourWidth(stroke.colour, stroke.width);
    QLineF line(start.x, start.y, end.x, end.y);
    GetPainter()->drawLine(line);
}

void
SurfaceImpl::PolyLine(const Point* pts, size_t npts, Stroke stroke)
{
    // TODO: set line joins and caps
    PenColourWidth(stroke.colour, stroke.width);
    std::vector<QPointF> qpts;
    std::transform(pts, pts + npts, std::back_inserter(qpts), QPointFFromPoint);
    GetPainter()->drawPolyline(&qpts[0], static_cast<int>(npts));
}

void
SurfaceImpl::Polygon(const Point* pts, size_t npts, FillStroke fillStroke)
{
    PenColourWidth(fillStroke.stroke.colour, fillStroke.stroke.width);
    BrushColour(fillStroke.fill.colour);

    std::vector<QPointF> qpts;
    std::transform(pts, pts + npts, std::back_inserter(qpts), QPointFFromPoint);

    GetPainter()->drawPolygon(&qpts[0], static_cast<int>(npts));
}

void
SurfaceImpl::RectangleDraw(PRectangle rc, FillStroke fillStroke)
{
    PenColourWidth(fillStroke.stroke.colour, fillStroke.stroke.width);
    BrushColour(fillStroke.fill.colour);
    const QRectF rect = QRectFFromPRect(rc.Inset(fillStroke.stroke.width / 2));
    GetPainter()->drawRect(rect);
}

void
SurfaceImpl::RectangleFrame(PRectangle rc, Stroke stroke)
{
    PenColourWidth(stroke.colour, stroke.width);
    // Default QBrush is Qt::NoBrush so does not fill
    GetPainter()->setBrush(QBrush());
    const QRectF rect = QRectFFromPRect(rc.Inset(stroke.width / 2));
    GetPainter()->drawRect(rect);
}

void
SurfaceImpl::FillRectangle(PRectangle rc, Fill fill)
{
    GetPainter()->fillRect(QRectFFromPRect(rc), QColorFromColourRGBA(fill.colour));
}

void
SurfaceImpl::FillRectangleAligned(PRectangle rc, Fill fill)
{
    FillRectangle(PixelAlign(rc, PixelDivisions()), fill);
}

void
SurfaceImpl::FillRectangle(PRectangle rc, Surface& surfacePattern)
{
    // Tile pattern over rectangle
    SurfaceImpl* surface = dynamic_cast<SurfaceImpl*>(&surfacePattern);
    const auto* image = static_cast<QImage*>(surface->GetPaintDevice());
    GetPainter()->fillRect(QRectFFromPRect(rc), QBrush(*image));
}

void
SurfaceImpl::RoundedRectangle(PRectangle rc, FillStroke fillStroke)
{
    PenColourWidth(fillStroke.stroke.colour, fillStroke.stroke.width);
    BrushColour(fillStroke.fill.colour);
    GetPainter()->drawRoundedRect(QRectFFromPRect(rc), 3.0f, 3.0f);
}

void
SurfaceImpl::AlphaRectangle(PRectangle rc, XYPOSITION cornerSize, FillStroke fillStroke)
{
    QColor qFill = QColorFromColourRGBA(fillStroke.fill.colour);
    QBrush brushFill(qFill);
    GetPainter()->setBrush(brushFill);
    if (fillStroke.fill.colour == fillStroke.stroke.colour) {
        painter->setPen(Qt::NoPen);
        QRectF rect = QRectFFromPRect(rc);
        if (cornerSize > 0.0f) {
            // A radius of 1 shows no curve so add 1
            qreal radius = cornerSize + 1;
            GetPainter()->drawRoundedRect(rect, radius, radius);
        } else {
            GetPainter()->fillRect(rect, brushFill);
        }
    } else {
        QColor qOutline = QColorFromColourRGBA(fillStroke.stroke.colour);
        QPen penOutline(qOutline);
        penOutline.setWidthF(fillStroke.stroke.width);
        GetPainter()->setPen(penOutline);

        QRectF rect = QRectFFromPRect(rc.Inset(fillStroke.stroke.width / 2));
        if (cornerSize > 0.0f) {
            // A radius of 1 shows no curve so add 1
            qreal radius = cornerSize + 1;
            GetPainter()->drawRoundedRect(rect, radius, radius);
        } else {
            GetPainter()->drawRect(rect);
        }
    }
}

void
SurfaceImpl::GradientRectangle(PRectangle rc, const std::vector<ColourStop>& stops, GradientOptions options)
{
    QRectF rect = QRectFFromPRect(rc);
    QLinearGradient linearGradient;
    switch (options) {
        case GradientOptions::leftToRight:
            linearGradient = QLinearGradient(rc.left, rc.top, rc.right, rc.top);
            break;
        case GradientOptions::topToBottom:
        default:
            linearGradient = QLinearGradient(rc.left, rc.top, rc.left, rc.bottom);
            break;
    }
    linearGradient.setSpread(QGradient::RepeatSpread);
    for (const ColourStop& stop : stops) {
        linearGradient.setColorAt(stop.position, QColorFromColourRGBA(stop.colour));
    }
    QBrush brush = QBrush(linearGradient);
    GetPainter()->fillRect(rect, brush);
}

void
SurfaceImpl::DrawRGBAImage(PRectangle rc, int width, int height, const unsigned char* pixelsImage)
{
    const QImage image(pixelsImage, width, height, QImage::Format_RGBA8888);
    GetPainter()->drawImage(QRectFFromPRect(rc), image);
}

void
SurfaceImpl::Ellipse(PRectangle rc, FillStroke fillStroke)
{
    PenColourWidth(fillStroke.stroke.colour, fillStroke.stroke.width);
    BrushColour(fillStroke.fill.colour);
    const QRectF rect = QRectFFromPRect(rc.Inset(fillStroke.stroke.width / 2));
    GetPainter()->drawEllipse(rect);
}

void
SurfaceImpl::Stadium(PRectangle rc, FillStroke fillStroke, Ends ends)
{
    const XYPOSITION halfStroke = fillStroke.stroke.width / 2.0f;
    const XYPOSITION radius = rc.Height() / 2.0f - halfStroke;
    PRectangle rcInner = rc;
    rcInner.left += radius;
    rcInner.right -= radius;
    const XYPOSITION arcHeight = rc.Height() - fillStroke.stroke.width;

    PenColourWidth(fillStroke.stroke.colour, fillStroke.stroke.width);
    BrushColour(fillStroke.fill.colour);

    QPainterPath path;

    const Ends leftSide = static_cast<Ends>(static_cast<unsigned int>(ends) & 0xfu);
    const Ends rightSide = static_cast<Ends>(static_cast<unsigned int>(ends) & 0xf0u);
    switch (leftSide) {
        case Ends::leftFlat:
            path.moveTo(rc.left + halfStroke, rc.top + halfStroke);
            path.lineTo(rc.left + halfStroke, rc.bottom - halfStroke);
            break;
        case Ends::leftAngle:
            path.moveTo(rcInner.left + halfStroke, rc.top + halfStroke);
            path.lineTo(rc.left + halfStroke, rc.Centre().y);
            path.lineTo(rcInner.left + halfStroke, rc.bottom - halfStroke);
            break;
        case Ends::semiCircles:
        default:
            path.moveTo(rcInner.left + halfStroke, rc.top + halfStroke);
            QRectF rectangleArc(rc.left + halfStroke, rc.top + halfStroke, arcHeight, arcHeight);
            path.arcTo(rectangleArc, 90, 180);
            break;
    }

    switch (rightSide) {
        case Ends::rightFlat:
            path.lineTo(rc.right - halfStroke, rc.bottom - halfStroke);
            path.lineTo(rc.right - halfStroke, rc.top + halfStroke);
            break;
        case Ends::rightAngle:
            path.lineTo(rcInner.right - halfStroke, rc.bottom - halfStroke);
            path.lineTo(rc.right - halfStroke, rc.Centre().y);
            path.lineTo(rcInner.right - halfStroke, rc.top + halfStroke);
            break;
        case Ends::semiCircles:
        default:
            path.lineTo(rcInner.right - halfStroke, rc.bottom - halfStroke);
            QRectF rectangleArc(rc.right - arcHeight - halfStroke, rc.top + halfStroke, arcHeight, arcHeight);
            path.arcTo(rectangleArc, 270, 180);
            break;
    }

    // Close the path to enclose it for stroking and for filling, then draw it
    path.closeSubpath();
    GetPainter()->drawPath(path);
}

void
SurfaceImpl::Copy(PRectangle rc, Point from, Surface& surfaceSource)
{
    SurfaceImpl* source = dynamic_cast<SurfaceImpl*>(&surfaceSource);
    const auto* image = static_cast<QImage*>(source->GetPaintDevice());
    const qreal ratio = image->devicePixelRatio();
    GetPainter()->drawImage(
      QRectFFromPRect(rc), *image, QRectF(from.x * ratio, from.y * ratio, rc.Width() * ratio, rc.Height() * ratio));
}

std::unique_ptr<IScreenLineLayout>
SurfaceImpl::Layout(const IScreenLine*)
{
    return {};
}

void
SurfaceImpl::DrawTextNoClip(PRectangle rc,
                            const Font* font,
                            XYPOSITION ybase,
                            std::string_view text,
                            ColourRGBA fore,
                            ColourRGBA back)
{
    SetFont(font);
    PenColour(fore);

    GetPainter()->setBackground(QColorFromColourRGBA(back));
    GetPainter()->setBackgroundMode(Qt::OpaqueMode);
    QString su = QString::fromUtf8(text.data(), static_cast<qsizetype>(text.size()));
    GetPainter()->drawText(QPointF(rc.left, ybase), su);
}

void
SurfaceImpl::DrawTextClipped(PRectangle rc,
                             const Font* font,
                             XYPOSITION ybase,
                             std::string_view text,
                             ColourRGBA fore,
                             ColourRGBA back)
{
    SetClip(rc);
    DrawTextNoClip(rc, font, ybase, text, fore, back);
    PopClip();
}

void
SurfaceImpl::DrawTextTransparent(PRectangle rc,
                                 const Font* font,
                                 XYPOSITION ybase,
                                 std::string_view text,
                                 ColourRGBA fore)
{
    SetFont(font);
    PenColour(fore);

    GetPainter()->setBackgroundMode(Qt::TransparentMode);
    QString su = QString::fromUtf8(text.data(), static_cast<qsizetype>(text.size()));
    GetPainter()->drawText(QPointF(rc.left, ybase), su);
}

void
SurfaceImpl::SetClip(PRectangle rc)
{
    GetPainter()->save();
    GetPainter()->setClipRect(QRectFFromPRect(rc), Qt::IntersectClip);
}

void
SurfaceImpl::PopClip()
{
    GetPainter()->restore();
}

void
SurfaceImpl::MeasureWidths(const Font* font, std::string_view text, XYPOSITION* positions)
{
    MeasureWidthsUTF8(font, text, positions);
}

XYPOSITION
SurfaceImpl::WidthText(const Font* font, std::string_view text)
{
    QFontMetricsF metrics(*FontPointer(font), device);
    QString su = QString::fromUtf8(text.data(), static_cast<qsizetype>(text.size()));
    return metrics.horizontalAdvance(su);
}

void
SurfaceImpl::DrawTextNoClipUTF8(PRectangle rc,
                                const Font* font,
                                XYPOSITION ybase,
                                std::string_view text,
                                ColourRGBA fore,
                                ColourRGBA back)
{
    DrawTextNoClip(rc, font, ybase, text, fore, back);
}

void
SurfaceImpl::DrawTextClippedUTF8(PRectangle rc,
                                 const Font* font,
                                 XYPOSITION ybase,
                                 std::string_view text,
                                 ColourRGBA fore,
                                 ColourRGBA back)
{
    DrawTextClipped(rc, font, ybase, text, fore, back);
}

void
SurfaceImpl::DrawTextTransparentUTF8(PRectangle rc,
                                     const Font* font,
                                     XYPOSITION ybase,
                                     std::string_view text,
                                     ColourRGBA fore)
{
    DrawTextTransparent(rc, font, ybase, text, fore);
}

void
SurfaceImpl::MeasureWidthsUTF8(const Font* font, std::string_view text, XYPOSITION* positions)
{
    if (!font)
        return;
    QString su = QString::fromUtf8(text.data(), static_cast<int>(text.length()));
    QTextLayout tlay(su, *FontPointer(font), GetPaintDevice());
    tlay.beginLayout();
    QTextLine tl = tlay.createLine();
    tlay.endLayout();
    int fit = su.size();
    int ui = 0;
    size_t i = 0;
    while (ui < fit) {
        const unsigned char uch = text[i];
        const unsigned int byteCount = UTF8BytesOfLead[uch];
        const int codeUnits = UTF16LengthFromUTF8ByteCount(byteCount);
        qreal xPosition = tl.cursorToX(ui + codeUnits);
        for (size_t bytePos = 0; (bytePos < byteCount) && (i < text.length()); bytePos++) {
            positions[i++] = xPosition;
        }
        ui += codeUnits;
    }
    XYPOSITION lastPos = 0;
    if (i > 0)
        lastPos = positions[i - 1];
    while (i < text.length()) {
        positions[i++] = lastPos;
    }
}

XYPOSITION
SurfaceImpl::WidthTextUTF8(const Font* font, std::string_view text)
{
    return WidthText(font, text);
}

XYPOSITION
SurfaceImpl::Ascent(const Font* font)
{
    QFontMetricsF metrics(*FontPointer(font), device);
    return metrics.ascent();
}

XYPOSITION
SurfaceImpl::Descent(const Font* font)
{
    QFontMetricsF metrics(*FontPointer(font), device);
    return metrics.descent();
}

XYPOSITION
SurfaceImpl::InternalLeading(const Font* /* font */)
{
    return 0;
}

XYPOSITION
SurfaceImpl::Height(const Font* font)
{
    QFontMetricsF metrics(*FontPointer(font), device);
    return metrics.height();
}

XYPOSITION
SurfaceImpl::AverageCharWidth(const Font* font)
{
    QFontMetricsF metrics(*FontPointer(font), device);
    return metrics.averageCharWidth();
}

void
SurfaceImpl::FlushCachedState()
{
    if (device->paintingActive()) {
        GetPainter()->setPen(QPen());
        GetPainter()->setBrush(QBrush());
    }
}

void
SurfaceImpl::FlushDrawing()
{
}

QPaintDevice*
SurfaceImpl::GetPaintDevice()
{
    return device;
}

QPainter*
SurfaceImpl::GetPainter()
{
    Q_ASSERT(device);
    if (!painter) {
        if (device->paintingActive()) {
            painter = device->paintEngine()->painter();
        } else {
            painterOwned = true;
            painter = new QPainter(device);
        }

        // Set text antialiasing unconditionally.
        // The font's style strategy will override.
        painter->setRenderHint(QPainter::TextAntialiasing, true);

        painter->setRenderHint(QPainter::Antialiasing, true);
    }

    return painter;
}

std::unique_ptr<Surface>
Surface::Allocate(Technology)
{
    return std::make_unique<SurfaceImpl>();
}

QFont
FontForQuick(const Font* font)
{
    return *FontPointer(font);
}

}
