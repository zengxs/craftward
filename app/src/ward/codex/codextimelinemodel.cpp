// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#include "ward/codex/codextimelinemodel.h"

#include <QStringList>
#include <QVariantMap>
#include <QtTranslation>

#include <algorithm>
#include <utility>

namespace {
QString
displayMessageText(const CodexMessage& message)
{
    using MessageRole = ward::codex::v1::MessageRoleGadget::MessageRole;
    QString text = message.text();
    if (message.role() != MessageRole::MESSAGE_ROLE_USER)
        return text;

    while (text.startsWith(QLatin1Char('\n')) || text.startsWith(QLatin1Char('\r')))
        text.remove(0, 1);
    while (text.endsWith(QLatin1Char('\n')) || text.endsWith(QLatin1Char('\r')))
        text.chop(1);
    return text;
}
}

CodexTimelineModel::CodexTimelineModel(QObject* parent)
  : QAbstractListModel(parent)
{
}

void
CodexTimelineModel::retranslate()
{
    if (!rows_.isEmpty())
        emit dataChanged(index(0), index(rows_.size() - 1), { ActivityLabelRole, ActivityItemsRole });
}

int
CodexTimelineModel::rowCount(const QModelIndex& parent) const
{
    return parent.isValid() ? 0 : rows_.size();
}

QString
CodexTimelineModel::entryIdAt(int row) const
{
    return row >= 0 && row < rows_.size() ? rows_.at(row).entryId : QString();
}

QVariant
CodexTimelineModel::data(const QModelIndex& index, int role) const
{
    if (!checkIndex(index,
                    QAbstractItemModel::CheckIndexOption::IndexIsValid |
                      QAbstractItemModel::CheckIndexOption::ParentIsInvalid))
        return {};

    const TimelineRow& row = rows_.at(index.row());
    using MessagePhase = ward::codex::v1::MessagePhaseGadget::MessagePhase;
    using MessageRole = ward::codex::v1::MessageRoleGadget::MessageRole;
    switch (role) {
        case EntryIdRole:
            return row.entryId;
        case TurnIdRole:
            return row.turnId;
        case ForkBoundaryRole:
            return row.forkBoundary;
        case TurnForkableRole:
            return row.turnForkable;
        case LatestTurnRole:
            return row.latestTurn;
        case ActivityGroupRole:
            return row.activityGroup;
        case StandaloneActivityRole:
            return row.activityGroup && row.activityKind == ActivityPresentationKind::ContextCompaction;
        case FromUserRole:
            return !row.activityGroup && row.message.role() == MessageRole::MESSAGE_ROLE_USER;
        case CommentaryRole:
            return !row.activityGroup && row.message.phase() == MessagePhase::MESSAGE_PHASE_COMMENTARY;
        case FinalAnswerRole:
            return !row.activityGroup && row.message.phase() == MessagePhase::MESSAGE_PHASE_FINAL_ANSWER;
        case TextRole:
            return row.activityGroup ? QString() : displayMessageText(row.message);
        case MarkupDocumentRole:
            return row.activityGroup ? QVariant()
                                     : QVariant::fromValue(static_cast<QObject*>(ensureMarkupDocument(row)));
        case ActivityLabelRole:
            return row.activityGroup ? activityGroupLabel(row.activityKind) : QString();
        case ActivityCountRole:
            return row.activityGroup ? static_cast<int>(row.activities.size()) : 0;
        case ActivityItemsRole:
            return row.activityGroup ? activityItems(row) : QVariantList();
        case FailedRole:
            return row.activityGroup && rowFailed(row);
        case RunningRole:
            return row.activityGroup && rowRunning(row);
        case TurnStartedAtUnixSecondsRole:
            return row.turnTiming.hasStartedAtUnixSeconds()
                     ? QVariant::fromValue(static_cast<qint64>(row.turnTiming.startedAtUnixSeconds()))
                     : QVariant();
        case TurnCompletedAtUnixSecondsRole:
            return row.turnTiming.hasCompletedAtUnixSeconds()
                     ? QVariant::fromValue(static_cast<qint64>(row.turnTiming.completedAtUnixSeconds()))
                     : QVariant();
        case TurnDurationMillisecondsRole:
            return row.turnTiming.hasDurationMilliseconds()
                     ? QVariant::fromValue(static_cast<qint64>(row.turnTiming.durationMilliseconds()))
                     : QVariant();
        case ActivityPresentationKindRole:
            return row.activityGroup ? activityPresentationKindName(row.activityKind) : QString();
        default:
            return {};
    }
}

QHash<int, QByteArray>
CodexTimelineModel::roleNames() const
{
    return {
        { EntryIdRole, "entryId" },
        { TurnIdRole, "turnId" },
        { ForkBoundaryRole, "forkBoundary" },
        { TurnForkableRole, "turnForkable" },
        { LatestTurnRole, "latestTurn" },
        { ActivityGroupRole, "activityGroup" },
        { StandaloneActivityRole, "standaloneActivity" },
        { FromUserRole, "fromUser" },
        { CommentaryRole, "commentary" },
        { FinalAnswerRole, "finalAnswer" },
        { TextRole, "text" },
        { MarkupDocumentRole, "markupDocument" },
        { ActivityLabelRole, "activityLabel" },
        { ActivityCountRole, "activityCount" },
        { ActivityItemsRole, "activityItems" },
        { FailedRole, "failed" },
        { RunningRole, "running" },
        { TurnStartedAtUnixSecondsRole, "turnStartedAtUnixSeconds" },
        { TurnCompletedAtUnixSecondsRole, "turnCompletedAtUnixSeconds" },
        { TurnDurationMillisecondsRole, "turnDurationMilliseconds" },
        { ActivityPresentationKindRole, "activityPresentationKind" },
    };
}

QList<CodexTimelineModel::TimelineRow>
CodexTimelineModel::buildRows(QList<CodexTimelineItem> timeline,
                              const QSet<QString>& forkableTurnIds,
                              const QHash<QString, CodexTurnTiming>& turnTimings) const
{
    QList<TimelineRow> rows;
    rows.reserve(timeline.size());
    for (qsizetype sourceIndex = 0; sourceIndex < timeline.size(); ++sourceIndex) {
        CodexTimelineItem& item = timeline[sourceIndex];
        if (item.hasMessage()) {
            CodexMessage message = item.message();
            const QString sourceId = message.messageId().isEmpty() ? QString::number(sourceIndex) : message.messageId();
            rows.append(TimelineRow{
              .entryId = QStringLiteral("message:%1:%2").arg(item.turnId(), sourceId),
              .turnId = item.turnId(),
              .activityGroup = false,
              .message = std::move(message),
              .turnTiming = turnTimings.value(item.turnId()),
            });
            continue;
        }
        if (!item.hasActivity())
            continue;

        CodexActivity activity = item.activity();
        const ActivityPresentationKind kind = presentationKind(activity);
        if (kind != ActivityPresentationKind::ContextCompaction && !rows.isEmpty() && rows.last().activityGroup &&
            rows.last().turnId == item.turnId() && rows.last().activityKind == kind) {
            rows.last().activities.append(std::move(activity));
            continue;
        }

        const QString sourceId = activity.activityId().isEmpty() ? QString::number(sourceIndex) : activity.activityId();
        TimelineRow row{
            .entryId = QStringLiteral("activity:%1:%2").arg(item.turnId(), sourceId),
            .turnId = item.turnId(),
            .activityGroup = true,
            .activityKind = kind,
            .turnTiming = turnTimings.value(item.turnId()),
        };
        row.activities.append(std::move(activity));
        rows.append(std::move(row));
    }
    const QString lastTurnId = rows.isEmpty() ? QString() : rows.constLast().turnId;
    using MessageRole = ward::codex::v1::MessageRoleGadget::MessageRole;
    for (qsizetype index = 0; index < rows.size(); ++index) {
        TimelineRow& row = rows[index];
        row.turnForkable = forkableTurnIds.contains(row.turnId);
        row.latestTurn = row.turnId == lastTurnId;
        row.forkBoundary = row.turnForkable && (index + 1 == rows.size() || rows[index + 1].turnId != row.turnId);
        row.markupFinalized = !row.activityGroup && (row.message.role() == MessageRole::MESSAGE_ROLE_USER ||
                                                     row.turnId != lastTurnId || forkableTurnIds.contains(row.turnId));
    }
    return rows;
}

CodexTimelineModel::ActivityPresentationKind
CodexTimelineModel::presentationKind(const CodexActivity& activity) const
{
    using ActivityKind = ward::codex::v1::ActivityKindGadget::ActivityKind;
    using CommandActionKind = ward::codex::v1::CommandActionKindGadget::CommandActionKind;

    switch (activity.kind()) {
        case ActivityKind::ACTIVITY_KIND_REASONING:
            return ActivityPresentationKind::Reasoning;
        case ActivityKind::ACTIVITY_KIND_PLAN:
            return ActivityPresentationKind::Plan;
        case ActivityKind::ACTIVITY_KIND_COMMAND_EXECUTION: {
            CommandActionKind sharedKind = CommandActionKind::COMMAND_ACTION_KIND_UNSPECIFIED;
            for (const auto& action : activity.commandActions()) {
                const CommandActionKind kind = action.kind();
                if (kind != CommandActionKind::COMMAND_ACTION_KIND_READ &&
                    kind != CommandActionKind::COMMAND_ACTION_KIND_LIST_FILES &&
                    kind != CommandActionKind::COMMAND_ACTION_KIND_SEARCH) {
                    return ActivityPresentationKind::RunCommands;
                }
                if (sharedKind == CommandActionKind::COMMAND_ACTION_KIND_UNSPECIFIED)
                    sharedKind = kind;
                else if (sharedKind != kind)
                    return ActivityPresentationKind::RunCommands;
            }
            switch (sharedKind) {
                case CommandActionKind::COMMAND_ACTION_KIND_READ:
                    return ActivityPresentationKind::ReadFiles;
                case CommandActionKind::COMMAND_ACTION_KIND_LIST_FILES:
                    return ActivityPresentationKind::ListFiles;
                case CommandActionKind::COMMAND_ACTION_KIND_SEARCH:
                    return ActivityPresentationKind::SearchFiles;
                case CommandActionKind::COMMAND_ACTION_KIND_OTHER:
                case CommandActionKind::COMMAND_ACTION_KIND_UNSPECIFIED:
                default:
                    return ActivityPresentationKind::RunCommands;
            }
        }
        case ActivityKind::ACTIVITY_KIND_FILE_CHANGE:
            return ActivityPresentationKind::FileChange;
        case ActivityKind::ACTIVITY_KIND_TOOL_CALL:
            return ActivityPresentationKind::ToolCall;
        case ActivityKind::ACTIVITY_KIND_COLLABORATION:
            return ActivityPresentationKind::Collaboration;
        case ActivityKind::ACTIVITY_KIND_WEB_SEARCH:
            return ActivityPresentationKind::WebSearch;
        case ActivityKind::ACTIVITY_KIND_IMAGE_VIEW:
            return ActivityPresentationKind::ImageView;
        case ActivityKind::ACTIVITY_KIND_WAIT:
            return ActivityPresentationKind::Wait;
        case ActivityKind::ACTIVITY_KIND_IMAGE_GENERATION:
            return ActivityPresentationKind::ImageGeneration;
        case ActivityKind::ACTIVITY_KIND_REVIEW_STARTED:
            return ActivityPresentationKind::ReviewStarted;
        case ActivityKind::ACTIVITY_KIND_REVIEW_COMPLETED:
            return ActivityPresentationKind::ReviewCompleted;
        case ActivityKind::ACTIVITY_KIND_CONTEXT_COMPACTION:
            return ActivityPresentationKind::ContextCompaction;
        case ActivityKind::ACTIVITY_KIND_UNSPECIFIED:
        default:
            return ActivityPresentationKind::Activity;
    }
}

QString
CodexTimelineModel::activityPresentationKindName(ActivityPresentationKind kind)
{
    switch (kind) {
        case ActivityPresentationKind::Reasoning:
            return QStringLiteral("reasoning");
        case ActivityPresentationKind::Plan:
            return QStringLiteral("plan");
        case ActivityPresentationKind::ReadFiles:
            return QStringLiteral("readFiles");
        case ActivityPresentationKind::ListFiles:
            return QStringLiteral("listFiles");
        case ActivityPresentationKind::SearchFiles:
            return QStringLiteral("searchFiles");
        case ActivityPresentationKind::RunCommands:
            return QStringLiteral("runCommands");
        case ActivityPresentationKind::FileChange:
            return QStringLiteral("fileChange");
        case ActivityPresentationKind::ToolCall:
            return QStringLiteral("toolCall");
        case ActivityPresentationKind::Collaboration:
            return QStringLiteral("collaboration");
        case ActivityPresentationKind::WebSearch:
            return QStringLiteral("webSearch");
        case ActivityPresentationKind::ImageView:
            return QStringLiteral("imageView");
        case ActivityPresentationKind::Wait:
            return QStringLiteral("wait");
        case ActivityPresentationKind::ImageGeneration:
            return QStringLiteral("imageGeneration");
        case ActivityPresentationKind::ReviewStarted:
            return QStringLiteral("reviewStarted");
        case ActivityPresentationKind::ReviewCompleted:
            return QStringLiteral("reviewCompleted");
        case ActivityPresentationKind::ContextCompaction:
            return QStringLiteral("contextCompaction");
        case ActivityPresentationKind::Activity:
        default:
            return QStringLiteral("activity");
    }
}

QString
CodexTimelineModel::activityGroupLabel(ActivityPresentationKind kind) const
{
    switch (kind) {
        case ActivityPresentationKind::Reasoning:
            return /*% "Reasoning" */ qtTrId("craftward.codex.timeline.activity.reasoning");
        case ActivityPresentationKind::Plan:
            return /*% "Planned" */ qtTrId("craftward.codex.timeline.activity.plan");
        case ActivityPresentationKind::ReadFiles:
            return /*% "Read files" */ qtTrId("craftward.codex.timeline.activity.read_files");
        case ActivityPresentationKind::ListFiles:
            return /*% "Listed files" */ qtTrId("craftward.codex.timeline.activity.list_files");
        case ActivityPresentationKind::SearchFiles:
            return /*% "Searched files" */ qtTrId("craftward.codex.timeline.activity.search_files");
        case ActivityPresentationKind::RunCommands:
            return /*% "Ran commands" */ qtTrId("craftward.codex.timeline.activity.run_commands");
        case ActivityPresentationKind::FileChange:
            return /*% "Changed files" */ qtTrId("craftward.codex.timeline.activity.change_files");
        case ActivityPresentationKind::ToolCall:
            return /*% "Used tools" */ qtTrId("craftward.codex.timeline.activity.use_tools");
        case ActivityPresentationKind::Collaboration:
            return /*% "Coordinated agents" */ qtTrId("craftward.codex.timeline.activity.coordinate_agents");
        case ActivityPresentationKind::WebSearch:
            return /*% "Searched the web" */ qtTrId("craftward.codex.timeline.activity.web_search");
        case ActivityPresentationKind::ImageView:
            return /*% "Viewed images" */ qtTrId("craftward.codex.timeline.activity.view_images");
        case ActivityPresentationKind::Wait:
            return /*% "Waited" */ qtTrId("craftward.codex.timeline.activity.wait");
        case ActivityPresentationKind::ImageGeneration:
            return /*% "Generated images" */ qtTrId("craftward.codex.timeline.activity.generate_images");
        case ActivityPresentationKind::ReviewStarted:
            return /*% "Entered review mode" */ qtTrId("craftward.codex.timeline.activity.review_started");
        case ActivityPresentationKind::ReviewCompleted:
            return /*% "Exited review mode" */ qtTrId("craftward.codex.timeline.activity.review_completed");
        case ActivityPresentationKind::ContextCompaction:
            return /*% "Compacted context" */ qtTrId("craftward.codex.timeline.activity.context_compaction");
        case ActivityPresentationKind::Activity:
        default:
            return /*% "Activity" */ qtTrId("craftward.codex.timeline.activity.other");
    }
}

QVariantList
CodexTimelineModel::activityItems(const TimelineRow& row) const
{
    QVariantList items;
    items.reserve(row.activities.size());
    for (const CodexActivity& activity : row.activities)
        items.append(activityItem(activity));
    return items;
}

QVariantMap
CodexTimelineModel::activityItem(const CodexActivity& activity) const
{
    const QString actionSummary = commandActionSummary(activity);
    const QString rawSummary = activity.summary();
    QString displaySummary = actionSummary.isEmpty() ? rawSummary : actionSummary;
    if (displaySummary.trimmed().isEmpty())
        displaySummary = activityGroupLabel(presentationKind(activity));
    const QString command = !actionSummary.isEmpty() && actionSummary != rawSummary ? rawSummary : QString();
    const QString detail = activity.hasDetail() ? activity.detail() : QString();
    const QString context = activity.hasContext() ? activity.context() : QString();
    const bool reasoning =
      activity.kind() == ward::codex::v1::ActivityKindGadget::ActivityKind::ACTIVITY_KIND_REASONING;
    const qint64 startedAtUnixMilliseconds =
      activity.hasStartedAtUnixMilliseconds() ? static_cast<qint64>(activity.startedAtUnixMilliseconds()) : qint64{ 0 };
    const qint64 completedAtUnixMilliseconds = activity.hasCompletedAtUnixMilliseconds()
                                                 ? static_cast<qint64>(activity.completedAtUnixMilliseconds())
                                                 : qint64{ 0 };

    return {
        { QStringLiteral("activityId"), activity.activityId() },
        { QStringLiteral("summary"), displaySummary },
        { QStringLiteral("command"), command },
        { QStringLiteral("detail"), detail },
        { QStringLiteral("context"), context },
        { QStringLiteral("statusLabel"), activityStatusLabel(activity) },
        { QStringLiteral("failed"), activityFailed(activity) },
        { QStringLiteral("running"), activityRunning(activity) },
        { QStringLiteral("reasoning"), reasoning },
        { QStringLiteral("startedAtUnixMilliseconds"), startedAtUnixMilliseconds },
        { QStringLiteral("completedAtUnixMilliseconds"), completedAtUnixMilliseconds },
        { QStringLiteral("expandable"), reasoning || !command.isEmpty() || !detail.isEmpty() || !context.isEmpty() },
    };
}

QString
CodexTimelineModel::activityStatusLabel(const CodexActivity& activity) const
{
    using ActivityStatus = ward::codex::v1::ActivityStatusGadget::ActivityStatus;
    switch (activity.status()) {
        case ActivityStatus::ACTIVITY_STATUS_IN_PROGRESS:
            return /*% "In progress" */ qtTrId("craftward.codex.timeline.status.in_progress");
        case ActivityStatus::ACTIVITY_STATUS_COMPLETED:
            return {};
        case ActivityStatus::ACTIVITY_STATUS_FAILED:
            return /*% "Failed" */ qtTrId("craftward.codex.timeline.status.failed");
        case ActivityStatus::ACTIVITY_STATUS_DECLINED:
            return /*% "Declined" */ qtTrId("craftward.codex.timeline.status.declined");
        case ActivityStatus::ACTIVITY_STATUS_OTHER:
            return /*% "Unknown status" */ qtTrId("craftward.codex.timeline.status.unknown");
        case ActivityStatus::ACTIVITY_STATUS_UNSPECIFIED:
        default:
            return {};
    }
}

QString
CodexTimelineModel::commandActionSummary(const CodexActivity& activity) const
{
    using CommandActionKind = ward::codex::v1::CommandActionKindGadget::CommandActionKind;

    QStringList summaries;
    for (const auto& action : activity.commandActions()) {
        const QString path = action.hasPath() ? action.path() : QString();
        switch (action.kind()) {
            case CommandActionKind::COMMAND_ACTION_KIND_READ: {
                const QString target = !path.isEmpty() ? path : (action.hasName() ? action.name() : QString());
                summaries.append(
                  target.isEmpty()
                    ? /*% "Read a file" */ qtTrId("craftward.codex.timeline.command.read_file")
                    : /*% "Read %1" */ qtTrId("craftward.codex.timeline.command.read_target").arg(target));
                break;
            }
            case CommandActionKind::COMMAND_ACTION_KIND_LIST_FILES:
                summaries.append(
                  path.isEmpty()
                    ? /*% "List files" */ qtTrId("craftward.codex.timeline.command.list_files")
                    : /*% "List files in %1" */ qtTrId("craftward.codex.timeline.command.list_files_in").arg(path));
                break;
            case CommandActionKind::COMMAND_ACTION_KIND_SEARCH: {
                const QString query = action.hasQuery() ? action.query() : QString();
                if (!query.isEmpty() && !path.isEmpty())
                    summaries.append(/*% "Search for “%1” in %2" */ qtTrId("craftward.codex.timeline.command.search_in")
                                       .arg(query, path));
                else if (!query.isEmpty())
                    summaries.append(
                      /*% "Search for “%1”" */ qtTrId("craftward.codex.timeline.command.search").arg(query));
                else if (!path.isEmpty())
                    summaries.append(
                      /*% "Search in %1" */ qtTrId("craftward.codex.timeline.command.search_location").arg(path));
                else
                    summaries.append(/*% "Search files" */ qtTrId("craftward.codex.timeline.command.search_files"));
                break;
            }
            case CommandActionKind::COMMAND_ACTION_KIND_OTHER:
            case CommandActionKind::COMMAND_ACTION_KIND_UNSPECIFIED:
            default:
                if (!action.command().trimmed().isEmpty())
                    summaries.append(action.command());
                break;
        }
    }
    return summaries.join(QLatin1Char('\n'));
}

bool
CodexTimelineModel::activityFailed(const CodexActivity& activity) const
{
    using ActivityStatus = ward::codex::v1::ActivityStatusGadget::ActivityStatus;
    return activity.status() == ActivityStatus::ACTIVITY_STATUS_FAILED ||
           activity.status() == ActivityStatus::ACTIVITY_STATUS_DECLINED;
}

bool
CodexTimelineModel::activityRunning(const CodexActivity& activity) const
{
    return activity.status() == ward::codex::v1::ActivityStatusGadget::ActivityStatus::ACTIVITY_STATUS_IN_PROGRESS;
}

bool
CodexTimelineModel::rowFailed(const TimelineRow& row) const
{
    return std::any_of(row.activities.cbegin(), row.activities.cend(), [this](const CodexActivity& activity) {
        return activityFailed(activity);
    });
}

bool
CodexTimelineModel::rowRunning(const TimelineRow& row) const
{
    return std::any_of(row.activities.cbegin(), row.activities.cend(), [this](const CodexActivity& activity) {
        return activityRunning(activity);
    });
}

bool
CodexTimelineModel::rowsEqual(const TimelineRow& left, const TimelineRow& right) const
{
    return left.entryId == right.entryId && left.turnId == right.turnId && left.forkBoundary == right.forkBoundary &&
           left.turnForkable == right.turnForkable && left.latestTurn == right.latestTurn &&
           left.activityGroup == right.activityGroup && left.message == right.message &&
           left.markupFinalized == right.markupFinalized && left.activityKind == right.activityKind &&
           left.activities == right.activities && left.turnTiming == right.turnTiming;
}

MarkupDocumentModel::SourceFormat
CodexTimelineModel::messageSourceFormat(const CodexMessage& message)
{
    using MessageRole = ward::codex::v1::MessageRoleGadget::MessageRole;
    switch (message.role()) {
        case MessageRole::MESSAGE_ROLE_USER:
        case MessageRole::MESSAGE_ROLE_AGENT:
            return MarkupDocumentModel::SourceFormat::Markdown;
        case MessageRole::MESSAGE_ROLE_UNSPECIFIED:
        default:
            return MarkupDocumentModel::SourceFormat::PlainText;
    }
}

MarkupDocumentModel*
CodexTimelineModel::ensureMarkupDocument(const TimelineRow& row) const
{
    if (row.activityGroup)
        return nullptr;
    if (!row.markupDocument) {
        row.markupDocument = std::make_shared<MarkupDocumentModel>(const_cast<CodexTimelineModel*>(this));
        row.markupDocument->reconcileSource(
          displayMessageText(row.message), messageSourceFormat(row.message), row.markupFinalized);
    }
    return row.markupDocument.get();
}

void
CodexTimelineModel::replaceRows(QList<TimelineRow> rows)
{
    beginResetModel();
    rows_ = std::move(rows);
    endResetModel();
}

void
CodexTimelineModel::reconcileTimeline(QList<CodexTimelineItem> timeline,
                                      const QStringList& forkableTurnIds,
                                      const QList<CodexTurnTiming>& turnTimings)
{
    QHash<QString, CodexTurnTiming> turnTimingById;
    turnTimingById.reserve(turnTimings.size());
    for (const CodexTurnTiming& timing : turnTimings) {
        if (!timing.turnId().isEmpty())
            turnTimingById.insert(timing.turnId(), timing);
    }
    QList<TimelineRow> rows =
      buildRows(std::move(timeline), QSet<QString>(forkableTurnIds.cbegin(), forkableTurnIds.cend()), turnTimingById);
    const qsizetype sharedSize = std::min(rows_.size(), rows.size());
    qsizetype commonPrefix = 0;
    while (commonPrefix < sharedSize && rows_.at(commonPrefix).entryId == rows.at(commonPrefix).entryId)
        ++commonPrefix;

    if (commonPrefix != sharedSize) {
        replaceRows(std::move(rows));
        return;
    }

    for (qsizetype index = 0; index < commonPrefix; ++index) {
        if (rows[index].activityGroup)
            continue;
        rows[index].markupDocument = rows_[index].markupDocument;
        if (rows[index].markupDocument) {
            rows[index].markupDocument->reconcileSource(displayMessageText(rows[index].message),
                                                        messageSourceFormat(rows[index].message),
                                                        rows[index].markupFinalized);
        }
    }

    qsizetype firstChanged = -1;
    qsizetype lastChanged = -1;
    for (qsizetype index = 0; index < commonPrefix; ++index) {
        if (rowsEqual(rows_.at(index), rows.at(index)))
            continue;
        rows_[index] = std::move(rows[index]);
        if (firstChanged < 0)
            firstChanged = index;
        lastChanged = index;
    }
    if (firstChanged >= 0)
        emit dataChanged(this->index(firstChanged), this->index(lastChanged));

    if (rows.size() > rows_.size()) {
        const qsizetype first = rows_.size();
        const qsizetype last = rows.size() - 1;
        beginInsertRows({}, first, last);
        for (qsizetype index = first; index <= last; ++index)
            rows_.append(std::move(rows[index]));
        endInsertRows();
    } else if (rows.size() < rows_.size()) {
        const qsizetype first = rows.size();
        const qsizetype last = rows_.size() - 1;
        beginRemoveRows({}, first, last);
        rows_.remove(first, rows_.size() - first);
        endRemoveRows();
    }
}

void
CodexTimelineModel::clear()
{
    replaceRows({});
}
