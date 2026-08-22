// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#import <AppKit/AppKit.h>

#include "app/applicationcontroller.h"
#include "ward/realm/realmcontroller.h"

#include <QGuiApplication>
#include <QWindow>
#include <QtTest/QTest>

class ApplicationControllerTest : public QObject
{
    Q_OBJECT

  private slots:
    void reappliesNativeTitleVisibilityWhenWindowSurfaceIsRecreated();
};

static NSWindow*
nativeWindowFor(QWindow& window)
{
    if (!window.handle())
        return nil;

    NSView* view = (__bridge NSView*)(reinterpret_cast<void*>(window.winId()));
    return view.window;
}

void
ApplicationControllerTest::reappliesNativeTitleVisibilityWhenWindowSurfaceIsRecreated()
{
    QGuiApplication::setQuitOnLastWindowClosed(false);
    RealmController realmController;
    ApplicationController controller(*qGuiApp, realmController);
    QWindow window;
    window.setTitle(QStringLiteral("Craftward"));
    window.setFlags(Qt::Window | Qt::ExpandedClientAreaHint | Qt::NoTitleBarBackgroundHint);
    window.resize(640, 480);
    window.show();

    QTRY_VERIFY(window.handle());
    controller.setNativeWindowTitleVisible(&window, false);
    NSWindow* firstNativeWindow = nativeWindowFor(window);
    QVERIFY(firstNativeWindow);
    QCOMPARE(firstNativeWindow.titleVisibility, NSWindowTitleHidden);
    const WId firstWindowId = reinterpret_cast<WId>(firstNativeWindow);

    QVERIFY(window.close());
    QTRY_VERIFY(!window.handle());
    window.show();

    QTRY_VERIFY(window.handle());
    NSWindow* secondNativeWindow = nativeWindowFor(window);
    QVERIFY(secondNativeWindow);
    QVERIFY(reinterpret_cast<WId>(secondNativeWindow) != firstWindowId);
    QCOMPARE(secondNativeWindow.titleVisibility, NSWindowTitleHidden);
}

QTEST_MAIN(ApplicationControllerTest)

#include "applicationcontrollertest.moc"
