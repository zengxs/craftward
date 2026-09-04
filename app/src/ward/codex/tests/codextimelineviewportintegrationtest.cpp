// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codextimelinepresentationmodel.h"

#include "timelinepresentationsourcemodelfixture.h"

#include <QMetaObject>
#include <QPointer>
#include <QQmlContext>
#include <QQmlEngine>
#include <QQuickItem>
#include <QQuickView>
#include <QQuickWindow>
#include <QStandardItem>
#include <QStandardItemModel>
#include <QtTest/QTest>

namespace {
using namespace TimelinePresentationSourceFixture;

QObject*
delegateForEntry(QObject* viewport, const QString& entryId)
{
    QVariant result;
    const bool invoked = QMetaObject::invokeMethod(
      viewport, "delegateForEntry", Q_RETURN_ARG(QVariant, result), Q_ARG(QVariant, QVariant::fromValue(entryId)));
    return invoked ? qvariant_cast<QObject*>(result) : nullptr;
}
}

class CodexTimelineViewportIntegrationTest : public QObject
{
    Q_OBJECT

  private slots:
    void initTestCase();
    void expandsDetailWithoutRebuildingVisibleDelegates();
};

void
CodexTimelineViewportIntegrationTest::initTestCase()
{
    QQuickWindow::setGraphicsApi(QSGRendererInterface::Software);
}

void
CodexTimelineViewportIntegrationTest::expandsDetailWithoutRebuildingVisibleDelegates()
{
    QStandardItemModel source;
    configureRoles(source);
    appendRow(source, QStringLiteral("user"), QStringLiteral("turn-1"), false, false, true);
    appendRow(source, QStringLiteral("detail-1"), QStringLiteral("turn-1"), true);
    appendRow(source, QStringLiteral("detail-2"), QStringLiteral("turn-1"), true);
    appendRow(source, QStringLiteral("detail-3"), QStringLiteral("turn-1"), true);
    appendRow(source, QStringLiteral("answer"), QStringLiteral("turn-1"), false, false, false, false, true);

    CodexTimelinePresentationModel presentationModel;
    presentationModel.setSourceModel(&source);
    QCOMPARE(presentationModel.rowCount(), 3);

    QQuickView view;
    view.engine()->addImportPath(QStringLiteral(CRAFTWARD_TEST_QML_IMPORT_PATH));
    view.rootContext()->setContextProperty(QStringLiteral("presentationModel"), &presentationModel);
    view.setSource(QUrl::fromLocalFile(QStringLiteral(CRAFTWARD_TIMELINE_VIEWPORT_HARNESS)));
    QCOMPARE(view.status(), QQuickView::Ready);
    view.show();

    QObject* viewport = view.rootObject()->findChild<QObject*>(QStringLiteral("integrationViewport"));
    QVERIFY(viewport);
    QTRY_VERIFY_WITH_TIMEOUT(delegateForEntry(viewport, QStringLiteral("detail-1")), 2000);
    QTRY_VERIFY_WITH_TIMEOUT(delegateForEntry(viewport, QStringLiteral("answer")), 2000);
    QPointer<QObject> detailHeaderBefore = delegateForEntry(viewport, QStringLiteral("detail-1"));
    QPointer<QObject> answerBefore = delegateForEntry(viewport, QStringLiteral("answer"));

    presentationModel.setTurnExpanded(QStringLiteral("turn-1"), true);

    QCOMPARE(presentationModel.rowCount(), 5);
    QTRY_VERIFY_WITH_TIMEOUT(delegateForEntry(viewport, QStringLiteral("detail-2")), 2000);
    QTRY_COMPARE_WITH_TIMEOUT(delegateForEntry(viewport, QStringLiteral("detail-1")), detailHeaderBefore.data(), 2000);
    QTRY_COMPARE_WITH_TIMEOUT(delegateForEntry(viewport, QStringLiteral("answer")), answerBefore.data(), 2000);
    QVERIFY(detailHeaderBefore);
    QVERIFY(answerBefore);
}

QTEST_MAIN(CodexTimelineViewportIntegrationTest)

#include "codextimelineviewportintegrationtest.moc"
