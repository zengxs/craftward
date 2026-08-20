// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "applicationmenus.h"

#import <AppKit/AppKit.h>

#include <QCoreApplication>
#include <QGuiApplication>
#include <QTimer>

namespace {

NSMenu*
findTopLevelMenu(NSString* englishTitle)
{
    NSBundle* appKitBundle = [NSBundle bundleForClass:[NSApplication class]];
    NSString* localizedTitle = [appKitBundle localizedStringForKey:englishTitle
                                                             value:englishTitle
                                                             table:@"MenuCommands"];

    for (NSMenuItem* item in NSApplication.sharedApplication.mainMenu.itemArray) {
        if ([item.title isEqualToString:englishTitle] || [item.title isEqualToString:localizedTitle])
            return item.submenu;
    }

    return nil;
}

void
registerNativeApplicationMenusNow()
{
    @autoreleasepool {
        NSApplication* application = NSApplication.sharedApplication;

        NSMenu* windowMenu = findTopLevelMenu(@"Window");
        NSMenu* helpMenu = findTopLevelMenu(@"Help");
        if (windowMenu == nil || helpMenu == nil)
            return;

        application.windowsMenu = windowMenu;
        application.helpMenu = helpMenu;
    }
}

void
scheduleNativeApplicationMenuRegistration(QGuiApplication* application)
{
    QTimer::singleShot(0, application, registerNativeApplicationMenusNow);
}

} // namespace

void
registerNativeApplicationMenus()
{
    auto* application = qobject_cast<QGuiApplication*>(QCoreApplication::instance());
    if (application == nullptr)
        return;

    QObject::connect(application, &QGuiApplication::focusWindowChanged, application, [application] {
        scheduleNativeApplicationMenuRegistration(application);
    });
    QObject::connect(
      application, &QGuiApplication::applicationStateChanged, application, [application](Qt::ApplicationState state) {
          if (state == Qt::ApplicationActive)
              scheduleNativeApplicationMenuRegistration(application);
      });
    scheduleNativeApplicationMenuRegistration(application);
}
