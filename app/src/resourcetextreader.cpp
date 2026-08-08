#include "resourcetextreader.h"

#include <QFile>

ResourceTextReader::ResourceTextReader(QObject* parent)
  : QObject(parent)
{
}

QVariantMap
ResourceTextReader::read(const QUrl& resourceUrl) const
{
    if (resourceUrl.scheme() != QStringLiteral("qrc")) {
        return {
            { QStringLiteral("errorMessage"), tr("Text resources must use a local Qt resource URL.") },
            { QStringLiteral("text"), QString() },
        };
    }

    QFile file(QStringLiteral(":") + resourceUrl.path());
    if (!file.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return {
            { QStringLiteral("errorMessage"), tr("The text resource could not be opened.") },
            { QStringLiteral("text"), QString() },
        };
    }

    return {
        { QStringLiteral("errorMessage"), QString() },
        { QStringLiteral("text"), QString::fromUtf8(file.readAll()) },
    };
}
