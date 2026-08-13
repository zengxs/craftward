// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "windowmovehelper.h"

#import <AppKit/AppKit.h>

#include <QWindow>

WindowMoveHelper::WindowMoveHelper(QObject* parent)
  : QObject(parent)
{
}

bool
WindowMoveHelper::startSystemMove(QWindow* window) const
{
    if (!window || (NSEvent.pressedMouseButtons & 1) == 0)
        return false;

    NSView* view = (__bridge NSView*)(reinterpret_cast<void*>(window->winId()));
    NSWindow* nativeWindow = view.window;
    if (!nativeWindow)
        return false;

    NSEvent* mouseEvent = [NSEvent mouseEventWithType:NSEventTypeLeftMouseDown
                                             location:nativeWindow.mouseLocationOutsideOfEventStream
                                        modifierFlags:NSEvent.modifierFlags
                                            timestamp:NSProcessInfo.processInfo.systemUptime
                                         windowNumber:nativeWindow.windowNumber
                                              context:nil
                                          eventNumber:0
                                           clickCount:1
                                             pressure:1.0];
    [nativeWindow performWindowDragWithEvent:mouseEvent];
    return true;
}
