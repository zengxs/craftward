// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "localization/localizationcontroller.h"

#include <QQmlComponent>
#include <QQmlEngine>
#include <QSettings>
#include <QTemporaryDir>
#include <QtTest/QSignalSpy>
#include <QtTest/QTest>
#include <QtTranslation>

#include <memory>

class LocalizationControllerTest : public QObject
{
    Q_OBJECT

  private slots:
    void loadsAndPersistsExplicitLanguagePreferences();
    void normalizesUnknownPersistedPreferences();
    void retranslatesQmlBindingsImmediately();
};

void
LocalizationControllerTest::loadsAndPersistsExplicitLanguagePreferences()
{
    QTemporaryDir directory;
    QVERIFY(directory.isValid());
    QSettings settings(directory.filePath(QStringLiteral("settings.ini")), QSettings::IniFormat);
    settings.setValue(QStringLiteral("ui/language"), QStringLiteral("zh-Hans"));
    QQmlEngine engine;
    LocalizationController controller(engine, settings);

    QCOMPARE(controller.languagePreference(), LocalizationController::SimplifiedChinese);
    QCOMPARE(controller.effectiveLanguage(), QStringLiteral("zh-Hans"));
    QCOMPARE(qtTrId("craftward.settings.general.title"), QStringLiteral("通用"));

    controller.setLanguagePreference(LocalizationController::English);

    QCOMPARE(controller.effectiveLanguage(), QStringLiteral("en"));
    QCOMPARE(settings.value(QStringLiteral("ui/language")).toString(), QStringLiteral("en"));
    QCOMPARE(qtTrId("craftward.settings.general.title"), QStringLiteral("General"));
}

void
LocalizationControllerTest::normalizesUnknownPersistedPreferences()
{
    QTemporaryDir directory;
    QVERIFY(directory.isValid());
    QSettings settings(directory.filePath(QStringLiteral("settings.ini")), QSettings::IniFormat);
    settings.setValue(QStringLiteral("ui/language"), QStringLiteral("unsupported"));
    QQmlEngine engine;
    LocalizationController controller(engine, settings);

    QCOMPARE(controller.languagePreference(), LocalizationController::SystemLanguage);
    QCOMPARE(settings.value(QStringLiteral("ui/language")).toString(), QStringLiteral("system"));
    QVERIFY(controller.effectiveLanguage() == QStringLiteral("en") ||
            controller.effectiveLanguage() == QStringLiteral("zh-Hans"));
}

void
LocalizationControllerTest::retranslatesQmlBindingsImmediately()
{
    QTemporaryDir directory;
    QVERIFY(directory.isValid());
    QSettings settings(directory.filePath(QStringLiteral("settings.ini")), QSettings::IniFormat);
    settings.setValue(QStringLiteral("ui/language"), QStringLiteral("en"));
    QQmlEngine engine;
    LocalizationController controller(engine, settings);
    QQmlComponent component(&engine);
    component.setData(R"(
        import QtQml
        QtObject {
            property string label: qsTrId("craftward.settings.general.title")
        }
    )",
                      QUrl());
    QTRY_VERIFY_WITH_TIMEOUT(component.status() != QQmlComponent::Loading, 5000);
    QVERIFY2(component.isReady(), qPrintable(component.errorString()));
    std::unique_ptr<QObject> object(component.create());
    QVERIFY2(object != nullptr, qPrintable(component.errorString()));
    QCOMPARE(object->property("label").toString(), QStringLiteral("General"));
    QSignalSpy languageSpy(&controller, &LocalizationController::effectiveLanguageChanged);

    controller.setLanguagePreference(LocalizationController::SimplifiedChinese);

    QCOMPARE(languageSpy.count(), 1);
    QCOMPARE(object->property("label").toString(), QStringLiteral("通用"));
}

QTEST_GUILESS_MAIN(LocalizationControllerTest)

#include "localizationcontrollertest.moc"
