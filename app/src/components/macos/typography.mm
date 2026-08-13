// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "typography.h"

#import <CoreText/CoreText.h>
#import <Foundation/Foundation.h>

#include <QDebug>
#include <QFontDatabase>

namespace {

constexpr qreal codeFontPointSize = 13.0;
constexpr qreal codeLineHeightScaleValue = 1.25;

bool
registerNativeFont(NSURL* fontUrl)
{
    CFErrorRef registrationError = nullptr;
    if (CTFontManagerRegisterFontsForURL((__bridge CFURLRef)fontUrl, kCTFontManagerScopeProcess, &registrationError)) {
        return true;
    }

    const bool alreadyRegistered = registrationError &&
                                   CFEqual(CFErrorGetDomain(registrationError), kCTFontManagerErrorDomain) &&
                                   CFErrorGetCode(registrationError) == kCTFontManagerErrorAlreadyRegistered;
    if (!alreadyRegistered) {
        const CFStringRef description = registrationError ? CFErrorCopyDescription(registrationError) : nullptr;
        qWarning() << "Failed to register native application font" << QString::fromNSString(fontUrl.lastPathComponent)
                   << (description ? QString::fromCFString(description) : QString());
        if (description)
            CFRelease(description);
    }

    if (registrationError)
        CFRelease(registrationError);
    return alreadyRegistered;
}

QString
loadApplicationMonoFont()
{
    NSArray<NSURL*>* fontUrls = [NSBundle.mainBundle URLsForResourcesWithExtension:@"ttf" subdirectory:@"fonts/lilex"];
    if (fontUrls.count == 0) {
        qWarning() << "Bundled application fonts are missing";
        return {};
    }

    QString codeFontFamily;
    bool codeFontRegisteredNatively = false;
    bool codeFontRegisteredWithQt = false;

    for (NSURL* fontUrl in fontUrls) {
        const bool registeredNatively = registerNativeFont(fontUrl);

        const int fontId = QFontDatabase::addApplicationFont(QString::fromNSString(fontUrl.path));
        if (fontId < 0)
            qWarning() << "Failed to register Qt application font" << QString::fromNSString(fontUrl.path);

        if ([fontUrl.lastPathComponent isEqualToString:@"Lilex-Medium.ttf"]) {
            codeFontRegisteredNatively = registeredNatively;
            if (fontId >= 0) {
                codeFontFamily = QFontDatabase::applicationFontFamilies(fontId).value(0);
                codeFontRegisteredWithQt = !codeFontFamily.isEmpty();
            }
        }
    }

    if (!codeFontRegisteredNatively || !codeFontRegisteredWithQt) {
        qWarning() << "Required bundled application font is unavailable: Lilex-Medium.ttf";
        return {};
    }

    return codeFontFamily;
}

} // namespace

Typography::Typography(QObject* parent)
  : QObject(parent)
  , m_monoFamily(loadApplicationMonoFont())
{
    if (m_monoFamily.isEmpty())
        m_monoFamily = QFontDatabase::systemFont(QFontDatabase::FixedFont).family();

    m_codeFont.setFamily(m_monoFamily);
    m_codeFont.setPointSizeF(codeFontPointSize);
    m_codeFont.setWeight(QFont::Medium);
}

QString
Typography::monoFamily() const
{
    return m_monoFamily;
}

QFont
Typography::codeFont() const
{
    return m_codeFont;
}

qreal
Typography::codeLineHeightScale() const
{
    return codeLineHeightScaleValue;
}
