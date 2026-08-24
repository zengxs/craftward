// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use serde_json::Value;

use super::{FakeCodexAppServerOptions, FakeThreadListRequest};

pub(super) struct FakeState {
    pub(super) options: FakeCodexAppServerOptions,
    pub(super) threads: Vec<FakeThread>,
    pub(super) next_fork_number: usize,
    pub(super) next_connection_id: u64,
    pub(super) thread_list_requests: Vec<FakeThreadListRequest>,
    pub(super) thread_read_request_count: usize,
}

impl FakeState {
    pub(super) fn new(options: FakeCodexAppServerOptions) -> Self {
        Self {
            options,
            threads: vec![],
            next_fork_number: 1,
            next_connection_id: 1,
            thread_list_requests: vec![],
            thread_read_request_count: 0,
        }
    }
}

#[derive(Clone)]
pub(super) struct FakeThread {
    pub(super) id: String,
    pub(super) cwd: String,
    pub(super) model: String,
    pub(super) reasoning_effort: String,
    pub(super) ephemeral: bool,
    pub(super) archived: bool,
    pub(super) name: Option<String>,
    pub(super) turns: Vec<FakeTurn>,
    pub(super) remaining_read_failures: usize,
    pub(super) writer_connection_id: Option<u64>,
}

#[derive(Clone)]
pub(super) struct FakeTurn {
    pub(super) number: usize,
    pub(super) prompt: String,
    pub(super) input: Vec<Value>,
    pub(super) guidance: Vec<String>,
    pub(super) answer: String,
    pub(super) completed: bool,
}
