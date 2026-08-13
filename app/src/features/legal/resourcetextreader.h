// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QObject>
#include <QUrl>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>

class ResourceTextReader final : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

  public:
    explicit ResourceTextReader(QObject* parent = nullptr);

    Q_INVOKABLE QVariantMap read(const QUrl& resourceUrl) const;
};
