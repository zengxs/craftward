// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QFileSystemModel>
#include <QSet>
#include <QtQmlIntegration/qqmlintegration.h>

class ProjectFilesModel : public QFileSystemModel
{
    Q_OBJECT
    QML_ELEMENT
    Q_PROPERTY(QString directory READ directory WRITE setDirectory NOTIFY directoryChanged)
    Q_PROPERTY(QModelIndex rootIndex READ rootIndex NOTIFY directoryChanged)
    Q_PROPERTY(bool available READ available NOTIFY directoryChanged)

  public:
    explicit ProjectFilesModel(QObject* parent = nullptr);

    QString directory() const { return directory_; }
    void setDirectory(const QString& directory);
    QModelIndex rootIndex() const;
    bool available() const { return !directory_.isEmpty(); }

    Q_INVOKABLE QString path(const QModelIndex& index) const;
    Q_INVOKABLE bool isDirectory(const QModelIndex& index) const;
    Q_INVOKABLE QModelIndex indexForPath(const QString& path) const;
    Q_INVOKABLE bool loadAncestors(const QModelIndex& index);

  signals:
    void directoryChanged();

  private:
    QString directory_;
    QSet<QString> loadedDirectories_;
};
