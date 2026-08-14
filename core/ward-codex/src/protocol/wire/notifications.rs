// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use serde::Deserialize;
use serde_json::Value;

use super::thread::{
    FileUpdateChange, WireThreadItem, WireThreadStatus, WireTurn, file_change_content,
};
use crate::{ActivityStatus, ActivityUpdate, ThreadItem, ThreadStreamEvent};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TurnNotification {
    thread_id: String,
    turn: WireTurn,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItemNotification {
    thread_id: String,
    turn_id: String,
    item: WireThreadItem,
    #[serde(default)]
    started_at_ms: Option<i64>,
    #[serde(default)]
    completed_at_ms: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct DeltaNotification {
    thread_id: String,
    turn_id: String,
    item_id: String,
    delta: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReasoningSummaryPartNotification {
    thread_id: String,
    turn_id: String,
    item_id: String,
    summary_index: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ErrorNotification {
    thread_id: String,
    turn_id: String,
    error: TurnErrorFields,
    will_retry: bool,
}

#[derive(Deserialize)]
struct TurnErrorFields {
    message: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ThreadStatusNotification {
    thread_id: String,
    status: WireThreadStatus,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FileChangePatchNotification {
    thread_id: String,
    turn_id: String,
    item_id: String,
    changes: Vec<FileUpdateChange>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct McpToolCallProgressNotification {
    thread_id: String,
    turn_id: String,
    item_id: String,
    message: String,
}

pub(crate) fn turn_stream_event(
    method: &str,
    params: Value,
) -> Result<Option<ThreadStreamEvent>, serde_json::Error> {
    match method {
        "thread/status/changed" => {
            let notification: ThreadStatusNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::ThreadStatusChanged {
                thread_id: notification.thread_id,
                status: notification.status.into_model(),
            }))
        }
        "turn/started" => {
            let notification: TurnNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::TurnStarted {
                thread_id: notification.thread_id,
                turn: notification.turn.into_model()?,
            }))
        }
        "item/started" | "item/completed" => {
            let notification: ItemNotification = serde_json::from_value(params)?;
            let mut item = notification.item.into_model()?;
            if let ThreadItem::Activity(activity) = &mut item
                && activity.status == ActivityStatus::Unspecified
            {
                activity.status = if method == "item/started" {
                    ActivityStatus::InProgress
                } else {
                    ActivityStatus::Completed
                };
            }
            if let ThreadItem::Activity(activity) = &mut item {
                if method == "item/started" {
                    activity.started_at_unix_milliseconds = notification.started_at_ms;
                } else {
                    activity.completed_at_unix_milliseconds = notification.completed_at_ms;
                }
            }
            let event = if method == "item/started" {
                ThreadStreamEvent::ItemStarted {
                    thread_id: notification.thread_id,
                    turn_id: notification.turn_id,
                    item,
                }
            } else {
                ThreadStreamEvent::ItemCompleted {
                    thread_id: notification.thread_id,
                    turn_id: notification.turn_id,
                    item,
                }
            };
            Ok(Some(event))
        }
        "item/agentMessage/delta" => {
            let notification: DeltaNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::AgentMessageDelta {
                thread_id: notification.thread_id,
                turn_id: notification.turn_id,
                item_id: notification.item_id,
                delta: notification.delta,
            }))
        }
        "item/plan/delta" | "item/reasoning/summaryTextDelta" => {
            let notification: DeltaNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::ActivityUpdated {
                thread_id: notification.thread_id,
                turn_id: notification.turn_id,
                item_id: notification.item_id,
                update: ActivityUpdate::AppendSummary(notification.delta),
            }))
        }
        "item/reasoning/summaryPartAdded" => {
            let notification: ReasoningSummaryPartNotification = serde_json::from_value(params)?;
            if notification.summary_index == 0 {
                return Ok(None);
            }
            Ok(Some(ThreadStreamEvent::ActivityUpdated {
                thread_id: notification.thread_id,
                turn_id: notification.turn_id,
                item_id: notification.item_id,
                update: ActivityUpdate::StartSummarySection,
            }))
        }
        "item/commandExecution/outputDelta" | "item/fileChange/outputDelta" => {
            let notification: DeltaNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::ActivityOutputDelta {
                thread_id: notification.thread_id,
                turn_id: notification.turn_id,
                item_id: notification.item_id,
                delta: notification.delta,
            }))
        }
        "item/fileChange/patchUpdated" => {
            let notification: FileChangePatchNotification = serde_json::from_value(params)?;
            let (summary, detail) = file_change_content(&notification.changes);
            Ok(Some(ThreadStreamEvent::ActivityUpdated {
                thread_id: notification.thread_id,
                turn_id: notification.turn_id,
                item_id: notification.item_id,
                update: ActivityUpdate::ReplaceContent { summary, detail },
            }))
        }
        "item/mcpToolCall/progress" => {
            let notification: McpToolCallProgressNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::ActivityUpdated {
                thread_id: notification.thread_id,
                turn_id: notification.turn_id,
                item_id: notification.item_id,
                update: ActivityUpdate::AppendDetail(format!("{}\n", notification.message)),
            }))
        }
        "error" => {
            let notification: ErrorNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::RuntimeError {
                thread_id: notification.thread_id,
                turn_id: notification.turn_id,
                message: notification.error.message,
                will_retry: notification.will_retry,
            }))
        }
        "turn/completed" => {
            let notification: TurnNotification = serde_json::from_value(params)?;
            Ok(Some(ThreadStreamEvent::TurnCompleted {
                thread_id: notification.thread_id,
                turn: notification.turn.into_model()?,
            }))
        }
        _ => Ok(None),
    }
}
