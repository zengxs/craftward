// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::PathBuf;

/// Information returned by the app-server initialization handshake.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServerInfo {
    pub codex_home: PathBuf,
    pub platform_family: String,
    pub platform_os: String,
    pub user_agent: String,
}

/// A page of Codex thread summaries.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreadPage {
    pub threads: Vec<ThreadSummary>,
    pub next_cursor: Option<String>,
}

/// Metadata sufficient to render a thread in a history list.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ThreadSummary {
    pub id: String,
    pub name: Option<String>,
    pub preview: String,
    pub cwd: PathBuf,
    pub created_at_unix_seconds: i64,
    pub updated_at_unix_seconds: i64,
}

/// A Codex thread with its persisted turns loaded.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Thread {
    pub summary: ThreadSummary,
    pub turns: Vec<Turn>,
}

/// One persisted turn in a Codex thread.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Turn {
    pub id: String,
    pub status: TurnStatus,
    pub items: Vec<ThreadItem>,
}

/// The lifecycle status recorded for a turn.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TurnStatus {
    Completed,
    Interrupted,
    Failed,
    InProgress,
    Unknown(String),
}

/// A persisted item relevant to rendering a conversation.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ThreadItem {
    UserMessage {
        id: String,
        content: Vec<UserInput>,
    },
    AgentMessage {
        id: String,
        text: String,
        phase: Option<AgentMessagePhase>,
    },
    Other {
        id: String,
        kind: String,
    },
}

/// Content attached to a persisted user message.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum UserInput {
    Text(String),
    Image { url: String },
    LocalImage { path: PathBuf },
    Audio { url: String },
    LocalAudio { path: PathBuf },
    Skill { name: String, path: PathBuf },
    Mention { name: String, path: PathBuf },
    Other { kind: String },
}

/// The presentation phase of an assistant message, when supplied by Codex.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AgentMessagePhase {
    Commentary,
    FinalAnswer,
    Unknown(String),
}
