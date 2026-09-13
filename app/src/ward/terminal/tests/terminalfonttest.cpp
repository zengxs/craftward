// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later
// Exercise Contour's real font locator and rasterizer with the application's fonts.

#include "ward/terminal/terminalcontroller.h"

#include <contour/config/Config.hpp>
#include <text_shaper/FontLocatorProvider.hpp>
#include <text_shaper/OpenShaper.hpp>

#include <QCryptographicHash>
#include <QFont>
#include <QGuiApplication>
#include <QRawFont>
#include <QSettings>
#include <QTemporaryDir>

#include <ft2build.h>
#include FT_FREETYPE_H

#include <array>
#include <iostream>

struct FontStyle
{
    const char* name;
    const char* postscript;
    bool bold;
    bool italic;
};

constexpr std::array styles{
    FontStyle{ "regular", "Menlo-Regular", false, false },
    FontStyle{ "bold", "Menlo-Bold", true, false },
    FontStyle{ "italic", "Menlo-Italic", false, true },
    FontStyle{ "bold italic", "Menlo-BoldItalic", true, true },
};

QByteArray
rasterSignature(text::OpenShaper& shaper, text::FontKey font)
{
    QCryptographicHash signature(QCryptographicHash::Sha256);
    for (char32_t codepoint : U"Hamburgefontsiv0123456789") {
        if (!codepoint)
            continue;
        const auto glyph = shaper.shape(font, codepoint).value();
        const auto bitmap = shaper.rasterize(glyph.glyph, text::RenderMode::Gray).value();
        const auto bytes = QByteArrayView(reinterpret_cast<const char*>(bitmap.bitmap.data()),
                                          static_cast<qsizetype>(bitmap.bitmap.size()));
        signature.addData(bytes);
    }
    return signature.result().toHex();
}

int
checkSystemFallback(text::FontLocator& locator, const QString& configPath)
{
    contour::config::Config config;
    contour::config::loadConfigFromFile(config, configPath.toStdString());
    const auto* profile = config.profile("main");
    if (!profile) {
        std::cerr << "FAIL: the supplied configuration has no main profile.\n";
        return 1;
    }
    bool passed = locator.resolve({}).empty();
    const auto& emojiDescription = profile->fonts.value().emoji;
    std::cout << "Configured emoji family: " << emojiDescription.familyName << '\n';
    const auto emojiSources = locator.locate(emojiDescription);
    FT_Library ft = nullptr;
    if (FT_Init_FreeType(&ft))
        return 1;
    const auto contains = [&](const text::FontSourceList& sources, char32_t character, bool needsColor) {
        if (sources.empty())
            return false;
        const auto* path = std::get_if<text::FontPath>(&sources.front());
        FT_Face face = nullptr;
        if (!path || FT_New_Face(ft, path->value.c_str(), path->collectionIndex, &face))
            return false;
        const bool covered = FT_Get_Char_Index(face, character) != 0 && (!needsColor || FT_HAS_COLOR(face));
        std::cout << "Resolved U+" << std::hex << static_cast<unsigned>(character) << std::dec << " to "
                  << FT_Get_Postscript_Name(face) << ": covered=" << covered << '\n';
        FT_Done_Face(face);
        return covered;
    };
    passed &= contains(emojiSources, U'🍺', true);
    for (const char32_t character : { U'⠋', U'🍺' }) {
        const auto sources = locator.resolve(gsl::span(&character, 1));
        passed &= contains(sources, character, character == U'🍺');
    }
    FT_Done_FreeType(ft);

    for (const auto* family : { "Fira Code", "Menlo" }) {
        text::OpenShaper shaper({ 192, 192 }, locator);
        // Force the real shaping path to resolve symbols beyond its short initial cascade.
        shaper.setFontFallbackLimit(1);
        text::FontDescription description;
        description.familyName = family;
        const auto primary = shaper.loadFont(description, { 10.0 });
        if (!primary)
            return 1;
        for (const char32_t character : { U'⠋', U'🍺' }) {
            std::u32string input(1, character);
            std::array<unsigned, 1> clusters{ 0 };
            text::ShapeResult shaped;
            shaper.shape(*primary,
                         input,
                         clusters,
                         unicode::Script::Common,
                         character == U'🍺' ? unicode::PresentationStyle::Emoji : unicode::PresentationStyle::Text,
                         shaped);
            // A replacement glyph can be nonzero: it must also belong to a fallback face.
            bool valid =
              shaped.size() == 1 && shaped.front().glyph.font != *primary && shaped.front().glyph.index.value != 0;
            if (valid) {
                const auto raster = shaper.rasterize(shaped.front().glyph, text::RenderMode::Gray);
                valid = raster && !raster->bitmap.empty() &&
                        (character != U'🍺' || raster->format == text::BitmapFormat::RGBA);
            }
            std::cout << family << " U+" << std::hex << static_cast<unsigned>(character) << std::dec
                      << " with fallback limit 1: " << (valid ? "PASS" : "FAIL") << '\n';
            passed &= valid;
        }
    }
    std::cout << (passed ? "PASS: configured color emoji font and system symbol fallback.\n"
                         : "FAIL: configured color emoji font or system symbol fallback.\n");
    return passed ? 0 : 1;
}

int
main(int argc, char** argv)
{
    QGuiApplication application(argc, argv);
    QTemporaryDir directory;
    if (!directory.isValid())
        return 1;
    QSettings settings(directory.filePath("settings.ini"), QSettings::IniFormat);
    // Exercise production registration in both CoreText and Qt's font database.
    TerminalController controller(settings);
    auto& locator = text::FontLocatorProvider::get().native();
    const auto arguments = application.arguments();
    const auto fallbackIndex = arguments.indexOf("--fallback");
    if (fallbackIndex >= 0) {
        if (fallbackIndex + 1 >= arguments.size()) {
            std::cerr << "Usage: CraftwardTerminalFontTest --fallback <contour.yml>\n";
            return 2;
        }
        return checkSystemFallback(locator, arguments.at(fallbackIndex + 1));
    }
    FT_Library freetype = nullptr;
    if (FT_Init_FreeType(&freetype))
        return 1;
    std::array<QByteArray, styles.size()> baseline;
    bool passed = true;

    for (int order = 0; order != 2; ++order) {
        text::OpenShaper shaper({ 192, 192 }, locator);
        shaper.setFontFallbackLimit(0);
        std::array<text::FontKey, styles.size()> keys;
        std::array<QByteArray, styles.size()> signatures;
        std::cout << (order ? "Reverse" : "Forward") << " load order:\n";
        for (size_t step = 0; step != styles.size(); ++step) {
            const auto index = order ? styles.size() - step - 1 : step;
            const auto& style = styles[index];
            text::FontDescription description;
            description.familyName = "Menlo";
            description.weight = style.bold ? text::FontWeight::Bold : text::FontWeight::Normal;
            description.slant = style.italic ? text::FontSlant::Italic : text::FontSlant::Normal;
            description.spacing = text::FontSpacing::Mono;
            description.fontFallback = text::FontFallbackNone{};
            const auto sources = locator.locate(description);
            const auto& source = std::get<text::FontPath>(sources.at(0));
            FT_Face face = nullptr;
            if (FT_New_Face(freetype, source.value.c_str(), source.collectionIndex, &face))
                return 1;
            const std::string postscript = FT_Get_Postscript_Name(face);
            std::cout << style.name << ": index=" << source.collectionIndex << " postscript=" << postscript << '\n';
            passed &= postscript == style.postscript;
            FT_Done_Face(face);

            const auto font = shaper.loadFont(description, { 10.0 }).value();
            keys[index] = font;
            signatures[index] = rasterSignature(shaper, font);
            std::cout << "  key=" << font.value << " raster sha256=" << signatures[index].constData() << '\n';
            if (order)
                passed &= signatures[index] == baseline[index];
            else
                baseline[index] = signatures[index];

            const auto resized = shaper.resizeFont(font, { 12.0 });
            const auto loaded = shaper.loadFont(description, { 12.0 }).value();
            passed &= resized == loaded;
            passed &= rasterSignature(shaper, resized) != signatures[index];

            QFont qtFont("Menlo");
            qtFont.setPixelSize(27);
            qtFont.setWeight(style.bold ? QFont::Bold : QFont::Normal);
            qtFont.setItalic(style.italic);
            const auto raw = QRawFont::fromFont(qtFont);
            std::cout << "  Qt resolved=" << raw.familyName().toStdString() << " / " << raw.styleName().toStdString()
                      << " weight=" << raw.weight() << '\n';
        }
        for (size_t a = 0; a != styles.size(); ++a) {
            for (size_t b = a + 1; b != styles.size(); ++b) {
                if (keys[a] == keys[b] || signatures[a] == signatures[b]) {
                    std::cerr << "FAIL: " << styles[a].name << " and " << styles[b].name
                              << " alias the same face or glyph rasters.\n";
                    passed = false;
                }
            }
        }
    }
    {
        text::OpenShaper shaper({ 192, 192 }, locator);
        shaper.setFontFallbackLimit(0);
        std::array<QByteArray, 2> signatures;
        for (size_t style = 0; style != 2; ++style) {
            text::FontDescription description;
            description.familyName = "Fira Code";
            description.weight = style ? text::FontWeight::Bold : text::FontWeight::Normal;
            description.spacing = text::FontSpacing::Mono;
            description.fontFallback = text::FontFallbackNone{};
            const auto sources = locator.locate(description);
            const auto& source = std::get<text::FontPath>(sources.at(0));
            FT_Face face = nullptr;
            if (FT_New_Face(freetype, source.value.c_str(), source.collectionIndex, &face))
                return 1;
            const std::string postscript = FT_Get_Postscript_Name(face);
            std::cout << "Fira Code " << (style ? "bold" : "regular") << ": path=" << source.value
                      << " postscript=" << postscript << '\n';
            // A system variable font with the same family may precede the registered static files.
            // Both must resolve the requested named face and keep regular/bold rasters distinct.
            passed &= postscript == (style ? "FiraCode-Bold" : "FiraCode-Regular") ||
                      postscript == (style ? "FiraCodeRoman-Bold" : "FiraCodeRoman-Regular");
            FT_Done_Face(face);
            const auto font = shaper.loadFont(description, { 10.0 }).value();
            signatures[style] = rasterSignature(shaper, font);
            QFont qtFont("Fira Code");
            qtFont.setPixelSize(27);
            qtFont.setWeight(style ? QFont::Bold : QFont::Normal);
            const auto raw = QRawFont::fromFont(qtFont);
            std::cout << "  Qt resolved=" << raw.familyName().toStdString() << " / " << raw.styleName().toStdString()
                      << " weight=" << raw.weight() << '\n';
            passed &= raw.familyName() == "Fira Code";
            passed &= raw.weight() == (style ? QFont::Bold : QFont::Normal);
        }
        passed &= signatures[0] != signatures[1];
    }
    FT_Done_FreeType(freetype);
    if (!passed) {
        std::cerr << "FAIL: font selection, face identity, load order, or resize mismatch.\n";
        return 1;
    }
    std::cout << "PASS: four correct faces, distinct rasters, both load orders, and resize identity.\n";
    std::cout << "PASS: Fira Code regular and bold resolve in both Contour and Qt.\n";
}
