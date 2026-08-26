// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QObject>
#include <QString>
#include <QtQmlIntegration/qqmlintegration.h>

class ApplicationClipboard : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

  public:
    explicit ApplicationClipboard(QObject* parent = nullptr);

    Q_INVOKABLE [[nodiscard]] bool copyText(const QString& text) const;
};
