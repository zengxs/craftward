// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

//! Codex app-server integration for Craftward.
//!
//! The crate owns app-server process management, protocol initialization, and
//! JSONL request framing. Callers receive a compact read-only model instead of
//! depending on the generated wire schema.

mod client;
mod error;
mod history;
mod model;
mod protocol;

pub use client::{CodexClient, ThreadListOptions};
pub use error::CodexError;
pub use history::{CodexHistorySession, ThreadPoll};
pub use model::{
    AgentMessagePhase, ServerInfo, Thread, ThreadItem, ThreadPage, ThreadSummary, Turn, TurnStatus,
    UserInput,
};
