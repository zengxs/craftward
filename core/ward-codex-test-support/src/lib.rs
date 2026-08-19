// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

//! In-memory Codex app-server support for integration tests.
//!
//! The fake speaks the same newline-delimited JSON protocol as a real
//! app-server. It intentionally implements product-level behavior instead of
//! exposing a generic sequence-of-responses scripting language.

mod catalog;
mod server;
mod state;
mod threads;
mod turns;

use std::sync::{Arc, Mutex};

use ward_codex::CodexAppServerSource;

use self::server::FakeConnector;
use self::state::FakeState;

/// Mutually exclusive turn behaviors supported by the fake app-server.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum FakeTurnScenario {
    /// Complete the turn without waiting for client input.
    #[default]
    Complete,
    /// Keep the turn active until the client supplies guidance.
    WaitForGuidance,
    /// Request approval for a command before completing the turn.
    RequestCommandApproval,
    /// Request structured user input before completing the turn.
    RequestUserInput,
}

/// Observable behaviors supported by the fake app-server.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FakeCodexAppServerOptions {
    /// Whether the app-server confirms ephemeral thread starts as ephemeral.
    pub confirm_ephemeral_thread_starts: bool,
    /// Number of initial reads of a newly started thread that report the
    /// app-server's transient `thread not loaded` error.
    pub initial_thread_read_failures: usize,
    /// Whether persisted history assigns different identifiers to the first
    /// live turn and its messages.
    pub renumber_persisted_first_turn: bool,
    /// Whether the first fork is applied before its connection closes without
    /// returning the mutation response.
    pub lose_first_fork_response: bool,
    /// Number of initial model-list requests that return a temporary error.
    pub model_list_failures: usize,
    /// Behavior exercised by each started turn.
    pub turn_scenario: FakeTurnScenario,
}

impl Default for FakeCodexAppServerOptions {
    fn default() -> Self {
        Self {
            confirm_ephemeral_thread_starts: true,
            initial_thread_read_failures: 0,
            renumber_persisted_first_turn: false,
            lose_first_fork_response: false,
            model_list_failures: 0,
            turn_scenario: FakeTurnScenario::default(),
        }
    }
}

/// A stateful in-memory Codex app-server shared by independent connections.
pub struct FakeCodexAppServer {
    source: CodexAppServerSource,
}

impl FakeCodexAppServer {
    #[must_use]
    pub fn new(options: FakeCodexAppServerOptions) -> Self {
        let state = Arc::new(Mutex::new(FakeState::new(options)));
        Self {
            source: CodexAppServerSource::with_connector(FakeConnector::new(state)),
        }
    }

    #[must_use]
    pub fn source(&self) -> CodexAppServerSource {
        self.source.clone()
    }
}

impl Default for FakeCodexAppServer {
    fn default() -> Self {
        Self::new(FakeCodexAppServerOptions::default())
    }
}
