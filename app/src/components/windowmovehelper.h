// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#ifndef CRAFTWARD_WINDOWMOVEHELPER_H
#define CRAFTWARD_WINDOWMOVEHELPER_H

#include <QObject>
#include <QQmlEngine>
#include <QWindow>

class WindowMoveHelper : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

  public:
    explicit WindowMoveHelper(QObject* parent = nullptr);

    Q_INVOKABLE bool startSystemMove(QWindow* window) const;
};

#endif
