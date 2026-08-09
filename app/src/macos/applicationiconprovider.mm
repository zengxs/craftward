#include "../applicationiconprovider.h"

#import <AppKit/AppKit.h>

#include <QImage>
#include <QSize>
#include <QString>

namespace {

constexpr CGFloat applicationIconDimension = 512.0;

QImage
loadApplicationIcon()
{
    @autoreleasepool {
        NSImage* applicationIcon = NSApplication.sharedApplication.applicationIconImage;
        if (applicationIcon == nil)
            return {};

        NSRect proposedRect = NSMakeRect(0.0, 0.0, applicationIconDimension, applicationIconDimension);
        CGImageRef iconImage = [applicationIcon CGImageForProposedRect:&proposedRect context:nil hints:nil];
        if (iconImage == nullptr)
            return {};

        NSBitmapImageRep* representation = [[NSBitmapImageRep alloc] initWithCGImage:iconImage];
        NSData* pngData = [representation representationUsingType:NSBitmapImageFileTypePNG properties:@{}];
        if (pngData == nil)
            return {};

        return QImage::fromData(
          static_cast<const uchar*>(pngData.bytes), static_cast<qsizetype>(pngData.length), "PNG");
    }
}

class MacApplicationIconProvider final : public QQuickImageProvider
{
  public:
    MacApplicationIconProvider()
      : QQuickImageProvider(QQuickImageProvider::Image)
      , image_(loadApplicationIcon())
    {
    }

    QImage requestImage(const QString& id, QSize* size, const QSize& requestedSize) override
    {
        if (id != QStringLiteral("app") || image_.isNull())
            return {};

        if (size != nullptr)
            *size = image_.size();

        if (!requestedSize.isValid() || requestedSize == image_.size())
            return image_;

        return image_.scaled(requestedSize, Qt::KeepAspectRatio, Qt::SmoothTransformation);
    }

  private:
    QImage image_;
};

} // namespace

std::unique_ptr<QQuickImageProvider>
createApplicationIconProvider()
{
    return std::make_unique<MacApplicationIconProvider>();
}
