// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

//! Codex app-server integration for Craftward.
//!
//! The crate owns app-server process management, protocol initialization, and
//! JSONL request framing. Callers receive a compact normalized model instead of
//! depending on the generated wire schema.

mod app_server;
mod client;
mod error;
mod history;
mod model;
mod protocol;

pub(crate) use model::ThreadInferenceState;

pub use app_server::{CodexAppServerConnector, CodexAppServerSource, CodexAppServerTransport};
pub use client::{CodexClient, ThreadListOptions};
pub use error::CodexError;
pub use history::{
    CodexHistoryCancellation, CodexHistorySession, CodexThreadWriter, ThreadPagePoll, ThreadPoll,
};
pub use model::{
    Activity, ActivityKind, ActivityStatus, ActivityUpdate, AgentMessagePhase, CommandAction,
    CommandActionKind, InferenceOverride, InteractionAnswer, InteractionDecision, InteractionId,
    InteractionOption, InteractionQuestion, InteractionResponse, InteractionResponseBody,
    ModelCatalog, ModelInfo, PendingInteraction, PendingInteractionKind, ReasoningEffort,
    ReasoningEffortOption, ServerInfo, Thread, ThreadActiveFlag, ThreadItem, ThreadPage,
    ThreadRuntimeStatus, ThreadStartOptions, ThreadStreamEvent, ThreadSubscription, ThreadSummary,
    Turn, TurnInput, TurnMode, TurnOptions, TurnPermissionPreset, TurnStatus, TurnTiming,
    UserInput,
};
