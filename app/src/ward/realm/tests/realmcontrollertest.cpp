// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/realm/realmcontroller.h"

#include <QCoreApplication>
#include <QTranslator>
#include <QtTest/QSignalSpy>
#include <QtTest/QTest>

class RealmControllerTest : public QObject
{
    Q_OBJECT

  private slots:
    void retranslatesStateTextWhenRequested();
};

void
RealmControllerTest::retranslatesStateTextWhenRequested()
{
    QTranslator english;
    QVERIFY(english.load(QStringLiteral(":/i18n/craftward_en.qm")));
    QVERIFY(QCoreApplication::installTranslator(&english));
    RealmController controller;
    QCOMPARE(controller.stateText(), QStringLiteral("Not opened"));
    QSignalSpy stateTextSpy(&controller, &RealmController::stateTextChanged);

    QTranslator simplifiedChinese;
    QVERIFY(simplifiedChinese.load(QStringLiteral(":/i18n/craftward_zh_CN.qm")));
    QVERIFY(QCoreApplication::installTranslator(&simplifiedChinese));
    controller.retranslate();

    QVERIFY(stateTextSpy.count() > 0);
    QCOMPARE(controller.stateText(), QStringLiteral("未打开"));

    QVERIFY(QCoreApplication::removeTranslator(&simplifiedChinese));
    QVERIFY(QCoreApplication::removeTranslator(&english));
}

QTEST_GUILESS_MAIN(RealmControllerTest)

#include "realmcontrollertest.moc"
