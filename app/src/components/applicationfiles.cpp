// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "applicationfiles.h"

#include <QDesktopServices>
#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QStringDecoder>
#include <QUrl>

ApplicationFiles::ApplicationFiles(QObject* parent)
  : QObject(parent)
{
}

bool
ApplicationFiles::openLocalFile(const QString& path) const
{
    const QFileInfo file(path);
    if (!file.isAbsolute() || !file.isFile())
        return false;
    // Resolve symlinks and parent segments before handing the target to URL handlers.
    const QString target = file.canonicalFilePath();
    return !target.isEmpty() && QDesktopServices::openUrl(QUrl::fromLocalFile(target));
}

QVariantMap
ApplicationFiles::readTextFile(const QString& path, const QString& projectDirectory) const
{
    const QUrl url(path);
    const QFileInfo info(url.isLocalFile() ? url.toLocalFile() : path);
    const QString canonical = info.canonicalFilePath();
    const QString resolved = canonical.isEmpty() ? info.absoluteFilePath() : canonical;
    const QString project = QFileInfo(projectDirectory).canonicalFilePath();
    const QString relative = project.isEmpty() ? resolved : QDir(project).relativeFilePath(resolved);
    const bool external = project.isEmpty() || relative == QStringLiteral("..") ||
                          relative.startsWith(QStringLiteral("../")) || QDir::isAbsolutePath(relative);
    QVariantMap result{
        { QStringLiteral("id"), resolved },
        { QStringLiteral("kind"), QStringLiteral("file") },
        { QStringLiteral("title"), info.fileName() },
        { QStringLiteral("path"), resolved },
        { QStringLiteral("location"), external ? resolved : relative },
        { QStringLiteral("external"), external },
        { QStringLiteral("text"), QString() },
        { QStringLiteral("error"), QString() },
    };
    QFile file(resolved);
    if (!info.isAbsolute() || !info.isFile() || !file.open(QIODevice::ReadOnly)) {
        //% "Could not read this file."
        result[QStringLiteral("error")] = qtTrId("craftward.file.read_failed");
        return result;
    }
    constexpr qint64 maximumPreviewBytes = 8 * 1024 * 1024;
    const QByteArray bytes = file.read(maximumPreviewBytes + 1);
    if (file.error() != QFileDevice::NoError) {
        result[QStringLiteral("error")] = qtTrId("craftward.file.read_failed");
        return result;
    }
    if (bytes.size() > maximumPreviewBytes) {
        //% "This file is too large to preview. Open it in its default application."
        result[QStringLiteral("error")] = qtTrId("craftward.file.preview_too_large");
        return result;
    }
    const auto encoding = QStringDecoder::encodingForData(bytes).value_or(QStringDecoder::Utf8);
    QStringDecoder decoder(encoding);
    const QString content = decoder(bytes);
    if (decoder.hasError() || content.contains(QChar::Null)) {
        //% "This file cannot be previewed as text. Open it in its default application."
        result[QStringLiteral("error")] = qtTrId("craftward.file.preview_unsupported");
        return result;
    }
    result[QStringLiteral("text")] = content;
    return result;
}
