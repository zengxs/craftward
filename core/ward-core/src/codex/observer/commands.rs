// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use tokio::sync::mpsc::Receiver;

#[derive(Debug)]
pub(super) enum ObserverCommand {
    Watch(String),
    Refresh,
    AcquireWrite(String),
    ReleaseWrite(String),
    StartTurn(TurnRequest),
    Stop,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum WriteAccessRequest {
    Acquire(String),
    Release(String),
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct TurnRequest {
    pub(super) thread_id: String,
    pub(super) prompt: String,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct CommandUpdate {
    pub(super) watched_thread: Option<String>,
    pub(super) refresh: bool,
    pub(super) write_access: Option<WriteAccessRequest>,
    pub(super) turn: Option<TurnRequest>,
}

impl CommandUpdate {
    pub(super) fn merge(&mut self, newer: Self) {
        if newer.watched_thread.is_some() {
            self.watched_thread = newer.watched_thread;
        }
        self.refresh |= newer.refresh;
        if newer.write_access.is_some() {
            self.write_access = newer.write_access;
        }
        if self.turn.is_none() {
            self.turn = newer.turn;
        }
    }

    pub(super) fn is_empty(&self) -> bool {
        self.watched_thread.is_none()
            && !self.refresh
            && self.write_access.is_none()
            && self.turn.is_none()
    }

    pub(super) fn is_turn_only(&self) -> bool {
        self.watched_thread.is_none()
            && !self.refresh
            && self.write_access.is_none()
            && self.turn.is_some()
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum DrainedCommands {
    Update(CommandUpdate),
    Stop,
}

pub(super) fn drain_commands(
    first: ObserverCommand,
    receiver: &mut Receiver<ObserverCommand>,
) -> DrainedCommands {
    let mut update = CommandUpdate::default();
    if !merge_command(&mut update, first) {
        return DrainedCommands::Stop;
    }
    drain_available_commands(&mut update, receiver)
}

fn drain_available_commands(
    update: &mut CommandUpdate,
    receiver: &mut Receiver<ObserverCommand>,
) -> DrainedCommands {
    while let Ok(command) = receiver.try_recv() {
        if !merge_command(update, command) {
            return DrainedCommands::Stop;
        }
    }
    DrainedCommands::Update(std::mem::take(update))
}

pub(super) fn merge_command(update: &mut CommandUpdate, command: ObserverCommand) -> bool {
    match command {
        ObserverCommand::Watch(thread_id) => update.watched_thread = Some(thread_id),
        ObserverCommand::Refresh => update.refresh = true,
        ObserverCommand::AcquireWrite(thread_id) => {
            update.write_access = Some(WriteAccessRequest::Acquire(thread_id));
        }
        ObserverCommand::ReleaseWrite(thread_id) => {
            update.write_access = Some(WriteAccessRequest::Release(thread_id));
        }
        ObserverCommand::StartTurn(request) if update.turn.is_none() => {
            update.turn = Some(request);
        }
        ObserverCommand::StartTurn(_) => {}
        ObserverCommand::Stop => return false,
    }
    true
}
