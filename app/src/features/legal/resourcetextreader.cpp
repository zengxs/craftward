// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "resourcetextreader.h"

#include <QFile>
#include <QtTranslation>

ResourceTextReader::ResourceTextReader(QObject* parent)
  : QObject(parent)
{
}

QVariantMap
ResourceTextReader::read(const QUrl& resourceUrl) const
{
    if (resourceUrl.scheme() != QStringLiteral("qrc")) {
        return {
            { QStringLiteral("errorMessage"),
              /*% "Text resources must use a local Qt resource URL." */ qtTrId("craftward.legal.error.resource_url") },
            { QStringLiteral("text"), QString() },
        };
    }

    QFile file(QStringLiteral(":") + resourceUrl.path());
    if (!file.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return {
            { QStringLiteral("errorMessage"),
              /*% "The text resource could not be opened." */ qtTrId("craftward.legal.error.resource_open") },
            { QStringLiteral("text"), QString() },
        };
    }

    return {
        { QStringLiteral("errorMessage"), QString() },
        { QStringLiteral("text"), QString::fromUtf8(file.readAll()) },
    };
}
