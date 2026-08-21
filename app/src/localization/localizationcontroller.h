// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QObject>
#include <QString>
#include <QtQml/qqmlregistration.h>

#include <memory>

class QQmlEngine;
class QSettings;
class QTranslator;

class LocalizationController final : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("LocalizationController is provided by the application.")
    Q_PROPERTY(LanguagePreference languagePreference READ languagePreference WRITE setLanguagePreference NOTIFY
                 languagePreferenceChanged)
    Q_PROPERTY(QString effectiveLanguage READ effectiveLanguage NOTIFY effectiveLanguageChanged)

  public:
    enum LanguagePreference
    {
        SystemLanguage,
        English,
        SimplifiedChinese,
    };
    Q_ENUM(LanguagePreference)

    explicit LocalizationController(QQmlEngine& engine, QSettings& settings, QObject* parent = nullptr);
    ~LocalizationController() override;

    [[nodiscard]] LanguagePreference languagePreference() const;
    void setLanguagePreference(LanguagePreference preference);
    [[nodiscard]] QString effectiveLanguage() const;

  signals:
    void languagePreferenceChanged();
    void effectiveLanguageChanged();

  private:
    [[nodiscard]] static LanguagePreference preferenceFromSettings(const QString& value, bool* valid = nullptr);
    [[nodiscard]] static QString preferenceForSettings(LanguagePreference preference);
    [[nodiscard]] static QString resolveEffectiveLanguage(LanguagePreference preference);
    void applyEffectiveLanguage(const QString& language);

    QQmlEngine& engine_;
    QSettings& settings_;
    std::unique_ptr<QTranslator> translator_;
    LanguagePreference languagePreference_ = SystemLanguage;
    QString effectiveLanguage_;
};
