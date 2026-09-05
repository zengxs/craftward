// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#pragma once

#include <QStandardItem>
#include <QStandardItemModel>
#include <QString>

namespace TimelinePresentationSourceFixture {
enum SourceRole
{
    EntryIdRole = Qt::UserRole + 1,
    TurnIdRole,
    ActivityGroupRole,
    StandaloneActivityRole,
    FromUserRole,
    CommentaryRole,
    FinalAnswerRole,
    TextRole,
    MarkupDocumentRole,
    CollisionProbeRole = Qt::UserRole + 0x1fff,
};

inline void
configureRoles(QStandardItemModel& model)
{
    model.setItemRoleNames({
      { EntryIdRole, "entryId" },
      { TurnIdRole, "turnId" },
      { ActivityGroupRole, "activityGroup" },
      { StandaloneActivityRole, "standaloneActivity" },
      { FromUserRole, "fromUser" },
      { CommentaryRole, "commentary" },
      { FinalAnswerRole, "finalAnswer" },
      { TextRole, "text" },
      { MarkupDocumentRole, "markupDocument" },
      { CollisionProbeRole, "collisionProbe" },
    });
}

inline void
appendRow(QStandardItemModel& model,
          const QString& entryId,
          const QString& turnId,
          bool activityGroup = false,
          bool standaloneActivity = false,
          bool fromUser = false,
          bool commentary = false,
          bool finalAnswer = false)
{
    auto* item = new QStandardItem;
    item->setData(entryId, EntryIdRole);
    item->setData(turnId, TurnIdRole);
    item->setData(activityGroup, ActivityGroupRole);
    item->setData(standaloneActivity, StandaloneActivityRole);
    item->setData(fromUser, FromUserRole);
    item->setData(commentary, CommentaryRole);
    item->setData(finalAnswer, FinalAnswerRole);
    model.appendRow(item);
}
}
