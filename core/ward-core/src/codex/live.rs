// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use ward_codex::{
    ActivityStatus, ActivityUpdate, Thread, ThreadActiveFlag, ThreadItem, ThreadRuntimeStatus,
    ThreadStreamEvent, ThreadSubscription, Turn, TurnStatus,
};

/// The application-level runtime state of a subscribed thread.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum LiveRuntimeState {
    Detached,
    Starting,
    Idle,
    Active {
        turn_id: Option<String>,
        active_flags: Vec<ThreadActiveFlag>,
    },
    SystemError,
    Unknown(String),
}

/// The visible effects produced by folding one app-server event.
#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct LiveProjectionEffect {
    pub(crate) conversation_changed: bool,
    pub(crate) runtime_changed: bool,
    pub(crate) started_turn_id: Option<String>,
}

/// A single projection of the subscribed runtime stream and persisted history.
#[derive(Default)]
pub(crate) struct LiveThreadProjection {
    conversation: Option<Thread>,
    retained_live_turn: Option<RetainedLiveTurn>,
    runtime: Option<LiveRuntimeState>,
    live_turn_id: Option<String>,
}

struct RetainedLiveTurn {
    index: usize,
    turn: Turn,
}

impl LiveThreadProjection {
    pub(crate) fn reset(&mut self) {
        *self = Self::default();
    }

    pub(crate) fn attach(&mut self, subscription: ThreadSubscription) {
        let active_turn_id = subscription
            .thread
            .turns
            .iter()
            .rev()
            .find(|turn| turn.status == TurnStatus::InProgress)
            .map(|turn| turn.id.clone());
        let runtime = runtime_from_status(subscription.runtime_status, active_turn_id.clone());
        self.conversation = Some(subscription.thread);
        self.retained_live_turn = None;
        self.live_turn_id = match &runtime {
            LiveRuntimeState::Active { turn_id, .. } => turn_id.clone(),
            _ => None,
        };
        self.runtime = Some(runtime);
        if let Some(turn_id) = self.live_turn_id.clone() {
            self.retain_turn(&turn_id);
        }
    }

    pub(crate) fn detach(&mut self) -> bool {
        self.live_turn_id = None;
        self.set_runtime(LiveRuntimeState::Detached)
    }

    pub(crate) fn begin_turn(&mut self) -> bool {
        self.live_turn_id = None;
        self.set_runtime(LiveRuntimeState::Starting)
    }

    pub(crate) fn conversation(&self) -> Option<&Thread> {
        self.conversation.as_ref()
    }

    pub(crate) fn runtime(&self) -> LiveRuntimeState {
        self.runtime.clone().unwrap_or(LiveRuntimeState::Detached)
    }

    pub(crate) fn apply_event(
        &mut self,
        event: &ThreadStreamEvent,
        expected_thread_id: &str,
    ) -> LiveProjectionEffect {
        let mut effect = LiveProjectionEffect::default();
        match event {
            ThreadStreamEvent::ThreadStatusChanged { thread_id, status }
                if thread_id == expected_thread_id =>
            {
                if matches!(
                    status,
                    ThreadRuntimeStatus::NotLoaded | ThreadRuntimeStatus::Idle
                ) {
                    self.live_turn_id = None;
                }
                let turn_id = self.live_turn_id.clone();
                effect.runtime_changed =
                    self.set_runtime(runtime_from_status(status.clone(), turn_id));
            }
            ThreadStreamEvent::TurnStarted { thread_id, turn }
                if thread_id == expected_thread_id =>
            {
                effect.conversation_changed = self.merge_turn(turn);
                let active_flags = match self.runtime.as_ref() {
                    Some(LiveRuntimeState::Active { active_flags, .. }) => active_flags.clone(),
                    _ => Vec::new(),
                };
                let was_same_turn = self.live_turn_id.as_deref() == Some(turn.id.as_str());
                self.live_turn_id = Some(turn.id.clone());
                effect.runtime_changed = self.set_runtime(LiveRuntimeState::Active {
                    turn_id: Some(turn.id.clone()),
                    active_flags,
                });
                if !was_same_turn {
                    effect.started_turn_id = Some(turn.id.clone());
                }
                self.retain_turn(&turn.id);
            }
            ThreadStreamEvent::TurnCompleted { thread_id, turn }
                if thread_id == expected_thread_id =>
            {
                effect.conversation_changed = self.merge_turn(turn);
                self.retain_turn(&turn.id);
                self.live_turn_id = None;
                if !matches!(self.runtime.as_ref(), Some(LiveRuntimeState::SystemError)) {
                    effect.runtime_changed = self.set_runtime(LiveRuntimeState::Idle);
                }
            }
            ThreadStreamEvent::ItemStarted {
                thread_id,
                turn_id,
                item,
            }
            | ThreadStreamEvent::ItemCompleted {
                thread_id,
                turn_id,
                item,
            } if thread_id == expected_thread_id => {
                effect.conversation_changed = self.upsert_item(turn_id, item.clone());
                self.retain_turn(turn_id);
            }
            ThreadStreamEvent::AgentMessageDelta {
                thread_id,
                turn_id,
                item_id,
                delta,
            } if thread_id == expected_thread_id => {
                effect.conversation_changed = self.append_agent_message(turn_id, item_id, delta);
                self.retain_turn(turn_id);
            }
            ThreadStreamEvent::ActivityOutputDelta {
                thread_id,
                turn_id,
                item_id,
                delta,
            } if thread_id == expected_thread_id => {
                effect.conversation_changed = self.append_activity_output(turn_id, item_id, delta);
                self.retain_turn(turn_id);
            }
            ThreadStreamEvent::ActivityUpdated {
                thread_id,
                turn_id,
                item_id,
                update,
            } if thread_id == expected_thread_id => {
                effect.conversation_changed = self.update_activity(turn_id, item_id, update);
                self.retain_turn(turn_id);
            }
            _ => {}
        }
        effect
    }

    pub(crate) fn accept_snapshot(&mut self, snapshot: Thread) -> bool {
        let Some(mut retained) = self.retained_live_turn.take() else {
            let changed = self.conversation.as_ref() != Some(&snapshot);
            self.conversation = Some(snapshot);
            return changed;
        };
        if snapshot
            .turns
            .iter()
            .find(|turn| turn.id == retained.turn.id)
            .is_some_and(|turn| turn_covers(turn, &retained.turn))
        {
            let changed = self.conversation.as_ref() != Some(&snapshot);
            self.conversation = Some(snapshot);
            return changed;
        }

        let merged = merge_retained_live_turn(snapshot, &mut retained);
        let changed = self.conversation.as_ref() != Some(&merged);
        self.conversation = Some(merged);
        self.retained_live_turn = Some(retained);
        changed
    }

    pub(crate) fn fail_stream(&mut self) -> LiveProjectionEffect {
        let mut effect = LiveProjectionEffect::default();
        if let Some(turn_id) = self.live_turn_id.clone() {
            effect.conversation_changed = self.mark_turn_failed(&turn_id);
            self.retain_turn(&turn_id);
        }
        effect.runtime_changed = self.detach();
        effect
    }

    fn set_runtime(&mut self, runtime: LiveRuntimeState) -> bool {
        if self.runtime.as_ref() == Some(&runtime) {
            return false;
        }
        self.runtime = Some(runtime);
        true
    }

    fn merge_turn(&mut self, update: &Turn) -> bool {
        let Some(thread) = self.conversation.as_mut() else {
            return false;
        };
        if let Some(turn) = thread.turns.iter_mut().find(|turn| turn.id == update.id) {
            turn.status = update.status.clone();
            for item in &update.items {
                upsert_item(&mut turn.items, item.clone());
            }
        } else {
            thread.turns.push(update.clone());
        }
        true
    }

    fn upsert_item(&mut self, turn_id: &str, item: ThreadItem) -> bool {
        let Some(thread) = self.conversation.as_mut() else {
            return false;
        };
        let turn = turn_mut_or_insert(thread, turn_id);
        upsert_item(&mut turn.items, item);
        true
    }

    fn append_agent_message(&mut self, turn_id: &str, item_id: &str, delta: &str) -> bool {
        let Some(thread) = self.conversation.as_mut() else {
            return false;
        };
        let turn = turn_mut_or_insert(thread, turn_id);
        if let Some(ThreadItem::AgentMessage { text, .. }) = turn
            .items
            .iter_mut()
            .find(|item| thread_item_id(item) == item_id)
        {
            text.push_str(delta);
        } else {
            turn.items.push(ThreadItem::AgentMessage {
                id: item_id.to_owned(),
                text: delta.to_owned(),
                phase: None,
            });
        }
        true
    }

    fn update_activity(&mut self, turn_id: &str, item_id: &str, update: &ActivityUpdate) -> bool {
        let Some(thread) = self.conversation.as_mut() else {
            return false;
        };
        let turn = turn_mut_or_insert(thread, turn_id);
        let Some(ThreadItem::Activity(activity)) = turn
            .items
            .iter_mut()
            .find(|item| thread_item_id(item) == item_id)
        else {
            return false;
        };
        match update {
            ActivityUpdate::AppendSummary(delta) => activity.summary.push_str(delta),
            ActivityUpdate::StartSummarySection => {
                if !activity.summary.is_empty() && !activity.summary.ends_with('\n') {
                    activity.summary.push('\n');
                }
            }
            ActivityUpdate::AppendDetail(delta) => {
                let detail = activity.detail.get_or_insert_default();
                if !detail.is_empty() && !detail.ends_with('\n') {
                    detail.push('\n');
                }
                detail.push_str(delta);
            }
            ActivityUpdate::ReplaceContent { summary, detail } => {
                activity.summary.clone_from(summary);
                activity.detail.clone_from(detail);
            }
            _ => return false,
        }
        true
    }

    fn append_activity_output(&mut self, turn_id: &str, item_id: &str, delta: &str) -> bool {
        let Some(thread) = self.conversation.as_mut() else {
            return false;
        };
        let turn = turn_mut_or_insert(thread, turn_id);
        let Some(ThreadItem::Activity(activity)) = turn
            .items
            .iter_mut()
            .find(|item| thread_item_id(item) == item_id)
        else {
            return false;
        };
        activity.detail.get_or_insert_default().push_str(delta);
        true
    }

    fn retain_turn(&mut self, turn_id: &str) {
        let Some(conversation) = self.conversation.as_ref() else {
            return;
        };
        let Some(index) = conversation
            .turns
            .iter()
            .position(|turn| turn.id == turn_id)
        else {
            return;
        };
        self.retained_live_turn = Some(RetainedLiveTurn {
            index,
            turn: conversation.turns[index].clone(),
        });
    }

    fn mark_turn_failed(&mut self, turn_id: &str) -> bool {
        let Some(thread) = self.conversation.as_mut() else {
            return false;
        };
        let Some(turn) = thread.turns.iter_mut().find(|turn| turn.id == turn_id) else {
            return false;
        };
        turn.status = TurnStatus::Failed;
        for item in &mut turn.items {
            if let ThreadItem::Activity(activity) = item
                && activity.status == ActivityStatus::InProgress
            {
                activity.status = ActivityStatus::Failed;
            }
        }
        true
    }
}

pub(crate) fn event_is_incremental(event: &ThreadStreamEvent) -> bool {
    matches!(
        event,
        ThreadStreamEvent::AgentMessageDelta { .. }
            | ThreadStreamEvent::ActivityOutputDelta { .. }
            | ThreadStreamEvent::ActivityUpdated { .. }
    )
}

fn runtime_from_status(
    status: ThreadRuntimeStatus,
    active_turn_id: Option<String>,
) -> LiveRuntimeState {
    match status {
        ThreadRuntimeStatus::NotLoaded => LiveRuntimeState::Detached,
        ThreadRuntimeStatus::Idle => LiveRuntimeState::Idle,
        ThreadRuntimeStatus::Active { active_flags } => LiveRuntimeState::Active {
            turn_id: active_turn_id,
            active_flags,
        },
        ThreadRuntimeStatus::SystemError => LiveRuntimeState::SystemError,
        ThreadRuntimeStatus::Unknown(kind) => LiveRuntimeState::Unknown(kind),
        _ => LiveRuntimeState::Unknown(String::new()),
    }
}

fn turn_mut_or_insert<'a>(thread: &'a mut Thread, turn_id: &str) -> &'a mut Turn {
    if let Some(index) = thread.turns.iter().position(|turn| turn.id == turn_id) {
        return &mut thread.turns[index];
    }
    thread.turns.push(Turn {
        id: turn_id.to_owned(),
        status: TurnStatus::InProgress,
        items: Vec::new(),
    });
    thread
        .turns
        .last_mut()
        .expect("the missing turn was appended above")
}

fn upsert_item(items: &mut Vec<ThreadItem>, mut item: ThreadItem) {
    if let Some(index) = items
        .iter()
        .position(|existing| thread_item_id(existing) == thread_item_id(&item))
    {
        if let (ThreadItem::Activity(existing), ThreadItem::Activity(update)) =
            (&items[index], &mut item)
        {
            if update.started_at_unix_milliseconds.is_none() {
                update.started_at_unix_milliseconds = existing.started_at_unix_milliseconds;
            }
            if update.completed_at_unix_milliseconds.is_none() {
                update.completed_at_unix_milliseconds = existing.completed_at_unix_milliseconds;
            }
        }
        items[index] = item;
    } else {
        items.push(item);
    }
}

fn thread_item_id(item: &ThreadItem) -> &str {
    match item {
        ThreadItem::UserMessage { id, .. }
        | ThreadItem::AgentMessage { id, .. }
        | ThreadItem::Other { id, .. } => id,
        ThreadItem::Activity(activity) => &activity.id,
        _ => "",
    }
}

fn turn_covers(snapshot: &Turn, retained: &Turn) -> bool {
    snapshot.id == retained.id
        && snapshot.status == retained.status
        && retained.items.iter().all(|retained_item| {
            snapshot.items.iter().any(|snapshot_item| {
                thread_item_id(snapshot_item) == thread_item_id(retained_item)
                    && snapshot_item == retained_item
            })
        })
}

fn merge_retained_live_turn(mut snapshot: Thread, retained: &mut RetainedLiveTurn) -> Thread {
    if let Some(index) = snapshot
        .turns
        .iter()
        .position(|turn| turn.id == retained.turn.id)
    {
        let merged = merge_snapshot_turn(snapshot.turns[index].clone(), &retained.turn);
        snapshot.turns[index] = merged.clone();
        retained.index = index;
        retained.turn = merged;
    } else {
        snapshot.turns.insert(
            retained.index.min(snapshot.turns.len()),
            retained.turn.clone(),
        );
    }
    snapshot
}

fn merge_snapshot_turn(mut snapshot: Turn, retained: &Turn) -> Turn {
    let snapshot_is_terminal = turn_status_is_terminal(&snapshot.status);
    if !snapshot_is_terminal {
        snapshot.status = retained.status.clone();
    }

    for (retained_index, retained_item) in retained.items.iter().enumerate() {
        if let Some(snapshot_index) = snapshot
            .items
            .iter()
            .position(|item| thread_item_id(item) == thread_item_id(retained_item))
        {
            snapshot.items[snapshot_index] = merge_snapshot_item(
                snapshot.items[snapshot_index].clone(),
                retained_item,
                snapshot_is_terminal,
            );
        } else {
            snapshot.items.insert(
                retained_index.min(snapshot.items.len()),
                retained_item.clone(),
            );
        }
    }
    snapshot
}

fn merge_snapshot_item(
    snapshot: ThreadItem,
    retained: &ThreadItem,
    snapshot_turn_is_terminal: bool,
) -> ThreadItem {
    match (snapshot, retained) {
        (ThreadItem::Activity(snapshot), ThreadItem::Activity(retained)) => {
            ThreadItem::Activity(merge_snapshot_activity(snapshot, retained))
        }
        (
            snapshot @ ThreadItem::AgentMessage { .. },
            retained @ ThreadItem::AgentMessage { .. },
        ) => {
            let snapshot_covers_retained = match (&snapshot, retained) {
                (
                    ThreadItem::AgentMessage {
                        text: snapshot_text,
                        ..
                    },
                    ThreadItem::AgentMessage {
                        text: retained_text,
                        ..
                    },
                ) => {
                    snapshot_text.len() >= retained_text.len()
                        && snapshot_text.starts_with(retained_text)
                }
                _ => false,
            };
            if snapshot_turn_is_terminal || snapshot_covers_retained {
                snapshot
            } else {
                retained.clone()
            }
        }
        (_, retained) => retained.clone(),
    }
}

fn merge_snapshot_activity(
    mut snapshot: ward_codex::Activity,
    retained: &ward_codex::Activity,
) -> ward_codex::Activity {
    if !activity_status_is_terminal(&snapshot.status)
        && activity_status_is_terminal(&retained.status)
    {
        snapshot = retained.clone();
    } else {
        if snapshot.started_at_unix_milliseconds.is_none() {
            snapshot.started_at_unix_milliseconds = retained.started_at_unix_milliseconds;
        }
        if snapshot.completed_at_unix_milliseconds.is_none() {
            snapshot.completed_at_unix_milliseconds = retained.completed_at_unix_milliseconds;
        }
    }
    snapshot
}

fn turn_status_is_terminal(status: &TurnStatus) -> bool {
    matches!(
        status,
        TurnStatus::Completed | TurnStatus::Interrupted | TurnStatus::Failed
    )
}

fn activity_status_is_terminal(status: &ActivityStatus) -> bool {
    matches!(
        status,
        ActivityStatus::Completed | ActivityStatus::Failed | ActivityStatus::Declined
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ward_codex::{Activity, ActivityKind, AgentMessagePhase, ThreadSummary, UserInput};

    use super::*;

    fn thread() -> Thread {
        Thread {
            summary: ThreadSummary {
                id: "thread-1".to_owned(),
                name: Some("Example".to_owned()),
                preview: "Preview".to_owned(),
                cwd: PathBuf::from("/workspace"),
                created_at_unix_seconds: 10,
                updated_at_unix_seconds: 20,
            },
            turns: vec![Turn {
                id: "turn-1".to_owned(),
                status: TurnStatus::Completed,
                items: vec![ThreadItem::AgentMessage {
                    id: "agent-1".to_owned(),
                    text: "Done".to_owned(),
                    phase: Some(AgentMessagePhase::FinalAnswer),
                }],
            }],
        }
    }

    fn attached_projection() -> LiveThreadProjection {
        let mut projection = LiveThreadProjection::default();
        projection.attach(ThreadSubscription {
            thread: thread(),
            runtime_status: ThreadRuntimeStatus::Idle,
        });
        projection
    }

    #[test]
    fn preserves_reasoning_lifecycle_timing_when_the_final_item_replaces_the_start() {
        let mut items = vec![ThreadItem::Activity(Activity {
            id: "reasoning-1".to_owned(),
            kind: ActivityKind::Reasoning,
            status: ActivityStatus::InProgress,
            started_at_unix_milliseconds: Some(1_723_456_789_000),
            completed_at_unix_milliseconds: None,
            summary: "Planning".to_owned(),
            detail: None,
            context: None,
            command_actions: vec![],
        })];

        upsert_item(
            &mut items,
            ThreadItem::Activity(Activity {
                id: "reasoning-1".to_owned(),
                kind: ActivityKind::Reasoning,
                status: ActivityStatus::Completed,
                started_at_unix_milliseconds: None,
                completed_at_unix_milliseconds: Some(1_723_456_793_250),
                summary: "Planning UI state".to_owned(),
                detail: None,
                context: None,
                command_actions: vec![],
            }),
        );

        let ThreadItem::Activity(activity) = &items[0] else {
            panic!("the item should remain an activity");
        };
        assert_eq!(
            activity.started_at_unix_milliseconds,
            Some(1_723_456_789_000)
        );
        assert_eq!(
            activity.completed_at_unix_milliseconds,
            Some(1_723_456_793_250)
        );
        assert_eq!(activity.summary, "Planning UI state");
    }

    #[test]
    fn distinguishes_starting_from_a_confirmed_active_turn() {
        let mut projection = attached_projection();

        assert!(projection.begin_turn());
        assert_eq!(projection.runtime(), LiveRuntimeState::Starting);

        let effect = projection.apply_event(
            &ThreadStreamEvent::TurnStarted {
                thread_id: "thread-1".to_owned(),
                turn: Turn {
                    id: "turn-2".to_owned(),
                    status: TurnStatus::InProgress,
                    items: vec![],
                },
            },
            "thread-1",
        );

        assert_eq!(effect.started_turn_id.as_deref(), Some("turn-2"));
        assert_eq!(
            projection.runtime(),
            LiveRuntimeState::Active {
                turn_id: Some("turn-2".to_owned()),
                active_flags: vec![],
            }
        );

        let status_effect = projection.apply_event(
            &ThreadStreamEvent::ThreadStatusChanged {
                thread_id: "thread-1".to_owned(),
                status: ThreadRuntimeStatus::Active {
                    active_flags: vec![ThreadActiveFlag::WaitingOnApproval],
                },
            },
            "thread-1",
        );
        assert!(status_effect.runtime_changed);
        assert_eq!(
            projection.runtime(),
            LiveRuntimeState::Active {
                turn_id: Some("turn-2".to_owned()),
                active_flags: vec![ThreadActiveFlag::WaitingOnApproval],
            }
        );
    }

    #[test]
    fn folds_incremental_messages_and_activity_content() {
        let mut projection = attached_projection();
        let turn_id = "turn-2".to_owned();
        projection.apply_event(
            &ThreadStreamEvent::TurnStarted {
                thread_id: "thread-1".to_owned(),
                turn: Turn {
                    id: turn_id.clone(),
                    status: TurnStatus::InProgress,
                    items: vec![],
                },
            },
            "thread-1",
        );
        projection.apply_event(
            &ThreadStreamEvent::ItemStarted {
                thread_id: "thread-1".to_owned(),
                turn_id: turn_id.clone(),
                item: ThreadItem::Activity(Activity {
                    id: "reasoning-1".to_owned(),
                    kind: ActivityKind::Reasoning,
                    status: ActivityStatus::InProgress,
                    started_at_unix_milliseconds: None,
                    completed_at_unix_milliseconds: None,
                    summary: String::new(),
                    detail: None,
                    context: None,
                    command_actions: vec![],
                }),
            },
            "thread-1",
        );
        projection.apply_event(
            &ThreadStreamEvent::ActivityUpdated {
                thread_id: "thread-1".to_owned(),
                turn_id: turn_id.clone(),
                item_id: "reasoning-1".to_owned(),
                update: ActivityUpdate::AppendSummary("Inspect files".to_owned()),
            },
            "thread-1",
        );
        projection.apply_event(
            &ThreadStreamEvent::ActivityUpdated {
                thread_id: "thread-1".to_owned(),
                turn_id: turn_id.clone(),
                item_id: "reasoning-1".to_owned(),
                update: ActivityUpdate::StartSummarySection,
            },
            "thread-1",
        );
        projection.apply_event(
            &ThreadStreamEvent::ActivityUpdated {
                thread_id: "thread-1".to_owned(),
                turn_id: turn_id.clone(),
                item_id: "reasoning-1".to_owned(),
                update: ActivityUpdate::AppendSummary("Update UI".to_owned()),
            },
            "thread-1",
        );
        projection.apply_event(
            &ThreadStreamEvent::ItemStarted {
                thread_id: "thread-1".to_owned(),
                turn_id: turn_id.clone(),
                item: ThreadItem::Activity(Activity {
                    id: "tool-1".to_owned(),
                    kind: ActivityKind::ToolCall,
                    status: ActivityStatus::InProgress,
                    started_at_unix_milliseconds: None,
                    completed_at_unix_milliseconds: None,
                    summary: "server / search".to_owned(),
                    detail: Some("{}".to_owned()),
                    context: None,
                    command_actions: vec![],
                }),
            },
            "thread-1",
        );
        projection.apply_event(
            &ThreadStreamEvent::ActivityUpdated {
                thread_id: "thread-1".to_owned(),
                turn_id: turn_id.clone(),
                item_id: "tool-1".to_owned(),
                update: ActivityUpdate::AppendDetail("Fetching results\n".to_owned()),
            },
            "thread-1",
        );
        projection.apply_event(
            &ThreadStreamEvent::AgentMessageDelta {
                thread_id: "thread-1".to_owned(),
                turn_id,
                item_id: "agent-2".to_owned(),
                delta: "Working".to_owned(),
            },
            "thread-1",
        );

        let live_turn = projection.conversation().unwrap().turns.last().unwrap();
        let ThreadItem::Activity(reasoning) = &live_turn.items[0] else {
            panic!("the first item should be live reasoning");
        };
        assert_eq!(reasoning.summary, "Inspect files\nUpdate UI");
        let ThreadItem::Activity(tool) = &live_turn.items[1] else {
            panic!("the second item should be the live tool call");
        };
        assert_eq!(tool.detail.as_deref(), Some("{}\nFetching results\n"));
        assert!(matches!(
            &live_turn.items[2],
            ThreadItem::AgentMessage { text, .. } if text == "Working"
        ));
    }

    #[test]
    fn retains_the_initial_active_subscription_until_persistence_catches_up() {
        let mut subscribed = thread();
        subscribed.turns.push(Turn {
            id: "turn-2".to_owned(),
            status: TurnStatus::InProgress,
            items: vec![ThreadItem::AgentMessage {
                id: "agent-2".to_owned(),
                text: "Live text".to_owned(),
                phase: Some(AgentMessagePhase::Commentary),
            }],
        });
        let mut projection = LiveThreadProjection::default();
        projection.attach(ThreadSubscription {
            thread: subscribed,
            runtime_status: ThreadRuntimeStatus::Active {
                active_flags: vec![],
            },
        });

        let mut stale = thread();
        stale.turns.push(Turn {
            id: "turn-2".to_owned(),
            status: TurnStatus::InProgress,
            items: vec![],
        });
        assert!(!projection.accept_snapshot(stale));

        assert_eq!(
            projection.runtime(),
            LiveRuntimeState::Active {
                turn_id: Some("turn-2".to_owned()),
                active_flags: vec![],
            }
        );
        assert_eq!(projection.conversation().unwrap().turns[1].items.len(), 1);
    }

    #[test]
    fn retains_a_partial_live_turn_without_hiding_new_persisted_turns() {
        let mut projection = attached_projection();
        projection.apply_event(
            &ThreadStreamEvent::TurnStarted {
                thread_id: "thread-1".to_owned(),
                turn: Turn {
                    id: "turn-2".to_owned(),
                    status: TurnStatus::InProgress,
                    items: vec![ThreadItem::UserMessage {
                        id: "user-2".to_owned(),
                        content: vec![UserInput::Text("Continue".to_owned())],
                    }],
                },
            },
            "thread-1",
        );

        let mut persisted = thread();
        persisted.turns.push(Turn {
            id: "turn-2".to_owned(),
            status: TurnStatus::InProgress,
            items: vec![],
        });
        persisted.turns.push(Turn {
            id: "turn-3".to_owned(),
            status: TurnStatus::Completed,
            items: vec![ThreadItem::AgentMessage {
                id: "final-3".to_owned(),
                text: "External answer".to_owned(),
                phase: Some(AgentMessagePhase::FinalAnswer),
            }],
        });

        assert!(projection.accept_snapshot(persisted));
        let merged = projection.conversation().unwrap();
        assert_eq!(
            merged
                .turns
                .iter()
                .map(|turn| turn.id.as_str())
                .collect::<Vec<_>>(),
            ["turn-1", "turn-2", "turn-3"]
        );
        assert_eq!(merged.turns[1].items.len(), 1);
        assert_eq!(merged.turns[2].items.len(), 1);
    }

    #[test]
    fn accepts_an_authoritative_completed_snapshot_after_missing_live_completion() {
        let mut projection = attached_projection();
        projection.apply_event(
            &ThreadStreamEvent::TurnStarted {
                thread_id: "thread-1".to_owned(),
                turn: Turn {
                    id: "turn-2".to_owned(),
                    status: TurnStatus::InProgress,
                    items: vec![ThreadItem::AgentMessage {
                        id: "commentary-2".to_owned(),
                        text: "Working".to_owned(),
                        phase: Some(AgentMessagePhase::Commentary),
                    }],
                },
            },
            "thread-1",
        );
        projection.apply_event(
            &ThreadStreamEvent::ItemStarted {
                thread_id: "thread-1".to_owned(),
                turn_id: "turn-2".to_owned(),
                item: ThreadItem::Activity(Activity {
                    id: "command-2".to_owned(),
                    kind: ActivityKind::CommandExecution,
                    status: ActivityStatus::InProgress,
                    started_at_unix_milliseconds: Some(1_000),
                    completed_at_unix_milliseconds: None,
                    summary: "cargo test".to_owned(),
                    detail: None,
                    context: Some("/workspace".to_owned()),
                    command_actions: vec![],
                }),
            },
            "thread-1",
        );

        let mut persisted = thread();
        persisted.turns.push(Turn {
            id: "turn-2".to_owned(),
            status: TurnStatus::Completed,
            items: vec![
                ThreadItem::Activity(Activity {
                    id: "command-2".to_owned(),
                    kind: ActivityKind::CommandExecution,
                    status: ActivityStatus::Completed,
                    started_at_unix_milliseconds: None,
                    completed_at_unix_milliseconds: Some(2_000),
                    summary: "cargo test".to_owned(),
                    detail: Some("passed".to_owned()),
                    context: Some("/workspace".to_owned()),
                    command_actions: vec![],
                }),
                ThreadItem::AgentMessage {
                    id: "final-2".to_owned(),
                    text: "Done".to_owned(),
                    phase: Some(AgentMessagePhase::FinalAnswer),
                },
            ],
        });

        assert!(projection.accept_snapshot(persisted));
        let completed = projection.conversation().unwrap().turns.last().unwrap();
        assert_eq!(completed.status, TurnStatus::Completed);
        assert!(completed.items.iter().any(|item| {
            matches!(
                item,
                ThreadItem::AgentMessage { id, text, .. }
                    if id == "commentary-2" && text == "Working"
            )
        }));
        assert!(completed.items.iter().any(|item| {
            matches!(
                item,
                ThreadItem::Activity(Activity {
                    id,
                    status: ActivityStatus::Completed,
                    started_at_unix_milliseconds: Some(1_000),
                    completed_at_unix_milliseconds: Some(2_000),
                    detail: Some(detail),
                    ..
                }) if id == "command-2" && detail == "passed"
            )
        }));
        assert!(completed.items.iter().any(|item| {
            matches!(
                item,
                ThreadItem::AgentMessage { id, text, .. }
                    if id == "final-2" && text == "Done"
            )
        }));
    }

    #[test]
    fn marks_only_the_active_turn_failed_when_the_stream_disconnects() {
        let mut projection = attached_projection();
        projection.apply_event(
            &ThreadStreamEvent::TurnStarted {
                thread_id: "thread-1".to_owned(),
                turn: Turn {
                    id: "turn-2".to_owned(),
                    status: TurnStatus::InProgress,
                    items: vec![ThreadItem::Activity(Activity {
                        id: "command-2".to_owned(),
                        kind: ActivityKind::CommandExecution,
                        status: ActivityStatus::InProgress,
                        started_at_unix_milliseconds: None,
                        completed_at_unix_milliseconds: None,
                        summary: "cargo test".to_owned(),
                        detail: None,
                        context: Some("/workspace".to_owned()),
                        command_actions: vec![],
                    })],
                },
            },
            "thread-1",
        );

        let effect = projection.fail_stream();

        assert!(effect.conversation_changed);
        assert!(effect.runtime_changed);
        assert_eq!(projection.runtime(), LiveRuntimeState::Detached);
        let turns = &projection.conversation().unwrap().turns;
        assert_eq!(turns[0].status, TurnStatus::Completed);
        assert_eq!(turns[1].status, TurnStatus::Failed);
        let ThreadItem::Activity(activity) = &turns[1].items[0] else {
            panic!("the failed turn should contain an activity");
        };
        assert_eq!(activity.status, ActivityStatus::Failed);
    }
}
