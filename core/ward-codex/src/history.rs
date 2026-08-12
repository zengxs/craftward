// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::OsStr;
use std::path::PathBuf;

use crate::client::ProcessInterrupt;
use crate::{CodexClient, CodexError, Thread};

/// A cloneable handle that interrupts an in-flight history session operation.
///
/// Cancellation is permanent for the associated session. It terminates the
/// currently owned app-server child process so a blocked stream read returns.
#[derive(Clone, Default)]
pub struct CodexHistoryCancellation {
    process: ProcessInterrupt,
}

impl CodexHistoryCancellation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.process.interrupt();
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.process.is_interrupted()
    }
}

/// The result of polling one persisted Codex thread.
///
/// Baseline and changed results contain complete snapshots. Persistence is
/// eventually consistent, so one logical update may yield multiple changed
/// snapshots as metadata and thread items become visible at different times.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ThreadPoll {
    /// The first snapshot observed for this thread.
    Baseline(Thread),
    /// A complete snapshot that differs from the previous successful poll.
    Changed(Thread),
    /// The persisted snapshot has not changed.
    Unchanged,
}

/// A reusable, read-only observer for persisted Codex thread history.
///
/// The session owns one app-server child process and retains the last
/// successful snapshot. If the app-server stream is lost, an idempotent read is
/// retried once through a newly initialized child process.
pub struct CodexHistorySession {
    executable: PathBuf,
    cancellation: CodexHistoryCancellation,
    client: CodexClient,
    tracker: ThreadTracker,
}

#[derive(Default)]
struct ThreadTracker {
    previous: Option<Thread>,
}

impl ThreadTracker {
    fn record(&mut self, thread: Thread) -> ThreadPoll {
        let poll = match self.previous.as_ref() {
            Some(previous) if previous == &thread => ThreadPoll::Unchanged,
            Some(previous) if previous.summary.id == thread.summary.id => {
                ThreadPoll::Changed(thread.clone())
            }
            Some(_) | None => ThreadPoll::Baseline(thread.clone()),
        };
        self.previous = Some(thread);
        poll
    }

    fn reset(&mut self) {
        self.previous = None;
    }
}

impl CodexHistorySession {
    /// Starts an app-server child process for a history observation session.
    pub fn spawn(executable: impl AsRef<OsStr>) -> Result<Self, CodexError> {
        Self::spawn_with_cancellation(executable, CodexHistoryCancellation::new())
    }

    /// Starts a history session controlled by a pre-created cancellation
    /// handle.
    pub fn spawn_with_cancellation(
        executable: impl AsRef<OsStr>,
        cancellation: CodexHistoryCancellation,
    ) -> Result<Self, CodexError> {
        let executable = PathBuf::from(executable.as_ref());
        let client = CodexClient::spawn_interruptible(&executable, &cancellation.process)?;
        Ok(Self {
            executable,
            cancellation,
            client,
            tracker: ThreadTracker::default(),
        })
    }

    /// Reads one thread and reports whether its complete snapshot changed.
    ///
    /// Polling a different thread establishes a new baseline. Failed polls do
    /// not replace the last successful snapshot.
    pub fn poll_thread(&mut self, thread_id: &str) -> Result<ThreadPoll, CodexError> {
        let thread = self.read_thread(thread_id)?;
        Ok(self.tracker.record(thread))
    }

    /// Makes the next successful poll establish a new baseline.
    ///
    /// The app-server connection remains alive. This is useful when a caller
    /// has discarded its rendered snapshot and needs the complete thread
    /// again, even if persistence has not changed.
    pub fn reset_baseline(&mut self) {
        self.tracker.reset();
    }

    fn read_thread(&mut self, thread_id: &str) -> Result<Thread, CodexError> {
        if self.cancellation.is_cancelled() {
            return Err(CodexError::Interrupted);
        }

        match self.client.read_thread(thread_id) {
            _ if self.cancellation.is_cancelled() => Err(CodexError::Interrupted),
            Ok(thread) => Ok(thread),
            Err(error) if error.is_connection_lost() => {
                self.client =
                    CodexClient::spawn_interruptible(&self.executable, &self.cancellation.process)?;
                let result = self.client.read_thread(thread_id);
                if self.cancellation.is_cancelled() {
                    Err(CodexError::Interrupted)
                } else {
                    result
                }
            }
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::{ThreadItem, ThreadSummary, Turn, TurnStatus};

    use super::*;

    fn thread(id: &str, text: &str) -> Thread {
        Thread {
            summary: ThreadSummary {
                id: id.to_owned(),
                name: Some("Example".to_owned()),
                preview: "Hello".to_owned(),
                cwd: PathBuf::from("/workspace"),
                created_at_unix_seconds: 10,
                updated_at_unix_seconds: 20,
            },
            turns: vec![Turn {
                id: "turn-1".to_owned(),
                status: TurnStatus::InProgress,
                items: vec![ThreadItem::AgentMessage {
                    id: "agent-1".to_owned(),
                    text: text.to_owned(),
                    phase: None,
                }],
            }],
        }
    }

    #[test]
    fn classifies_baseline_unchanged_changed_and_switched_threads() {
        let first = thread("thread-1", "Working");
        let changed = thread("thread-1", "Done");
        let switched = thread("thread-2", "Other");
        let mut tracker = ThreadTracker::default();

        assert!(matches!(tracker.record(first), ThreadPoll::Baseline(_)));
        assert_eq!(
            tracker.record(thread("thread-1", "Working")),
            ThreadPoll::Unchanged
        );
        assert_eq!(
            tracker.record(changed.clone()),
            ThreadPoll::Changed(changed)
        );
        assert_eq!(
            tracker.record(switched.clone()),
            ThreadPoll::Baseline(switched)
        );
    }

    #[test]
    fn reports_metadata_and_item_visibility_as_separate_changes() {
        let mut tracker = ThreadTracker::default();
        let initial = thread("thread-1", "Working");
        let mut metadata_visible = initial.clone();
        metadata_visible.summary.updated_at_unix_seconds += 1;
        let mut item_visible = metadata_visible.clone();
        let ThreadItem::AgentMessage { text, .. } = &mut item_visible.turns[0].items[0] else {
            panic!("the fixture must contain an agent message");
        };
        *text = "Done".to_owned();

        assert!(matches!(tracker.record(initial), ThreadPoll::Baseline(_)));
        assert!(matches!(
            tracker.record(metadata_visible),
            ThreadPoll::Changed(_)
        ));
        assert!(matches!(
            tracker.record(item_visible),
            ThreadPoll::Changed(_)
        ));
    }

    #[test]
    fn reset_makes_an_unchanged_thread_a_new_baseline() {
        let first = thread("thread-1", "Working");
        let mut tracker = ThreadTracker::default();

        assert!(matches!(
            tracker.record(first.clone()),
            ThreadPoll::Baseline(_)
        ));
        assert_eq!(tracker.record(first.clone()), ThreadPoll::Unchanged);
        tracker.reset();
        assert_eq!(tracker.record(first.clone()), ThreadPoll::Baseline(first));
    }
}
