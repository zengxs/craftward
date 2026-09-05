// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codextimelinepresentationmodel.h"
#include "ward/codex/codextimelineviewportmodel.h"
#include "ward/markup/markupdocumentmodel.h"

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
    void streamsSemanticBlockWithoutRebuildingVisibleDelegates();
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
    view.rootContext()->setContextProperty(QStringLiteral("viewportModel"), &presentationModel);
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

void
CodexTimelineViewportIntegrationTest::streamsSemanticBlockWithoutRebuildingVisibleDelegates()
{
    MarkupDocumentModel document;
    QVERIFY(document.reconcileSource(QStringLiteral("Before\n\n```cpp\nreturn 0;\n```"),
                                     MarkupDocumentModel::SourceFormat::Markdown));

    QStandardItemModel source;
    configureRoles(source);
    appendRow(source, QStringLiteral("answer"), QStringLiteral("turn-1"), false, false, false, false, true);
    source.item(0)->setData(QVariant::fromValue(static_cast<QObject*>(&document)), MarkupDocumentRole);

    CodexTimelinePresentationModel presentationModel;
    presentationModel.setSourceModel(&source);
    CodexTimelineViewportModel viewportModel;
    viewportModel.setSourceModel(&presentationModel);
    QTRY_COMPARE_WITH_TIMEOUT(viewportModel.rowCount(), 2, 5000);

    QQuickView view;
    view.engine()->addImportPath(QStringLiteral(CRAFTWARD_TEST_QML_IMPORT_PATH));
    view.rootContext()->setContextProperty(QStringLiteral("viewportModel"), &viewportModel);
    view.setSource(QUrl::fromLocalFile(QStringLiteral(CRAFTWARD_TIMELINE_VIEWPORT_HARNESS)));
    QCOMPARE(view.status(), QQuickView::Ready);
    view.show();

    QObject* viewport = view.rootObject()->findChild<QObject*>(QStringLiteral("integrationViewport"));
    QVERIFY(viewport);
    const QString firstEntryId = viewportModel.entryIdAt(0);
    const QString secondEntryId = viewportModel.entryIdAt(1);
    QTRY_VERIFY_WITH_TIMEOUT(delegateForEntry(viewport, firstEntryId), 2000);
    QTRY_VERIFY_WITH_TIMEOUT(delegateForEntry(viewport, secondEntryId), 2000);
    QPointer<QObject> firstDelegateBefore = delegateForEntry(viewport, firstEntryId);
    QPointer<QObject> secondDelegateBefore = delegateForEntry(viewport, secondEntryId);

    QVERIFY(document.reconcileSource(QStringLiteral("Before\n\n```cpp\nreturn 0;\n```\n\nAfter"),
                                     MarkupDocumentModel::SourceFormat::Markdown));

    QTRY_COMPARE_WITH_TIMEOUT(viewportModel.rowCount(), 3, 5000);
    QTRY_VERIFY_WITH_TIMEOUT(delegateForEntry(viewport, viewportModel.entryIdAt(2)), 2000);
    QTRY_COMPARE_WITH_TIMEOUT(delegateForEntry(viewport, firstEntryId), firstDelegateBefore.data(), 2000);
    QTRY_COMPARE_WITH_TIMEOUT(delegateForEntry(viewport, secondEntryId), secondDelegateBefore.data(), 2000);
    QVERIFY(firstDelegateBefore);
    QVERIFY(secondDelegateBefore);
}

QTEST_MAIN(CodexTimelineViewportIntegrationTest)

#include "codextimelineviewportintegrationtest.moc"
