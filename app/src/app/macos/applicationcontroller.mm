// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "applicationcontroller.h"

#import <AppKit/AppKit.h>

#include <QWindow>

void
ApplicationController::requestBringAllWindowsToFront()
{
    @autoreleasepool {
        [NSApplication.sharedApplication arrangeInFront:nil];
    }
}

void
ApplicationController::requestMinimizeActiveWindow()
{
    @autoreleasepool {
        [NSApplication.sharedApplication.keyWindow performMiniaturize:nil];
    }
}

void
ApplicationController::requestZoomActiveWindow()
{
    @autoreleasepool {
        [NSApplication.sharedApplication.keyWindow performZoom:nil];
    }
}

void
ApplicationController::applyNativeWindowTitleVisibility(QWindow* window, bool visible)
{
    if (!window || !window->handle())
        return;

    @autoreleasepool {
        NSView* view = (__bridge NSView*)(reinterpret_cast<void*>(window->winId()));
        NSWindow* nativeWindow = view.window;
        if (!nativeWindow)
            return;

        nativeWindow.titleVisibility = visible ? NSWindowTitleVisible : NSWindowTitleHidden;
    }
}
