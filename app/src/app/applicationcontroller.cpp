// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "applicationcontroller.h"

#include "ward/realm/realmcontroller.h"

#include <QEvent>
#include <QGuiApplication>
#include <QPlatformSurfaceEvent>
#include <QTimer>
#include <QWindow>

ApplicationController::ApplicationController(QGuiApplication& application,
                                             RealmController& realmController,
                                             QObject* parent)
  : QObject(parent)
  , application_(application)
  , realmController_(realmController)
{
    application_.installEventFilter(this);
    connect(&application_, &QGuiApplication::applicationStateChanged, this, [this](Qt::ApplicationState state) {
        if (state == Qt::ApplicationActive)
            QTimer::singleShot(0, this, &ApplicationController::requestReopenIfNeeded);
    });
}

void
ApplicationController::requestCloseActiveWindow()
{
    QWindow* window = application_.focusWindow();
    while (window && window->parent())
        window = window->parent();

    if (window)
        window->close();
}

void
ApplicationController::requestQuit()
{
    application_.quit();
}

void
ApplicationController::setNativeWindowTitleVisible(QWindow* window, bool visible)
{
    if (!window)
        return;

    const bool alreadyTracked = nativeWindowTitleVisibility_.contains(window);
    nativeWindowTitleVisibility_.insert(window, visible);
    if (!alreadyTracked) {
        connect(window, &QObject::destroyed, this, [this, window] { nativeWindowTitleVisibility_.remove(window); });
    }

    applyNativeWindowTitleVisibility(window, visible);
}

bool
ApplicationController::eventFilter(QObject* watched, QEvent* event)
{
    if (event->type() == QEvent::PlatformSurface) {
        const auto* surfaceEvent = static_cast<QPlatformSurfaceEvent*>(event);
        if (surfaceEvent->surfaceEventType() == QPlatformSurfaceEvent::SurfaceCreated) {
            auto* window = qobject_cast<QWindow*>(watched);
            const auto visibility = nativeWindowTitleVisibility_.constFind(window);
            if (visibility != nativeWindowTitleVisibility_.constEnd())
                applyNativeWindowTitleVisibility(window, visibility.value());
        }
    }

    if (watched == &application_ && event->type() == QEvent::Quit && realmController_.requiresStopBeforeExit()) {
        emit quitBlocked();
        return true;
    }

    return QObject::eventFilter(watched, event);
}

void
ApplicationController::requestReopenIfNeeded()
{
    for (const QWindow* window : QGuiApplication::topLevelWindows()) {
        if (window->isVisible())
            return;
    }

    emit reopenRequested();
}
