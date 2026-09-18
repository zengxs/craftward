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
using AttachmentSource = UserMessageAttachmentSourceGadget::UserMessageAttachmentSource;

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
withAttachments(TimelineItem item)
{
    UserMessageAttachment image;
    image.setLabel(QStringLiteral("Design sketch"));
    image.setLocalPath(QStringLiteral("/missing/design sketch.png"));
    image.setResourceId(QStringLiteral("attachment:design-sketch"));
    image.setSources({ AttachmentSource::USER_MESSAGE_ATTACHMENT_SOURCE_MENTIONED_FILE,
                       AttachmentSource::USER_MESSAGE_ATTACHMENT_SOURCE_IMAGE_INPUT });
    UserMessageAttachment pasted;
    pasted.setLabel(QStringLiteral("Pasted **literal** excerpt"));
    pasted.setLocalPath(QStringLiteral("/missing/pasted-text.txt"));
    pasted.setResourceId(QStringLiteral("attachment:pasted-text"));
    pasted.setSources({ AttachmentSource::USER_MESSAGE_ATTACHMENT_SOURCE_PASTED_FILE });
    pasted.setStartLine(2);
    pasted.setEndLine(4);
    auto value = item.message();
    auto content = value.userMessagePresentation();
    content.setAttachments({ image, pasted });
    value.setUserMessagePresentation(content);
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
    UserMessagePresentation content;
    content.setBody(body);
    content.setAnnotations({ annotation });
    auto item = message(id, MessageRole::MESSAGE_ROLE_USER, QStringLiteral("\nOriginal client envelope\n"));
    auto value = item.message();
    value.setUserMessagePresentation(content);
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
    void presentsAttachmentsWithoutARequestBody();
    void forwardsAttachmentResourceIdentity_data();
    void forwardsAttachmentResourceIdentity();
    void reconcilesAttachmentsAndKeepsAnnotationReferences();
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
    source.reconcileTimeline({ withAttachments(annotatedMessage(QStringLiteral("user-1"), body)) }, {});
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
        QCOMPARE(viewport.valueAt(row, QStringLiteral("attachments")).toList().size(), 2);
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
    auto content = value.userMessagePresentation();
    auto annotations = content.annotations();
    auto second = annotations.first();
    second.setIndex(2);
    annotations.append(second);
    content.setAnnotations(annotations);
    content.setBody(QStringLiteral("Updated body"));
    value.setUserMessagePresentation(content);
    user.setMessage(value);
    model.reconcileTimeline({ user }, {});
    QCOMPARE(reset.size(), 0);
    QCOMPARE(documentAt(model, 0), body);
    QCOMPARE(model.entryIdAt(0), id);
    QCOMPARE(model.responseAnnotations(id).size(), 2);
    QVERIFY(changed.last().at(2).value<QList<int>>().contains(CodexTimelineModel::AnnotationCountRole));
    content.setBody(QStringLiteral("\n  \n"));
    value.setUserMessagePresentation(content);
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
    auto item = withAttachments(annotatedMessage());
    auto value = item.message();
    value.setRole(MessageRole::MESSAGE_ROLE_AGENT);
    item.setMessage(value);
    CodexTimelineModel model;
    model.reconcileTimeline({ item }, {});
    QCOMPARE(model.rowCount(), 1);
    QCOMPARE(model.data(model.index(0), CodexTimelineModel::TextRole).toString(), value.text());
    QCOMPARE(model.data(model.index(0), CodexTimelineModel::AnnotationCountRole).toInt(), 0);
    QVERIFY(model.data(model.index(0), CodexTimelineModel::AttachmentsRole).toList().isEmpty());
    QVERIFY(model.responseAnnotations(model.entryIdAt(0)).isEmpty());
}

void
CodexTimelineModelTest::presentsAttachmentsWithoutARequestBody()
{
    auto item =
      withAttachments(message(QStringLiteral("files"), MessageRole::MESSAGE_ROLE_USER, QStringLiteral("Raw input")));
    CodexTimelineModel model;
    model.reconcileTimeline({ item }, {});
    QCOMPARE(documentAt(model, 0), nullptr);
    QCOMPARE(model.data(model.index(0), CodexTimelineModel::AnnotationCountRole).toInt(), 0);
    QVERIFY(model.data(model.index(0), CodexTimelineModel::DisplayTextRole).toString().isEmpty());
    const auto attachments = model.data(model.index(0), CodexTimelineModel::AttachmentsRole).toList();
    QCOMPARE(attachments.size(), 2);
    const auto image = attachments.first().toMap();
    QCOMPARE(image.value(QStringLiteral("label")).toString(), QStringLiteral("Design sketch"));
    QCOMPARE(image.value(QStringLiteral("url")).toUrl(),
             QUrl::fromLocalFile(QStringLiteral("/missing/design sketch.png")));
    QVERIFY(image.value(QStringLiteral("image")).toBool());
    QCOMPARE(image.value(QStringLiteral("resourceId")).toString(), QStringLiteral("attachment:design-sketch"));
    QCOMPARE(image.value(QStringLiteral("sources")).toList().size(), 2);
    QVERIFY(attachments[1].toMap().value(QStringLiteral("pasted")).toBool());
    const auto copy = model.data(model.index(0), CodexTimelineModel::TextRole).toString();
    QVERIFY(copy.contains(QStringLiteral("Design sketch: /missing/design sketch.png")));
    QVERIFY(copy.contains(QStringLiteral("Pasted **literal** excerpt: /missing/pasted-text.txt:2-4")));
    CodexTimelinePresentationModel presentation;
    presentation.setSourceModel(&model);
    CodexTimelineViewportModel viewport;
    viewport.setSourceModel(&presentation);
    QTRY_COMPARE(viewport.rowCount(), 1);
    QCOMPARE(viewport.valueAt(0, QStringLiteral("attachments")).toList(), attachments);
    QVERIFY(!viewport.valueAt(0, QStringLiteral("semanticBlock")).toBool());

    auto updated = item.message();
    auto content = updated.userMessagePresentation();
    auto files = content.attachments();
    files[0].setLabel(QStringLiteral("Another label for the same image"));
    content.setAttachments(files);
    updated.setUserMessagePresentation(content);
    item.setMessage(updated);
    model.reconcileTimeline({ item }, {});
    QCOMPARE(model.data(model.index(0), CodexTimelineModel::AttachmentsRole)
               .toList()
               .first()
               .toMap()
               .value(QStringLiteral("resourceId")),
             image.value(QStringLiteral("resourceId")));
    files[0].setLocalPath(QStringLiteral("/missing/another image.png"));
    files[0].setResourceId(QStringLiteral("attachment:another-image"));
    content.setAttachments(files);
    updated.setUserMessagePresentation(content);
    item.setMessage(updated);
    model.reconcileTimeline({ item }, {});
    QVERIFY(model.data(model.index(0), CodexTimelineModel::AttachmentsRole)
              .toList()
              .first()
              .toMap()
              .value(QStringLiteral("resourceId")) != image.value(QStringLiteral("resourceId")));
}

void
CodexTimelineModelTest::forwardsAttachmentResourceIdentity_data()
{
    QTest::addColumn<QString>("location");
    QTest::addColumn<bool>("local");
    QTest::newRow("local-image") << QStringLiteral("/work/./screen.png") << true;
    QTest::newRow("local-file") << QStringLiteral("/work/notes.txt") << true;
    QTest::newRow("image-url") << QStringLiteral("https://example.com/screen.png?revision=2") << false;
    QTest::newRow("inline-image") << QStringLiteral("data:image/png;base64,aGVsbG8=") << false;
}

void
CodexTimelineModelTest::forwardsAttachmentResourceIdentity()
{
    QFETCH(QString, location);
    QFETCH(bool, local);
    const auto resourceId = QStringLiteral("opaque-identity-from-adapter");
    UserMessageAttachment attachment;
    if (local)
        attachment.setLocalPath(location);
    else
        attachment.setImageUrl(location);
    attachment.setResourceId(resourceId);
    UserMessagePresentation content;
    content.setAttachments({ attachment });
    auto item = message(QStringLiteral("user"), MessageRole::MESSAGE_ROLE_USER, location);
    auto value = item.message();
    value.setUserMessagePresentation(content);
    item.setMessage(value);
    CodexTimelineModel model;
    model.reconcileTimeline({ item }, {});
    const auto result = model.data(model.index(0), CodexTimelineModel::AttachmentsRole).toList().first().toMap();
    QCOMPARE(result.value(QStringLiteral("resourceId")).toString(), resourceId);
    QCOMPARE(result.value(QStringLiteral("path")).toString(), local ? location : QString());
    QCOMPARE(result.value(QStringLiteral("url")).toUrl(), local ? QUrl::fromLocalFile(location) : QUrl(location));
}

void
CodexTimelineModelTest::reconcilesAttachmentsAndKeepsAnnotationReferences()
{
    CodexTimelineModel model;
    const auto user = annotatedMessage(QStringLiteral("user"), QStringLiteral("Body"));
    const auto agent = message(QStringLiteral("agent"), MessageRole::MESSAGE_ROLE_AGENT, QStringLiteral("Done"));
    model.reconcileTimeline({ user, agent }, {});
    CodexTimelinePresentationModel presentation;
    presentation.setSourceModel(&model);
    CodexTimelineViewportModel viewport;
    viewport.setSourceModel(&presentation);
    QTRY_COMPARE(viewport.rowCount(), 2);
    const auto references = model.responseAnnotations(model.entryIdAt(1), 1);
    auto* document = documentAt(model, 0);
    QSignalSpy changed(&model, &QAbstractItemModel::dataChanged);
    model.reconcileTimeline({ withAttachments(user), agent }, {});
    QCOMPARE(documentAt(model, 0), document);
    QCOMPARE(model.responseAnnotations(model.entryIdAt(1), 1), references);
    QVERIFY(!references.isEmpty());
    QVERIFY(changed.last().at(2).value<QList<int>>().contains(CodexTimelineModel::AttachmentsRole));
    QCOMPARE(model.data(model.index(0), CodexTimelineModel::AttachmentsRole).toList().size(), 2);
    QCOMPARE(viewport.valueAt(0, QStringLiteral("attachments")).toList().size(), 2);
    model.reconcileTimeline({ user, agent }, {});
    QVERIFY(model.data(model.index(0), CodexTimelineModel::AttachmentsRole).toList().isEmpty());
    QVERIFY(viewport.valueAt(0, QStringLiteral("attachments")).toList().isEmpty());
}

QTEST_MAIN(CodexTimelineModelTest)
#include "codextimelinemodeltest.moc"
