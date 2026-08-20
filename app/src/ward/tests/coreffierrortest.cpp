// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/coreffierror.h"
#include <ward_core.h>

#include <QtTest/QTest>

class CoreFfiErrorTest : public QObject
{
    Q_OBJECT

  private slots:
    void takesNullError();
    void takesOwnedErrorMessage();
};

void
CoreFfiErrorTest::takesNullError()
{
    QVERIFY(ward::coreffi::takeErrorMessage(nullptr).isEmpty());
}

void
CoreFfiErrorTest::takesOwnedErrorMessage()
{
    WardBlake3Digest digest{};
    WardError* error = nullptr;
    QVERIFY(!ward_core_blake3_hash_file(nullptr, &digest, &error));
    QVERIFY(error != nullptr);

    const QString message = ward::coreffi::takeErrorMessage(error);

    QVERIFY(message.contains(QStringLiteral("path"), Qt::CaseInsensitive));
}

QTEST_GUILESS_MAIN(CoreFfiErrorTest)

#include "coreffierrortest.moc"
