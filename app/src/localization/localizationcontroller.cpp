// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "localization/localizationcontroller.h"

#include <QCoreApplication>
#include <QLocale>
#include <QQmlEngine>
#include <QSettings>
#include <QTranslator>

#include <array>
#include <utility>

namespace {
constexpr auto languagePreferenceKey = "ui/language";
constexpr auto systemPreferenceValue = "system";

struct LanguageDescriptor
{
    LocalizationController::LanguagePreference preference;
    QLocale::Language language;
    QLocale::Script script;
    const char* settingsValue;
    const char* uiLanguage;
    const char* catalog;
};

constexpr std::array languageDescriptors{
    LanguageDescriptor{
      LocalizationController::English,
      QLocale::English,
      QLocale::AnyScript,
      "en",
      "en",
      "craftward_en.qm",
    },
    LanguageDescriptor{
      LocalizationController::SimplifiedChinese,
      QLocale::Chinese,
      QLocale::SimplifiedHanScript,
      "zh-Hans",
      "zh-Hans",
      "craftward_zh_CN.qm",
    },
};

const LanguageDescriptor*
descriptorForPreference(LocalizationController::LanguagePreference preference)
{
    for (const LanguageDescriptor& descriptor : languageDescriptors) {
        if (descriptor.preference == preference)
            return &descriptor;
    }
    return nullptr;
}

const LanguageDescriptor*
descriptorForUiLanguage(const QString& language)
{
    for (const LanguageDescriptor& descriptor : languageDescriptors) {
        if (language == QLatin1StringView(descriptor.uiLanguage))
            return &descriptor;
    }
    return nullptr;
}

bool
isValidPreference(LocalizationController::LanguagePreference preference)
{
    return preference == LocalizationController::SystemLanguage || descriptorForPreference(preference) != nullptr;
}
}

LocalizationController::LocalizationController(QQmlEngine& engine, QSettings& settings, QObject* parent)
  : QObject(parent)
  , engine_(engine)
  , settings_(settings)
{
    bool valid = false;
    languagePreference_ = preferenceFromSettings(settings_.value(languagePreferenceKey).toString(), &valid);
    if (!valid)
        settings_.setValue(languagePreferenceKey, preferenceForSettings(languagePreference_));
    applyEffectiveLanguage(resolveEffectiveLanguage(languagePreference_));
}

LocalizationController::~LocalizationController()
{
    if (translator_ != nullptr)
        QCoreApplication::removeTranslator(translator_.get());
}

LocalizationController::LanguagePreference
LocalizationController::languagePreference() const
{
    return languagePreference_;
}

void
LocalizationController::setLanguagePreference(LanguagePreference preference)
{
    if (!isValidPreference(preference) || languagePreference_ == preference)
        return;

    languagePreference_ = preference;
    settings_.setValue(languagePreferenceKey, preferenceForSettings(preference));
    emit languagePreferenceChanged();
    applyEffectiveLanguage(resolveEffectiveLanguage(preference));
}

QString
LocalizationController::effectiveLanguage() const
{
    return effectiveLanguage_;
}

LocalizationController::LanguagePreference
LocalizationController::preferenceFromSettings(const QString& value, bool* valid)
{
    if (valid != nullptr)
        *valid = true;
    if (value.isEmpty() || value == QLatin1StringView(systemPreferenceValue))
        return SystemLanguage;
    for (const LanguageDescriptor& descriptor : languageDescriptors) {
        if (value == QLatin1StringView(descriptor.settingsValue))
            return descriptor.preference;
    }
    if (valid != nullptr)
        *valid = false;
    return SystemLanguage;
}

QString
LocalizationController::preferenceForSettings(LanguagePreference preference)
{
    const LanguageDescriptor* descriptor = descriptorForPreference(preference);
    return descriptor == nullptr ? QString::fromLatin1(systemPreferenceValue)
                                 : QString::fromLatin1(descriptor->settingsValue);
}

QString
LocalizationController::resolveEffectiveLanguage(LanguagePreference preference)
{
    if (const LanguageDescriptor* descriptor = descriptorForPreference(preference); descriptor != nullptr)
        return QString::fromLatin1(descriptor->uiLanguage);

    for (const QString& language : QLocale::system().uiLanguages()) {
        const QLocale locale(language);
        for (const LanguageDescriptor& descriptor : languageDescriptors) {
            if (locale.language() == descriptor.language &&
                (descriptor.script == QLocale::AnyScript || locale.script() == descriptor.script)) {
                return QString::fromLatin1(descriptor.uiLanguage);
            }
        }
    }
    return QString::fromLatin1(descriptorForPreference(English)->uiLanguage);
}

void
LocalizationController::applyEffectiveLanguage(const QString& language)
{
    if (translator_ != nullptr && effectiveLanguage_ == language)
        return;

    const LanguageDescriptor* descriptor = descriptorForUiLanguage(language);
    if (descriptor == nullptr)
        qFatal("The effective Craftward language is unsupported: %s", qUtf8Printable(language));
    const QString catalog = QString::fromLatin1(descriptor->catalog);
    auto translator = std::make_unique<QTranslator>();
    if (!translator->load(QStringLiteral(":/i18n/%1").arg(catalog)))
        qFatal("The embedded Craftward translation catalog could not be loaded: %s", qUtf8Printable(catalog));
    if (!QCoreApplication::installTranslator(translator.get()))
        qFatal("The Craftward translation catalog could not be installed: %s", qUtf8Printable(catalog));

    if (translator_ != nullptr)
        QCoreApplication::removeTranslator(translator_.get());
    translator_ = std::move(translator);
    effectiveLanguage_ = language;
    engine_.setUiLanguage(language);
    engine_.retranslate();
    emit effectiveLanguageChanged();
}
