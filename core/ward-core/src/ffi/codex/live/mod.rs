// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use ward_codex::{
    ActivityStatus, ActivityUpdate, Thread, ThreadActiveFlag, ThreadItem, ThreadRuntimeStatus,
    ThreadStreamEvent, ThreadSubscription, Turn, TurnStatus, TurnTiming,
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
    forkable_turn_ids: Vec<String>,
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
        self.forkable_turn_ids = terminal_turn_ids(&subscription.thread);
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

    pub(crate) fn forkable_turn_ids(&self) -> &[String] {
        &self.forkable_turn_ids
    }

    pub(crate) fn needs_persisted_reconciliation(&self) -> bool {
        self.conversation
            .as_ref()
            .and_then(|thread| thread.turns.last())
            .is_some_and(|turn| {
                turn.status.is_terminal() && !self.forkable_turn_ids.contains(&turn.id)
            })
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
        let forkable_turn_ids = terminal_turn_ids(&snapshot);
        let forkable_turns_changed = self.forkable_turn_ids != forkable_turn_ids;
        self.forkable_turn_ids = forkable_turn_ids;
        let Some(mut retained) = self.retained_live_turn.take() else {
            let changed = self.conversation.as_ref() != Some(&snapshot);
            self.conversation = Some(snapshot);
            return changed || forkable_turns_changed;
        };
        if find_snapshot_turn_index(&snapshot.turns, &retained)
            .and_then(|index| snapshot.turns.get(index))
            .is_some_and(|turn| turn_covers(turn, &retained.turn))
        {
            let changed = self.conversation.as_ref() != Some(&snapshot);
            self.conversation = Some(snapshot);
            return changed || forkable_turns_changed;
        }

        let merged = merge_retained_live_turn(snapshot, &mut retained);
        let changed = self.conversation.as_ref() != Some(&merged);
        self.conversation = Some(merged);
        self.retained_live_turn = Some(retained);
        changed || forkable_turns_changed
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
            turn.timing.apply_sparse_update(&update.timing);
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

fn terminal_turn_ids(thread: &Thread) -> Vec<String> {
    thread
        .turns
        .iter()
        .filter(|turn| turn.status.is_terminal())
        .map(|turn| turn.id.clone())
        .collect()
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
        timing: TurnTiming::default(),
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
    snapshot.status == retained.status
        && snapshot.timing.covers(&retained.timing)
        && retained.items.iter().all(|retained_item| {
            snapshot.items.iter().any(|snapshot_item| {
                thread_item_id(snapshot_item) == thread_item_id(retained_item)
                    && snapshot_item == retained_item
            })
        })
}

fn turns_share_stable_identity(left: &Turn, right: &Turn) -> bool {
    // Live and persisted snapshots can report different turn identifiers for
    // the first turn while retaining the same stable item identifiers.
    left.id == right.id
        || left.items.iter().any(|left_item| {
            let left_id = thread_item_id(left_item);
            !left_id.is_empty()
                && right
                    .items
                    .iter()
                    .any(|right_item| thread_item_id(right_item) == left_id)
        })
}

fn turns_have_equivalent_message_sequence(left: &Turn, right: &Turn) -> bool {
    let left_messages = left
        .items
        .iter()
        .filter(|item| {
            matches!(
                item,
                ThreadItem::UserMessage { .. } | ThreadItem::AgentMessage { .. }
            )
        })
        .collect::<Vec<_>>();
    let right_messages = right
        .items
        .iter()
        .filter(|item| {
            matches!(
                item,
                ThreadItem::UserMessage { .. } | ThreadItem::AgentMessage { .. }
            )
        })
        .collect::<Vec<_>>();
    !left_messages.is_empty()
        && left_messages.len() == right_messages.len()
        && left_messages
            .into_iter()
            .zip(right_messages)
            .all(|(left, right)| messages_represent_same_logical_item(left, right))
}

fn find_snapshot_turn_index(turns: &[Turn], retained: &RetainedLiveTurn) -> Option<usize> {
    turns
        .iter()
        .position(|turn| turns_share_stable_identity(turn, &retained.turn))
        .or_else(|| {
            // Only the first live turn is known to receive a provisional turn
            // identifier. Keep the content fallback at that same turn slot so
            // equal messages in later turns cannot be matched across turns.
            (retained.index == 0)
                .then(|| turns.first())
                .flatten()
                .filter(|turn| turns_have_equivalent_message_sequence(turn, &retained.turn))
                .map(|_| 0)
        })
}

fn merge_retained_live_turn(mut snapshot: Thread, retained: &mut RetainedLiveTurn) -> Thread {
    if let Some(index) = find_snapshot_turn_index(&snapshot.turns, retained) {
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
    let snapshot_is_terminal = snapshot.status.is_terminal();
    if !snapshot_is_terminal {
        snapshot.status = retained.status.clone();
    }
    snapshot.timing.fill_missing_from(&retained.timing);

    let mut matched_snapshot_items = vec![false; snapshot.items.len()];
    let mut snapshot_cursor = 0;
    for retained_item in &retained.items {
        let retained_id = thread_item_id(retained_item);
        let snapshot_index = snapshot
            .items
            .iter()
            .enumerate()
            .position(|(index, item)| {
                !matched_snapshot_items[index]
                    && !retained_id.is_empty()
                    && thread_item_id(item) == retained_id
            })
            .or_else(|| {
                // History reads can renumber messages that were already seen
                // on the live stream. Match them once and in turn order.
                snapshot
                    .items
                    .iter()
                    .enumerate()
                    .skip(snapshot_cursor.min(snapshot.items.len()))
                    .find(|(index, item)| {
                        !matched_snapshot_items[*index]
                            && messages_represent_same_logical_item(item, retained_item)
                    })
                    .map(|(index, _)| index)
            });
        if let Some(snapshot_index) = snapshot_index {
            matched_snapshot_items[snapshot_index] = true;
            snapshot_cursor = snapshot_cursor.max(snapshot_index + 1);
            snapshot.items[snapshot_index] = merge_snapshot_item(
                snapshot.items[snapshot_index].clone(),
                retained_item,
                snapshot_is_terminal,
            );
        } else {
            let insertion_index = snapshot_cursor.min(snapshot.items.len());
            snapshot
                .items
                .insert(insertion_index, retained_item.clone());
            matched_snapshot_items.insert(insertion_index, true);
            snapshot_cursor = insertion_index + 1;
        }
    }
    snapshot
}

fn messages_represent_same_logical_item(left: &ThreadItem, right: &ThreadItem) -> bool {
    match (left, right) {
        (
            ThreadItem::UserMessage {
                content: left_content,
                ..
            },
            ThreadItem::UserMessage {
                content: right_content,
                ..
            },
        ) => left_content == right_content,
        (
            ThreadItem::AgentMessage {
                text: left_text,
                phase: left_phase,
                ..
            },
            ThreadItem::AgentMessage {
                text: right_text,
                phase: right_phase,
                ..
            },
        ) => left_text == right_text && left_phase == right_phase,
        _ => false,
    }
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
        (snapshot @ ThreadItem::UserMessage { .. }, ThreadItem::UserMessage { .. })
            if snapshot_turn_is_terminal =>
        {
            snapshot
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

fn activity_status_is_terminal(status: &ActivityStatus) -> bool {
    matches!(
        status,
        ActivityStatus::Completed | ActivityStatus::Failed | ActivityStatus::Declined
    )
}

#[cfg(test)]
mod tests;
