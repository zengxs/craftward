// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QObject>
#include <QtQml/qqmlregistration.h>

class QEvent;
class QGuiApplication;
class RealmController;

class ApplicationController : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_UNCREATABLE("ApplicationController is provided by the application.")

  public:
    ApplicationController(QGuiApplication& application, RealmController& realmController, QObject* parent = nullptr);

    Q_INVOKABLE void requestBringAllWindowsToFront();
    Q_INVOKABLE void requestCloseActiveWindow();
    Q_INVOKABLE void requestMinimizeActiveWindow();
    Q_INVOKABLE void requestQuit();
    Q_INVOKABLE void requestZoomActiveWindow();

  signals:
    void quitBlocked();
    void reopenRequested();

  protected:
    bool eventFilter(QObject* watched, QEvent* event) override;

  private:
    void requestReopenIfNeeded();

    QGuiApplication& application_;
    RealmController& realmController_;
};
