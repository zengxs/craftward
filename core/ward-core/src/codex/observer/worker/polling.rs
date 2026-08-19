// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashSet;

use ward_codex::{CodexError, Thread, ThreadPoll};

#[derive(Debug, Eq, PartialEq)]
pub(super) enum PollSample<T> {
    Updated(T),
    Unchanged,
}

#[derive(Default)]
pub(super) struct InitialConversationReads {
    pending: HashSet<String>,
}

impl InitialConversationReads {
    pub(super) fn mark_started(&mut self, thread_id: &str) {
        self.pending.insert(thread_id.to_owned());
    }

    pub(super) fn classify(
        &mut self,
        thread_id: &str,
        result: Result<ThreadPoll, CodexError>,
    ) -> Result<PollSample<Thread>, CodexError> {
        match result {
            Ok(poll) => {
                self.pending.remove(thread_id);
                Ok(match poll {
                    ThreadPoll::Baseline(thread) | ThreadPoll::Changed(thread) => {
                        PollSample::Updated(thread)
                    }
                    _ => PollSample::Unchanged,
                })
            }
            Err(error)
                if self.pending.contains(thread_id) && error.is_thread_not_loaded(thread_id) =>
            {
                Ok(PollSample::Unchanged)
            }
            Err(error) => Err(error),
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum PollEffect<T> {
    Updated(T),
    Recovered,
    Unchanged,
    Error(String),
    RepeatedError,
    Cancelled,
}

impl<T> PollEffect<T> {
    pub(super) fn is_successful(&self) -> bool {
        matches!(self, Self::Updated(_) | Self::Recovered | Self::Unchanged)
    }
}

#[derive(Default)]
pub(super) struct PollHealth {
    last_error: Option<String>,
}

impl PollHealth {
    pub(super) fn observe<T>(
        &mut self,
        result: Result<PollSample<T>, CodexError>,
        cancelled: bool,
    ) -> PollEffect<T> {
        match result {
            Ok(PollSample::Updated(value)) => {
                self.last_error = None;
                PollEffect::Updated(value)
            }
            Ok(PollSample::Unchanged) if self.last_error.take().is_some() => PollEffect::Recovered,
            Ok(PollSample::Unchanged) => PollEffect::Unchanged,
            Err(_) if cancelled => PollEffect::Cancelled,
            Err(error) => {
                let message = error.to_string();
                if self.last_error.as_deref() == Some(message.as_str()) {
                    PollEffect::RepeatedError
                } else {
                    self.last_error = Some(message.clone());
                    PollEffect::Error(message)
                }
            }
        }
    }

    pub(super) fn reset(&mut self) {
        self.last_error = None;
    }
}
