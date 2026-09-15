// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QObject>
#include <QTemporaryDir>

class FileTreeFixture : public QObject
{
    Q_OBJECT
    Q_PROPERTY(QString directory READ directory CONSTANT)

  public:
    explicit FileTreeFixture(QObject* parent = nullptr)
      : QObject(parent)
    {
    }

    QString directory() const { return directory_.path(); }

    Q_INVOKABLE QString createFile(const QString& relativePath)
    {
        const QString path = directory_.filePath(relativePath);
        if (!QDir().mkpath(QFileInfo(path).absolutePath()))
            return {};
        QFile file(path);
        if (!file.open(QIODevice::WriteOnly) || file.write("fixture\n") != 8)
            return {};
        return path;
    }

  private:
    QTemporaryDir directory_;
};
