// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "applicationfiles.h"
#include "projectfilesmodel.h"

#include <QDir>
#include <QFile>
#include <QFileInfo>
#include <QTemporaryDir>
#include <QTest>
#include <QUrl>

class WorkbenchFilesTest : public QObject
{
    Q_OBJECT

    static void writeFile(const QString& path, const QByteArray& content)
    {
        QFile file(path);
        QVERIFY(file.open(QIODevice::WriteOnly));
        QCOMPARE(file.write(content), content.size());
    }

  private slots:
    void readsLocalUrlsAndDistinguishesProjectFiles()
    {
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const QString project = directory.filePath("project");
        QVERIFY(QDir().mkpath(project));
        const QString path = QDir(project).filePath(QStringLiteral("a # with spaces.txt"));
        writeFile(path, "hello\n");
        ApplicationFiles files;
        const auto inside = files.readTextFile(QUrl::fromLocalFile(path).toString(), project);
        QCOMPARE(inside.value("text").toString(), QStringLiteral("hello\n"));
        QVERIFY(inside.value("error").toString().isEmpty());
        QVERIFY(!inside.value("external").toBool());
        QCOMPARE(inside.value("location").toString(), QStringLiteral("a # with spaces.txt"));
        const auto independent = files.readTextFile(path);
        QVERIFY(independent.value("external").toBool());
        QCOMPARE(independent.value("location"), independent.value("path"));
    }

    void siblingPrefixesAndSymlinksDoNotImplyProjectMembership()
    {
        QTemporaryDir directory;
        const QString project = directory.filePath("project");
        const QString sibling = directory.filePath("project-other");
        QVERIFY(QDir().mkpath(project));
        QVERIFY(QDir().mkpath(sibling));
        const QString outside = QDir(sibling).filePath("notes.txt");
        writeFile(outside, "outside");
        const QString link = QDir(project).filePath("linked.txt");
        QVERIFY(QFile::link(outside, link));
        ApplicationFiles files;
        QVERIFY(files.readTextFile(outside, project).value("external").toBool());
        const auto linked = files.readTextFile(link, project);
        QVERIFY(linked.value("external").toBool());
        QCOMPARE(linked.value("path"), files.readTextFile(outside, project).value("path"));
        ProjectFilesModel model;
        model.setDirectory(project);
        QVERIFY(!model.indexForPath(outside).isValid());
        QVERIFY(!model.indexForPath(link).isValid());
    }

    void fileTreeResolvesSymlinkedProjectPaths()
    {
        QTemporaryDir directory;
        QVERIFY(directory.isValid());
        const QString project = directory.filePath("project");
        QVERIFY(QDir().mkpath(QDir(project).filePath("nested")));
        const QString path = QDir(project).filePath("nested/notes.txt");
        writeFile(path, "notes");
        const QString alias = directory.filePath("project-alias");
        QVERIFY(QFile::link(project, alias));
        ProjectFilesModel model;
        model.setDirectory(alias);
        const QString canonical = QFileInfo(path).canonicalFilePath();
        const auto index = model.indexForPath(canonical);
        QVERIFY(index.isValid());
        QCOMPARE(model.indexForPath(path), index);
        QCOMPARE(model.indexForPath(QDir(alias).filePath("nested/notes.txt")), index);
        QCOMPARE(model.path(index), canonical);
    }

    void unsupportedAndOversizedFilesProduceExplicitErrors()
    {
        QTemporaryDir directory;
        ApplicationFiles files;
        const QString binary = directory.filePath("binary.dat");
        writeFile(binary, QByteArray("a\0b", 3));
        QVERIFY(!files.readTextFile(binary).value("error").toString().isEmpty());
        const QString large = directory.filePath("large.txt");
        QFile largeFile(large);
        QVERIFY(largeFile.open(QIODevice::WriteOnly));
        QVERIFY(largeFile.resize(8 * 1024 * 1024 + 1));
        largeFile.close();
        QVERIFY(!files.readTextFile(large).value("error").toString().isEmpty());
        QVERIFY(!files.readTextFile(directory.filePath("missing.txt")).value("error").toString().isEmpty());
        QVERIFY(!files.readTextFile(directory.path()).value("error").toString().isEmpty());
    }

    void fileTreeIsScopedAndObservesDirectoryChanges()
    {
        QTemporaryDir directory;
        const QString project = directory.filePath("project");
        QVERIFY(QDir().mkpath(project));
        writeFile(QDir(project).filePath("first.txt"), "one");
        ProjectFilesModel model;
        model.setDirectory(project);
        QVERIFY(model.available());
        QVERIFY(model.rootIndex().isValid());
        QTRY_COMPARE(model.rowCount(model.rootIndex()), 1);
        writeFile(QDir(project).filePath("second.txt"), "two");
        QTRY_COMPARE(model.rowCount(model.rootIndex()), 2);
        QVERIFY(model.indexForPath(QDir(project).filePath("first.txt")).isValid());
        QVERIFY(!model.indexForPath(directory.path()).isValid());
        model.setDirectory({});
        QVERIFY(!model.available());
        QVERIFY(!model.rootIndex().isValid());
    }
};

QTEST_MAIN(WorkbenchFilesTest)
#include "workbenchfilestest.moc"
