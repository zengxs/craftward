// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "projectfilesmodel.h"

#include <QFileInfo>

ProjectFilesModel::ProjectFilesModel(QObject* parent)
  : QFileSystemModel(parent)
{
    setReadOnly(true);
    setOption(QFileSystemModel::DontUseCustomDirectoryIcons);
    setFilter(QDir::AllEntries | QDir::NoDotAndDotDot | QDir::AllDirs | QDir::Hidden);
    connect(
      this, &QFileSystemModel::directoryLoaded, this, [this](const QString& path) { loadedDirectories_.insert(path); });
    sort(0);
}

void
ProjectFilesModel::setDirectory(const QString& directory)
{
    const QFileInfo info(directory);
    const QString resolved = info.isAbsolute() && info.isDir() ? info.canonicalFilePath() : QString();
    if (directory_ == resolved)
        return;
    directory_ = resolved;
    if (!directory_.isEmpty())
        setRootPath(directory_);
    emit directoryChanged();
}

QModelIndex
ProjectFilesModel::rootIndex() const
{
    return available() ? index(directory_) : QModelIndex();
}

QString
ProjectFilesModel::path(const QModelIndex& index) const
{
    return filePath(index);
}

bool
ProjectFilesModel::isDirectory(const QModelIndex& index) const
{
    return isDir(index);
}

QModelIndex
ProjectFilesModel::indexForPath(const QString& path) const
{
    if (!available())
        return {};
    const QString canonical = QFileInfo(path).canonicalFilePath();
    if (canonical.isEmpty())
        return {};
    const QString relative = QDir(directory_).relativeFilePath(canonical);
    if (relative == QStringLiteral("..") || relative.startsWith(QStringLiteral("../")) ||
        QDir::isAbsolutePath(relative))
        return {};
    return index(canonical);
}

bool
ProjectFilesModel::loadAncestors(const QModelIndex& index)
{
    if (!index.isValid() || index.model() != this || !available())
        return false;
    QList<QModelIndex> ancestors;
    for (QModelIndex ancestor = index.parent(); ancestor.isValid(); ancestor = ancestor.parent()) {
        ancestors.prepend(ancestor);
        if (filePath(ancestor) == directory_)
            break;
    }
    if (ancestors.isEmpty() || filePath(ancestors.first()) != directory_)
        return false;
    bool loaded = true;
    for (const auto& ancestor : ancestors) {
        if (canFetchMore(ancestor))
            fetchMore(ancestor);
        loaded = loaded && loadedDirectories_.contains(filePath(ancestor));
    }
    return loaded;
}
