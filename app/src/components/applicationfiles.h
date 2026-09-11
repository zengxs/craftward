// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QObject>
#include <QString>
#include <QtQmlIntegration/qqmlintegration.h>

class ApplicationFiles : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

  public:
    explicit ApplicationFiles(QObject* parent = nullptr);

    Q_INVOKABLE [[nodiscard]] bool openLocalFile(const QString& path) const;
};
