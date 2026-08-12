// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// An error while communicating with a Codex app-server.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum CodexError {
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
}

impl CodexError {
    pub(crate) fn io(operation: &'static str, source: io::Error) -> Self {
        Self::Io { operation, source }
    }

    pub(crate) fn is_connection_lost(&self) -> bool {
        matches!(self, Self::Io { .. } | Self::UnexpectedEof(_))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_stream_failures_are_treated_as_lost_connections() {
        assert!(CodexError::UnexpectedEof("thread/read").is_connection_lost());
        assert!(CodexError::io("read from", io::Error::other("closed")).is_connection_lost());
        assert!(
            !CodexError::Server {
                method: "thread/read",
                code: -1,
                message: "missing".to_owned(),
            }
            .is_connection_lost()
        );
    }
}
