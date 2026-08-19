// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::OsStr;
use std::path::Path;

use tokio_util::sync::CancellationToken;

use crate::{
    CodexAppServerSource, CodexClient, CodexError, InteractionResponse, ModelCatalog, Thread,
    ThreadListOptions, ThreadPage, ThreadStartOptions, ThreadStreamEvent, ThreadSubscription,
};

/// A cloneable handle that interrupts in-flight Codex session operations.
///
/// Cancellation is permanent for the associated session. Every in-flight
/// operation observes this token so its owner can proceed to asynchronous
/// shutdown of the app-server transport.
#[derive(Clone, Default)]
pub struct CodexHistoryCancellation {
    token: CancellationToken,
}

impl CodexHistoryCancellation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.token.cancel();
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.token.is_cancelled()
    }

    pub async fn cancelled(&self) {
        self.token.cancelled().await;
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

/// The result of polling one page of persisted Codex thread summaries.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ThreadPagePoll {
    /// The first page snapshot observed by this session.
    Baseline(ThreadPage),
    /// A complete page snapshot that differs from the previous successful poll.
    Changed(ThreadPage),
    /// The persisted page snapshot has not changed.
    Unchanged,
}

/// A reusable observer for Codex model metadata and persisted history.
///
/// The session owns one read-only app-server connection and retains the last
/// successful page and conversation snapshots. If the app-server stream is
/// lost, an idempotent request is retried once through a newly initialized
/// connection. Thread writers use separate connections.
pub struct CodexHistorySession {
    source: CodexAppServerSource,
    cancellation: CodexHistoryCancellation,
    client: CodexClient,
    thread_tracker: ThreadTracker,
    page_tracker: ThreadPageTracker,
}

/// An exclusive writer lease for one started or resumed Codex thread.
///
/// The underlying app-server connection remains alive until this value is dropped,
/// so a successful availability check and the following turns use the same
/// writer. Dropping the value closes that connection and releases the writer.
pub struct CodexThreadWriter {
    thread_id: String,
    cancellation: CodexHistoryCancellation,
    client: CodexClient,
}

enum ThreadWriterCreation<'a> {
    Start {
        working_directory: &'a Path,
        options: ThreadStartOptions,
    },
    Fork {
        thread_id: &'a str,
        last_turn_id: Option<&'a str>,
    },
}

#[derive(Default)]
struct ThreadTracker {
    previous: Option<Thread>,
}

#[derive(Default)]
struct ThreadPageTracker {
    previous: Option<ThreadPage>,
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

impl ThreadPageTracker {
    fn record(&mut self, page: ThreadPage) -> ThreadPagePoll {
        let poll = match self.previous.as_ref() {
            Some(previous) if previous == &page => ThreadPagePoll::Unchanged,
            Some(_) => ThreadPagePoll::Changed(page.clone()),
            None => ThreadPagePoll::Baseline(page.clone()),
        };
        self.previous = Some(page);
        poll
    }

    fn reset(&mut self) {
        self.previous = None;
    }
}

impl CodexHistorySession {
    /// Starts an app-server child process for a history observation session.
    pub async fn spawn(executable: impl AsRef<OsStr>) -> Result<Self, CodexError> {
        Self::spawn_with_cancellation(executable, CodexHistoryCancellation::new()).await
    }

    /// Starts a history session controlled by a pre-created cancellation
    /// handle.
    pub async fn spawn_with_cancellation(
        executable: impl AsRef<OsStr>,
        cancellation: CodexHistoryCancellation,
    ) -> Result<Self, CodexError> {
        Self::connect_with_cancellation(CodexAppServerSource::executable(executable), cancellation)
            .await
    }

    /// Opens a history session through a reusable app-server source.
    pub async fn connect(source: CodexAppServerSource) -> Result<Self, CodexError> {
        Self::connect_with_cancellation(source, CodexHistoryCancellation::new()).await
    }

    /// Opens a history session through a reusable app-server source and a
    /// pre-created cancellation handle.
    pub async fn connect_with_cancellation(
        source: CodexAppServerSource,
        cancellation: CodexHistoryCancellation,
    ) -> Result<Self, CodexError> {
        let client =
            CodexClient::connect_with_cancellation(&source, cancellation.token.clone()).await?;
        Ok(Self {
            source,
            cancellation,
            client,
            thread_tracker: ThreadTracker::default(),
            page_tracker: ThreadPageTracker::default(),
        })
    }

    /// Loads the complete visible model catalog through this read-only session.
    pub async fn load_model_catalog(&mut self) -> Result<ModelCatalog, CodexError> {
        if self.cancellation.is_cancelled() {
            return Err(CodexError::Interrupted);
        }
        let result = self.client.list_models().await;
        match result {
            Err(error) if error.is_connection_lost() && !self.cancellation.is_cancelled() => {
                self.reconnect().await?;
                self.client.list_models().await
            }
            result => result,
        }
    }

    /// Lists persisted threads and reports whether the complete page changed.
    pub async fn poll_thread_page(
        &mut self,
        options: &ThreadListOptions,
    ) -> Result<ThreadPagePoll, CodexError> {
        if self.cancellation.is_cancelled() {
            return Err(CodexError::Interrupted);
        }
        let result = self.client.list_threads(options).await;
        let page = match result {
            Err(error) if error.is_connection_lost() && !self.cancellation.is_cancelled() => {
                self.reconnect().await?;
                self.client.list_threads(options).await?
            }
            result => result?,
        };
        Ok(self.page_tracker.record(page))
    }

    /// Reads one thread and reports whether its complete snapshot changed.
    ///
    /// Polling a different thread establishes a new baseline. Failed polls do
    /// not replace the last successful snapshot.
    pub async fn poll_thread(&mut self, thread_id: &str) -> Result<ThreadPoll, CodexError> {
        if self.cancellation.is_cancelled() {
            return Err(CodexError::Interrupted);
        }
        let result = self.client.read_thread(thread_id).await;
        let thread = match result {
            Err(error) if error.is_connection_lost() && !self.cancellation.is_cancelled() => {
                self.reconnect().await?;
                self.client.read_thread(thread_id).await?
            }
            result => result?,
        };
        Ok(self.thread_tracker.record(thread))
    }

    /// Sets the user-facing name of one persisted thread.
    ///
    /// Repeating this operation with the same name is idempotent, so a lost
    /// app-server connection is retried once after reconnecting.
    pub async fn rename_thread(&mut self, thread_id: &str, name: &str) -> Result<(), CodexError> {
        if self.cancellation.is_cancelled() {
            return Err(CodexError::Interrupted);
        }
        let result = self.client.rename_thread(thread_id, name).await;
        match result {
            Err(error) if error.is_connection_lost() && !self.cancellation.is_cancelled() => {
                self.reconnect().await?;
                self.client.rename_thread(thread_id, name).await
            }
            result => result,
        }
    }

    /// Moves one persisted thread out of the active history list.
    ///
    /// This mutation is not retried after a lost response because the
    /// app-server may already have applied it.
    pub async fn archive_thread(&mut self, thread_id: &str) -> Result<(), CodexError> {
        if self.cancellation.is_cancelled() {
            return Err(CodexError::Interrupted);
        }
        self.client.archive_thread(thread_id).await
    }

    /// Restores one archived thread and returns its persisted snapshot.
    ///
    /// This mutation is not retried after a lost response because the
    /// app-server may already have applied it.
    pub async fn unarchive_thread(&mut self, thread_id: &str) -> Result<Thread, CodexError> {
        if self.cancellation.is_cancelled() {
            return Err(CodexError::Interrupted);
        }
        self.client.unarchive_thread(thread_id).await
    }

    /// Makes the next successful poll establish a new baseline.
    ///
    /// The app-server connection remains alive. This is useful when a caller
    /// has discarded its rendered snapshot and needs the complete thread
    /// again, even if persistence has not changed.
    pub fn reset_thread_baseline(&mut self) {
        self.thread_tracker.reset();
    }

    /// Makes the next successful thread-page poll establish a new baseline.
    pub fn reset_thread_page_baseline(&mut self) {
        self.page_tracker.reset();
    }

    async fn reconnect(&mut self) -> Result<(), CodexError> {
        let replacement =
            CodexClient::connect_with_cancellation(&self.source, self.cancellation.token.clone())
                .await?;
        let previous = std::mem::replace(&mut self.client, replacement);
        previous.shutdown().await;
        Ok(())
    }

    /// Closes the session's app-server connection.
    pub async fn shutdown(self) {
        self.client.shutdown().await;
    }
}

impl CodexThreadWriter {
    async fn create_on(
        source: &CodexAppServerSource,
        cancellation: CodexHistoryCancellation,
        creation: ThreadWriterCreation<'_>,
    ) -> Result<(Self, ThreadSubscription), CodexError> {
        let mut client =
            CodexClient::connect_with_cancellation(source, cancellation.token.clone()).await?;
        let result = match creation {
            ThreadWriterCreation::Start {
                working_directory,
                options,
            } => client.start_thread(working_directory, options).await,
            ThreadWriterCreation::Fork {
                thread_id,
                last_turn_id,
            } => client.fork_thread(thread_id, last_turn_id).await,
        };
        let subscription = match result {
            Ok(subscription) => subscription,
            Err(error) => {
                client.shutdown().await;
                return Err(error);
            }
        };
        if cancellation.is_cancelled() {
            client.shutdown().await;
            return Err(CodexError::Interrupted);
        }
        let thread_id = subscription.thread.summary.id.clone();
        Ok((
            Self {
                thread_id,
                cancellation,
                client,
            },
            subscription,
        ))
    }

    /// Starts a new thread in a dedicated app-server connection controlled by the
    /// supplied cancellation handle.
    ///
    /// A failed start is not retried because the app-server may have created
    /// the thread before its response became unavailable.
    pub async fn start_with_cancellation(
        executable: impl AsRef<OsStr>,
        cancellation: CodexHistoryCancellation,
        working_directory: &Path,
        options: ThreadStartOptions,
    ) -> Result<(Self, ThreadSubscription), CodexError> {
        let source = CodexAppServerSource::executable(executable);
        Self::start_on(&source, cancellation, working_directory, options).await
    }

    /// Starts a new thread through a reusable app-server source.
    pub async fn start_on(
        source: &CodexAppServerSource,
        cancellation: CodexHistoryCancellation,
        working_directory: &Path,
        options: ThreadStartOptions,
    ) -> Result<(Self, ThreadSubscription), CodexError> {
        Self::create_on(
            source,
            cancellation,
            ThreadWriterCreation::Start {
                working_directory,
                options,
            },
        )
        .await
    }

    /// Acquires the writer for a persisted thread on a dedicated app-server
    /// connection controlled by the supplied cancellation handle.
    pub async fn acquire_with_cancellation(
        executable: impl AsRef<OsStr>,
        cancellation: CodexHistoryCancellation,
        thread_id: &str,
    ) -> Result<(Self, ThreadSubscription), CodexError> {
        let source = CodexAppServerSource::executable(executable);
        Self::acquire_on(&source, cancellation, thread_id).await
    }

    /// Acquires a persisted thread writer through a reusable app-server
    /// source.
    pub async fn acquire_on(
        source: &CodexAppServerSource,
        cancellation: CodexHistoryCancellation,
        thread_id: &str,
    ) -> Result<(Self, ThreadSubscription), CodexError> {
        let mut client =
            CodexClient::connect_with_cancellation(source, cancellation.token.clone()).await?;
        let mut result = client.resume_thread(thread_id).await;
        if result
            .as_ref()
            .is_err_and(|error| error.is_connection_lost())
            && !cancellation.is_cancelled()
        {
            client.shutdown().await;
            client =
                CodexClient::connect_with_cancellation(source, cancellation.token.clone()).await?;
            result = client.resume_thread(thread_id).await;
        }
        let subscription = match result {
            Ok(subscription) => subscription,
            Err(error) => {
                client.shutdown().await;
                return Err(error);
            }
        };
        if cancellation.is_cancelled() {
            client.shutdown().await;
            return Err(CodexError::Interrupted);
        }
        Ok((
            Self {
                thread_id: thread_id.to_owned(),
                cancellation,
                client,
            },
            subscription,
        ))
    }

    /// Forks a persisted thread, optionally through an inclusive last turn,
    /// and adopts the dedicated connection as the new thread's writer.
    ///
    /// A failed fork is not retried because the app-server may have created
    /// the new thread before its response became unavailable.
    pub async fn fork_with_cancellation(
        executable: impl AsRef<OsStr>,
        cancellation: CodexHistoryCancellation,
        thread_id: &str,
        last_turn_id: Option<&str>,
    ) -> Result<(Self, ThreadSubscription), CodexError> {
        let source = CodexAppServerSource::executable(executable);
        Self::fork_on(&source, cancellation, thread_id, last_turn_id).await
    }

    /// Forks a persisted thread through a reusable app-server source,
    /// optionally omitting turns after `last_turn_id`.
    pub async fn fork_on(
        source: &CodexAppServerSource,
        cancellation: CodexHistoryCancellation,
        thread_id: &str,
        last_turn_id: Option<&str>,
    ) -> Result<(Self, ThreadSubscription), CodexError> {
        Self::create_on(
            source,
            cancellation,
            ThreadWriterCreation::Fork {
                thread_id,
                last_turn_id,
            },
        )
        .await
    }

    #[must_use]
    pub fn thread_id(&self) -> &str {
        &self.thread_id
    }

    /// Returns the active model reported for this thread writer.
    #[must_use]
    pub fn active_model(&self) -> Option<&str> {
        self.client.active_model()
    }

    /// Returns the active reasoning effort reported for this thread writer.
    #[must_use]
    pub fn active_reasoning_effort(&self) -> Option<&str> {
        self.client.active_reasoning_effort()
    }

    /// Starts a text turn without resuming again, preserving the writer lease
    /// acquired on this same connection.
    pub async fn begin_text_turn(
        &mut self,
        text: &str,
        options: crate::TurnOptions,
    ) -> Result<ThreadStreamEvent, CodexError> {
        if self.cancellation.is_cancelled() {
            return Err(CodexError::Interrupted);
        }
        let result = self
            .client
            .begin_text_turn(&self.thread_id, text, options)
            .await;
        if self.cancellation.is_cancelled() {
            Err(CodexError::Interrupted)
        } else {
            result
        }
    }

    /// Waits for the next event emitted by the subscribed thread connection.
    pub async fn next_subscription_event(&mut self) -> Result<ThreadStreamEvent, CodexError> {
        if self.cancellation.is_cancelled() {
            return Err(CodexError::Interrupted);
        }
        self.client.next_subscription_event().await
    }

    /// Resolves one pending approval or user-input request.
    pub async fn resolve_interaction(
        &mut self,
        response: InteractionResponse,
    ) -> Result<ThreadStreamEvent, CodexError> {
        if self.cancellation.is_cancelled() {
            return Err(CodexError::Interrupted);
        }
        self.client.resolve_interaction(response).await
    }

    /// Returns the current pending interactions for this subscribed thread.
    #[must_use]
    pub fn pending_interactions(&self) -> Vec<crate::PendingInteraction> {
        self.client.pending_interactions(&self.thread_id)
    }

    /// Adds text guidance to the expected active turn.
    pub async fn steer_text_turn(
        &mut self,
        expected_turn_id: &str,
        text: &str,
    ) -> Result<(), CodexError> {
        if self.cancellation.is_cancelled() {
            return Err(CodexError::Interrupted);
        }
        self.client
            .steer_text_turn(&self.thread_id, expected_turn_id, text)
            .await
    }

    /// Requests interruption of the active turn.
    pub async fn interrupt_turn(&mut self, turn_id: &str) -> Result<(), CodexError> {
        if self.cancellation.is_cancelled() {
            return Err(CodexError::Interrupted);
        }
        self.client.interrupt_turn(&self.thread_id, turn_id).await
    }

    /// Closes the writer's app-server connection.
    pub async fn shutdown(self) {
        self.client.shutdown().await;
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::time::Duration;

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

    #[test]
    fn classifies_thread_page_snapshots_and_reset_baselines() {
        let first = ThreadPage {
            threads: vec![thread("thread-1", "Working").summary],
            next_cursor: Some("next".to_owned()),
        };
        let changed = ThreadPage {
            threads: vec![thread("thread-2", "Other").summary],
            next_cursor: None,
        };
        let mut tracker = ThreadPageTracker::default();

        assert_eq!(
            tracker.record(first.clone()),
            ThreadPagePoll::Baseline(first.clone())
        );
        assert_eq!(tracker.record(first.clone()), ThreadPagePoll::Unchanged);
        assert_eq!(
            tracker.record(changed.clone()),
            ThreadPagePoll::Changed(changed)
        );
        tracker.reset();
        assert_eq!(
            tracker.record(first.clone()),
            ThreadPagePoll::Baseline(first)
        );
    }

    #[tokio::test]
    async fn cancellation_wakes_waiters() {
        let cancellation = CodexHistoryCancellation::new();
        let waiter = tokio::spawn({
            let cancellation = cancellation.clone();
            async move { cancellation.cancelled().await }
        });

        cancellation.cancel();

        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("cancellation should wake the waiter")
            .expect("the cancellation waiter should not panic");
    }

    #[tokio::test]
    async fn pre_cancelled_thread_start_does_not_spawn_an_app_server() {
        let cancellation = CodexHistoryCancellation::new();
        cancellation.cancel();

        let result = CodexThreadWriter::start_with_cancellation(
            "/an/executable/that/does/not/exist",
            cancellation,
            Path::new("/workspace"),
            ThreadStartOptions::default(),
        )
        .await;

        assert!(matches!(result, Err(CodexError::Interrupted)));
    }
}
