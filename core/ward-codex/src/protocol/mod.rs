// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

mod connection;
mod wire;

pub(crate) use connection::{Connection, ServerMessage};
pub(crate) use wire::{
    InitializeParams, InitializeResponse, ThreadListParams, ThreadListResponse, ThreadReadParams,
    ThreadReadResponse, ThreadResumeParams, ThreadResumeResponse, ThreadSetNameParams,
    ThreadSetNameResponse, ThreadStartParams, ThreadStartResponse, TurnInterruptParams,
    TurnInterruptResponse, TurnStartParams, TurnStartResponse, TurnSteerParams, TurnSteerResponse,
    interaction_result, pending_interaction, resolved_server_request, turn_stream_event,
};

pub(crate) const INITIALIZE_METHOD: &str = "initialize";
pub(crate) const THREAD_LIST_METHOD: &str = "thread/list";
pub(crate) const THREAD_READ_METHOD: &str = "thread/read";
pub(crate) const THREAD_RESUME_METHOD: &str = "thread/resume";
pub(crate) const THREAD_SET_NAME_METHOD: &str = "thread/name/set";
pub(crate) const THREAD_START_METHOD: &str = "thread/start";
pub(crate) const TURN_START_METHOD: &str = "turn/start";
pub(crate) const TURN_STEER_METHOD: &str = "turn/steer";
pub(crate) const TURN_INTERRUPT_METHOD: &str = "turn/interrupt";
