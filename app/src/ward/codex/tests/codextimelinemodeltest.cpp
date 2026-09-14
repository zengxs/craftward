// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codextimelinemodel.h"
#include "ward/codex/codextimelinepresentationmodel.h"
#include "ward/codex/codextimelineviewportmodel.h"

#include <QAbstractItemModelTester>
#include <QSignalSpy>
#include <QtTest/QTest>

namespace {
using namespace ward::codex::v1;
using MessageRole = MessageRoleGadget::MessageRole;

TimelineItem
message(const QString& id, MessageRole role, const QString& text, const QString& turn = QStringLiteral("turn-1"))
{
    Message value;
    value.setMessageId(id);
    value.setRole(role);
    value.setText(text);
    TimelineItem item;
    item.setTurnId(turn);
    item.setMessage(value);
    return item;
}

TimelineItem
annotatedMessage(const QString& id = QStringLiteral("user-1"), const QString& body = {})
{
    ResponseAnnotation annotation;
    annotation.setIndex(1);
    annotation.setText(QStringLiteral("Selected **literal** :codex-annotation{index=\"9\"}"));
    annotation.setComment(QStringLiteral("Change it"));
    AnnotatedUserMessage content;
    content.setBody(body);
    content.setAnnotations({ annotation });
    auto item = message(id, MessageRole::MESSAGE_ROLE_USER, QStringLiteral("\nOriginal client envelope\n"));
    auto value = item.message();
    value.setAnnotatedUserMessage(content);
    item.setMessage(value);
    return item;
}

MarkupDocumentModel*
documentAt(CodexTimelineModel& model, int row)
{
    return qobject_cast<MarkupDocumentModel*>(
      model.data(model.index(row), CodexTimelineModel::MarkupDocumentRole).value<QObject*>());
}
}

class CodexTimelineModelTest : public QObject
{
    Q_OBJECT

  private slots:
    void exposesOwnCollectionWithoutAnEmptyBody();
    void preservesBodyAndCollectionAcrossViewportSegments();
    void resolvesAllEarlierCandidatesInTheSameTurn();
    void reconcilesBodyAndMetadataWithoutDuplicatingCandidates();
    void ignoresUserPresentationMetadataOnAgentMessages();
};

void
CodexTimelineModelTest::exposesOwnCollectionWithoutAnEmptyBody()
{
    CodexTimelineModel model;
    QAbstractItemModelTester tester(&model, QAbstractItemModelTester::FailureReportingMode::QtTest);
    model.reconcileTimeline({ annotatedMessage() }, {});
    QCOMPARE(model.rowCount(), 1);
    QCOMPARE(model.data(model.index(0), CodexTimelineModel::AnnotationCountRole).toInt(), 1);
    QCOMPARE(documentAt(model, 0), nullptr);
    QCOMPARE(model.data(model.index(0), CodexTimelineModel::RawTextRole).toString(),
             QStringLiteral("\nOriginal client envelope\n"));
    const auto copy = model.data(model.index(0), CodexTimelineModel::TextRole).toString();
    QVERIFY(copy.startsWith(QStringLiteral("[1] Selected **literal**")));
    QVERIFY(copy.endsWith(QStringLiteral("\n\nChange it")));
    const auto own = model.responseAnnotations(model.entryIdAt(0));
    QCOMPARE(own.size(), 1);
    QCOMPARE(own.first().toMap().value(QStringLiteral("inputNumber")).toInt(), 1);
    QCOMPARE(own.first().toMap().value(QStringLiteral("text")).toString(),
             QStringLiteral("Selected **literal** :codex-annotation{index=\"9\"}"));
    QVERIFY(model.responseAnnotations(model.entryIdAt(0), 1).isEmpty());
    CodexTimelinePresentationModel presentation;
    presentation.setSourceModel(&model);
    CodexTimelineViewportModel viewport;
    viewport.setSourceModel(&presentation);
    QTRY_COMPARE(viewport.rowCount(), 1);
    QVERIFY(!viewport.valueAt(0, QStringLiteral("semanticBlock")).toBool());
    QCOMPARE(viewport.valueAt(0, QStringLiteral("annotationCount")).toInt(), 1);
}

void
CodexTimelineModelTest::preservesBodyAndCollectionAcrossViewportSegments()
{
    const auto body = QStringLiteral("\nBefore\n\n```cpp\nreturn 0;\n```\n\nAfter\n\n");
    CodexTimelineModel source;
    source.reconcileTimeline({ annotatedMessage(QStringLiteral("user-1"), body) }, {});
    QCOMPARE(source.rowCount(), 1);
    QCOMPARE(source.data(source.index(0), CodexTimelineModel::DisplayTextRole).toString(), body);
    CodexTimelinePresentationModel presentation;
    presentation.setSourceModel(&source);
    CodexTimelineViewportModel viewport;
    viewport.setSourceModel(&presentation);
    QAbstractItemModelTester tester(&viewport, QAbstractItemModelTester::FailureReportingMode::QtTest);
    QTRY_COMPARE(viewport.rowCount(), 3);
    QVERIFY(viewport.valueAt(1, QStringLiteral("codeBlock")).toBool());
    for (int row = 0; row < viewport.rowCount(); ++row) {
        QCOMPARE(viewport.valueAt(row, QStringLiteral("annotationCount")).toInt(), 1);
        QCOMPARE(viewport.valueAt(row, QStringLiteral("sourceEntryId")).toString(), source.entryIdAt(0));
        QCOMPARE(viewport.valueAt(row, QStringLiteral("firstBlockInEntry")).toBool(), row == 0);
        QCOMPARE(viewport.valueAt(row, QStringLiteral("lastBlockInEntry")).toBool(), row == 2);
        QVERIFY(viewport.valueAt(row, QStringLiteral("text")).toString().endsWith(body));
    }
}

void
CodexTimelineModelTest::resolvesAllEarlierCandidatesInTheSameTurn()
{
    auto previousTurn = annotatedMessage(QStringLiteral("old"));
    previousTurn.setTurnId(QStringLiteral("turn-0"));
    const auto first = annotatedMessage();
    const auto guidance = annotatedMessage(QStringLiteral("guidance"));
    const auto answer = [](const QString& id) {
        return message(id, MessageRole::MESSAGE_ROLE_AGENT, QStringLiteral("Done :codex-annotation{index=\"1\"}"));
    };
    const QList<TimelineItem> items = {
        previousTurn,
        first,
        answer(QStringLiteral("before")),
        message(QStringLiteral("plain-guidance"), MessageRole::MESSAGE_ROLE_USER, QStringLiteral("Continue")),
        guidance,
        answer(QStringLiteral("after")),
        annotatedMessage(QStringLiteral("future"))
    };
    CodexTimelineModel model;
    // Lookup works before either source document or viewport delegate exists.
    model.reconcileTimeline(items, {});
    const auto before = model.responseAnnotations(model.entryIdAt(2), 1);
    QCOMPARE(before.size(), 1);
    QCOMPARE(before.first().toMap().value(QStringLiteral("sourceEntryId")).toString(), model.entryIdAt(1));
    auto after = model.responseAnnotations(model.entryIdAt(5), 1);
    QCOMPARE(after.size(), 2);
    QCOMPARE(after[0].toMap().value(QStringLiteral("sourceEntryId")).toString(), model.entryIdAt(4));
    QCOMPARE(after[0].toMap().value(QStringLiteral("inputNumber")).toInt(), 3);
    QCOMPARE(after[1].toMap().value(QStringLiteral("sourceEntryId")).toString(), model.entryIdAt(1));
    // Identical payloads from different owning inputs remain separate candidates.
    QCOMPARE(after[0].toMap().value(QStringLiteral("text")), after[1].toMap().value(QStringLiteral("text")));
    model.reconcileTimeline(items, {});
    QCOMPARE(model.responseAnnotations(model.entryIdAt(5), 1), after);
    QCOMPARE(model.responseAnnotations(model.entryIdAt(4)).size(), 1);
    QVERIFY(model.responseAnnotations(model.entryIdAt(5), 99).isEmpty());
    QVERIFY(model.responseAnnotations(model.entryIdAt(5)).isEmpty());
    QVERIFY(model.responseAnnotations(QStringLiteral("missing"), 1).isEmpty());
}

void
CodexTimelineModelTest::reconcilesBodyAndMetadataWithoutDuplicatingCandidates()
{
    CodexTimelineModel model;
    auto user = annotatedMessage(QStringLiteral("user-1"), QStringLiteral("Body"));
    model.reconcileTimeline({ user }, {});
    auto* body = documentAt(model, 0);
    QVERIFY(body);
    QTRY_COMPARE(body->rowCount(), 1);
    const auto id = model.entryIdAt(0);
    QSignalSpy reset(&model, &QAbstractItemModel::modelReset);
    QSignalSpy changed(&model, &QAbstractItemModel::dataChanged);
    auto value = user.message();
    auto content = value.annotatedUserMessage();
    auto annotations = content.annotations();
    auto second = annotations.first();
    second.setIndex(2);
    annotations.append(second);
    content.setAnnotations(annotations);
    content.setBody(QStringLiteral("Updated body"));
    value.setAnnotatedUserMessage(content);
    user.setMessage(value);
    model.reconcileTimeline({ user }, {});
    QCOMPARE(reset.size(), 0);
    QCOMPARE(documentAt(model, 0), body);
    QCOMPARE(model.entryIdAt(0), id);
    QCOMPARE(model.responseAnnotations(id).size(), 2);
    QVERIFY(changed.last().at(2).value<QList<int>>().contains(CodexTimelineModel::AnnotationCountRole));
    content.setBody(QStringLiteral("\n  \n"));
    value.setAnnotatedUserMessage(content);
    user.setMessage(value);
    model.reconcileTimeline({ user }, {});
    QCOMPARE(documentAt(model, 0), nullptr);
    QCOMPARE(model.rowCount(), 1);
    model.clear();
    QVERIFY(model.responseAnnotations(id).isEmpty());
}

void
CodexTimelineModelTest::ignoresUserPresentationMetadataOnAgentMessages()
{
    auto item = annotatedMessage();
    auto value = item.message();
    value.setRole(MessageRole::MESSAGE_ROLE_AGENT);
    item.setMessage(value);
    CodexTimelineModel model;
    model.reconcileTimeline({ item }, {});
    QCOMPARE(model.rowCount(), 1);
    QCOMPARE(model.data(model.index(0), CodexTimelineModel::TextRole).toString(), value.text());
    QCOMPARE(model.data(model.index(0), CodexTimelineModel::AnnotationCountRole).toInt(), 0);
    QVERIFY(model.responseAnnotations(model.entryIdAt(0)).isEmpty());
}

QTEST_MAIN(CodexTimelineModelTest)
#include "codextimelinemodeltest.moc"
