// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

use tokio::sync::mpsc::Receiver;
use ward_codex::{InteractionResponse, TurnOptions};

#[cfg(test)]
mod tests;

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ObserverCommand {
    Watch(String),
    SetThreadListScope(ThreadListScope),
    Refresh,
    AcquireWrite(String),
    ReleaseWrite(String),
    RenameThread(ThreadRenameRequest),
    ForkThread(ThreadForkRequest),
    ChangeThreadLifecycle(ThreadLifecycleRequest),
    StartThread(ThreadStartRequest),
    StartTurn(TurnRequest),
    SteerTurn(TurnSteerRequest),
    InterruptTurn(String),
    ResolveInteraction(InteractionResponse),
    Stop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ThreadListScope {
    Active,
    Archived,
}

impl ThreadListScope {
    pub(super) const fn is_archived(self) -> bool {
        matches!(self, Self::Archived)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ThreadLifecycleAction {
    Archive,
    Restore,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ThreadLifecycleRequest {
    pub(super) thread_id: String,
    pub(super) action: ThreadLifecycleAction,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum ThreadControlRequest {
    Steer(TurnSteerRequest),
    Interrupt(String),
    ResolveInteraction(InteractionResponse),
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
    pub(super) options: TurnOptions,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct TurnSteerRequest {
    pub(super) thread_id: String,
    pub(super) expected_turn_id: String,
    pub(super) prompt: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ThreadRenameRequest {
    pub(super) thread_id: String,
    pub(super) name: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ThreadForkRequest {
    pub(super) thread_id: String,
    pub(super) last_turn_id: String,
}

#[derive(Debug, Eq, PartialEq)]
pub(super) struct ThreadStartRequest {
    pub(super) working_directory: PathBuf,
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(super) struct CommandUpdate {
    pub(super) watched_thread: Option<String>,
    pub(super) thread_list_scope: Option<ThreadListScope>,
    pub(super) refresh: bool,
    pub(super) write_access: Option<WriteAccessRequest>,
    pub(super) thread_rename: Option<Box<ThreadRenameRequest>>,
    pub(super) thread_fork: Option<Box<ThreadForkRequest>>,
    pub(super) thread_lifecycle: Vec<ThreadLifecycleRequest>,
    pub(super) thread_start: Option<Box<ThreadStartRequest>>,
    pub(super) turn: Option<Box<TurnRequest>>,
    pub(super) controls: Vec<ThreadControlRequest>,
}

impl CommandUpdate {
    pub(super) fn merge(&mut self, newer: Self) {
        if newer.watched_thread.is_some() {
            self.watched_thread = newer.watched_thread;
        }
        if newer.thread_list_scope.is_some() {
            self.thread_list_scope = newer.thread_list_scope;
        }
        self.refresh |= newer.refresh;
        if newer.write_access.is_some() {
            self.write_access = newer.write_access;
        }
        if newer.thread_rename.is_some() {
            self.thread_rename = newer.thread_rename;
        }
        if self.thread_fork.is_none() {
            self.thread_fork = newer.thread_fork;
        }
        self.thread_lifecycle.extend(newer.thread_lifecycle);
        if self.thread_start.is_none() {
            self.thread_start = newer.thread_start;
        }
        if self.turn.is_none() {
            self.turn = newer.turn;
        }
        self.controls.extend(newer.controls);
    }

    pub(super) fn is_empty(&self) -> bool {
        self.watched_thread.is_none()
            && self.thread_list_scope.is_none()
            && !self.refresh
            && self.write_access.is_none()
            && self.thread_rename.is_none()
            && self.thread_fork.is_none()
            && self.thread_lifecycle.is_empty()
            && self.thread_start.is_none()
            && self.turn.is_none()
            && self.controls.is_empty()
    }

    pub(super) fn is_exclusive_operation_only(&self) -> bool {
        self.watched_thread.is_none()
            && self.thread_list_scope.is_none()
            && !self.refresh
            && self.write_access.is_none()
            && self.thread_rename.is_none()
            && self.thread_lifecycle.is_empty()
            && usize::from(self.thread_fork.is_some())
                + usize::from(self.thread_start.is_some())
                + usize::from(self.turn.is_some())
                == 1
            && self.controls.is_empty()
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
        ObserverCommand::SetThreadListScope(scope) => update.thread_list_scope = Some(scope),
        ObserverCommand::Refresh => update.refresh = true,
        ObserverCommand::AcquireWrite(thread_id) => {
            update.write_access = Some(WriteAccessRequest::Acquire(thread_id));
        }
        ObserverCommand::ReleaseWrite(thread_id) => {
            update.write_access = Some(WriteAccessRequest::Release(thread_id));
        }
        ObserverCommand::RenameThread(request) => update.thread_rename = Some(Box::new(request)),
        ObserverCommand::ForkThread(request) if update.thread_fork.is_none() => {
            update.thread_fork = Some(Box::new(request));
        }
        ObserverCommand::ForkThread(_) => {}
        ObserverCommand::ChangeThreadLifecycle(request) => {
            update.thread_lifecycle.push(request);
        }
        ObserverCommand::StartThread(request) if update.thread_start.is_none() => {
            update.thread_start = Some(Box::new(request));
        }
        ObserverCommand::StartThread(_) => {}
        ObserverCommand::StartTurn(request) if update.turn.is_none() => {
            update.turn = Some(Box::new(request));
        }
        ObserverCommand::StartTurn(_) => {}
        ObserverCommand::SteerTurn(request) => {
            update.controls.push(ThreadControlRequest::Steer(request));
        }
        ObserverCommand::InterruptTurn(thread_id) => {
            update
                .controls
                .push(ThreadControlRequest::Interrupt(thread_id));
        }
        ObserverCommand::ResolveInteraction(response) => {
            update
                .controls
                .push(ThreadControlRequest::ResolveInteraction(response));
        }
        ObserverCommand::Stop => return false,
    }
    true
}
