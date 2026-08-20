// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "applicationcontroller.h"

#import <AppKit/AppKit.h>

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
