// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include "history.qpb.h"
#include "ward/markup/markupdocumentmodel.h"

#include <QAbstractListModel>
#include <QList>
#include <QSet>
#include <QString>
#include <QStringList>
#include <QVariantList>
#include <QVariantMap>
#include <QtQml/qqmlregistration.h>

#include <memory>

using CodexActivity = ward::codex::v1::Activity;
using CodexMessage = ward::codex::v1::Message;
using CodexTimelineItem = ward::codex::v1::TimelineItem;

class CodexTimelineModel : public QAbstractListModel
{
    Q_OBJECT
    QML_ANONYMOUS

  public:
    enum Role
    {
        EntryIdRole = Qt::UserRole + 1,
        TurnIdRole,
        ForkBoundaryRole,
        ActivityGroupRole,
        FromUserRole,
        CommentaryRole,
        FinalAnswerRole,
        TextRole,
        MarkupDocumentRole,
        ActivityLabelRole,
        ActivityCountRole,
        ActivityItemsRole,
        FailedRole,
        RunningRole,
    };

    explicit CodexTimelineModel(QObject* parent = nullptr);

    [[nodiscard]] int rowCount(const QModelIndex& parent = {}) const override;
    Q_INVOKABLE [[nodiscard]] QString entryIdAt(int row) const;
    [[nodiscard]] QVariant data(const QModelIndex& index, int role) const override;
    [[nodiscard]] QHash<int, QByteArray> roleNames() const override;

    void reconcileTimeline(QList<CodexTimelineItem> timeline, const QStringList& forkableTurnIds);
    void clear();
    void retranslate();

  private:
    enum class ActivityPresentationKind
    {
        Activity,
        Reasoning,
        Plan,
        ReadFiles,
        ListFiles,
        SearchFiles,
        RunCommands,
        FileChange,
        ToolCall,
        Collaboration,
        WebSearch,
        ImageView,
        Wait,
        ImageGeneration,
        ReviewStarted,
        ReviewCompleted,
        ContextCompaction,
    };

    struct TimelineRow
    {
        QString entryId;
        QString turnId;
        bool forkBoundary = false;
        bool activityGroup = false;
        CodexMessage message;
        bool markupFinalized = false;
        mutable std::shared_ptr<MarkupDocumentModel> markupDocument;
        ActivityPresentationKind activityKind = ActivityPresentationKind::Activity;
        QList<CodexActivity> activities;
    };

    [[nodiscard]] QList<TimelineRow> buildRows(QList<CodexTimelineItem> timeline,
                                               const QSet<QString>& forkableTurnIds) const;
    [[nodiscard]] ActivityPresentationKind presentationKind(const CodexActivity& activity) const;
    [[nodiscard]] QString activityGroupLabel(ActivityPresentationKind kind) const;
    [[nodiscard]] QVariantList activityItems(const TimelineRow& row) const;
    [[nodiscard]] QVariantMap activityItem(const CodexActivity& activity) const;
    [[nodiscard]] QString activityStatusLabel(const CodexActivity& activity) const;
    [[nodiscard]] QString commandActionSummary(const CodexActivity& activity) const;
    [[nodiscard]] bool activityFailed(const CodexActivity& activity) const;
    [[nodiscard]] bool activityRunning(const CodexActivity& activity) const;
    [[nodiscard]] bool rowFailed(const TimelineRow& row) const;
    [[nodiscard]] bool rowRunning(const TimelineRow& row) const;
    [[nodiscard]] bool rowsEqual(const TimelineRow& left, const TimelineRow& right) const;
    [[nodiscard]] static MarkupDocumentModel::SourceFormat messageSourceFormat(const CodexMessage& message);
    [[nodiscard]] MarkupDocumentModel* ensureMarkupDocument(const TimelineRow& row) const;
    void replaceRows(QList<TimelineRow> rows);

    QList<TimelineRow> rows_;
};
