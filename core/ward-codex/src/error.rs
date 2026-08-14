// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io;
use std::path::PathBuf;

use thiserror::Error;

use crate::InteractionId;

/// An error while communicating with a Codex app-server.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CodexError {
    #[error("the Codex app-server operation was interrupted")]
    Interrupted,
    #[error("the Codex app-server timed out while responding to {0}")]
    TimedOut(&'static str),
    #[error("failed to start Codex app-server from {executable}: {source}")]
    Spawn {
        executable: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("the Codex app-server child process did not expose its {0} pipe")]
    MissingPipe(&'static str),
    #[error("failed to {operation} the Codex app-server stream: {source}")]
    Io {
        operation: &'static str,
        #[source]
        source: io::Error,
    },
    #[error("failed to encode a Codex app-server request: {0}")]
    Encode(#[source] serde_json::Error),
    #[error("the Codex app-server returned invalid JSON: {0}")]
    InvalidJson(#[source] serde_json::Error),
    #[error("the Codex app-server response to {method} is invalid: {source}")]
    InvalidResponse {
        method: &'static str,
        #[source]
        source: serde_json::Error,
    },
    #[error("the Codex app-server closed its output while responding to {0}")]
    UnexpectedEof(&'static str),
    #[error("the Codex app-server returned error {code} for {method}: {message}")]
    Server {
        method: &'static str,
        code: i64,
        message: String,
    },
    #[error("unexpected Codex app-server message while responding to {method}: {description}")]
    UnexpectedMessage {
        method: &'static str,
        description: String,
    },
    #[error("Codex interaction {0} is no longer pending")]
    UnknownInteraction(InteractionId),
    #[error("the response to Codex interaction {interaction_id} is invalid: {description}")]
    InvalidInteractionResponse {
        interaction_id: InteractionId,
        description: String,
    },
    #[error("the Codex app-server does not support the selected turn controls: {description}")]
    UnsupportedTurnControls { description: String },
}

impl CodexError {
    pub(crate) fn io(operation: &'static str, source: io::Error) -> Self {
        Self::Io { operation, source }
    }

    pub(crate) fn is_connection_lost(&self) -> bool {
        matches!(
            self,
            Self::Io { .. } | Self::UnexpectedEof(_) | Self::TimedOut(_)
        )
    }

    /// Returns whether a thread resume failed because another app-server owns
    /// the persisted thread writer.
    #[must_use]
    pub fn is_thread_writer_conflict(&self) -> bool {
        matches!(
            self,
            Self::Server {
                method: "thread/resume",
                code: -32600,
                message,
            } if message.contains("already has an active writer")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_stream_failures_are_treated_as_lost_connections() {
        assert!(CodexError::UnexpectedEof("thread/read").is_connection_lost());
        assert!(CodexError::io("read from", io::Error::other("closed")).is_connection_lost());
        assert!(CodexError::TimedOut("thread/read").is_connection_lost());
        assert!(!CodexError::Interrupted.is_connection_lost());
        assert!(
            !CodexError::Server {
                method: "thread/read",
                code: -1,
                message: "missing".to_owned(),
            }
            .is_connection_lost()
        );
    }

    #[test]
    fn recognizes_only_the_thread_resume_writer_conflict() {
        assert!(
            CodexError::Server {
                method: "thread/resume",
                code: -32600,
                message: "thread thread-1 already has an active writer".to_owned(),
            }
            .is_thread_writer_conflict()
        );
        assert!(
            !CodexError::Server {
                method: "thread/read",
                code: -32600,
                message: "thread thread-1 already has an active writer".to_owned(),
            }
            .is_thread_writer_conflict()
        );
        assert!(
            !CodexError::Server {
                method: "thread/resume",
                code: -32600,
                message: "the thread is unavailable".to_owned(),
            }
            .is_thread_writer_conflict()
        );
    }
}
