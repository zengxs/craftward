// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "applicationfiles.h"

#include <QDesktopServices>
#include <QFileInfo>
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
