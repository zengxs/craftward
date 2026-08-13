// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

mod connection;
mod wire;

pub(crate) use connection::{Connection, ServerMessage};
pub(crate) use wire::{
    InitializeParams, InitializeResponse, ThreadListParams, ThreadListResponse, ThreadReadParams,
    ThreadReadResponse, ThreadResumeParams, ThreadResumeResponse, TurnStartParams,
    TurnStartResponse, turn_stream_event,
};

pub(crate) const INITIALIZE_METHOD: &str = "initialize";
pub(crate) const THREAD_LIST_METHOD: &str = "thread/list";
pub(crate) const THREAD_READ_METHOD: &str = "thread/read";
pub(crate) const THREAD_RESUME_METHOD: &str = "thread/resume";
pub(crate) const TURN_START_METHOD: &str = "turn/start";
