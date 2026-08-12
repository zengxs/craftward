// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::OsStr;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::protocol::{
    Connection, INITIALIZE_METHOD, InitializeParams, InitializeResponse, THREAD_LIST_METHOD,
    THREAD_READ_METHOD, ThreadListParams, ThreadListResponse, ThreadReadParams, ThreadReadResponse,
};
use crate::{CodexError, ServerInfo, Thread, ThreadPage};

type AppServerConnection = Connection<BufReader<ChildStdout>, BufWriter<ChildStdin>>;

/// Filters and pagination controls for a thread history request.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ThreadListOptions {
    pub cursor: Option<String>,
    pub limit: Option<u32>,
    pub archived: Option<bool>,
}

/// A synchronous, read-only client for one Codex app-server child process.
///
/// Calls block until the corresponding app-server response arrives. GUI callers
/// should own the client on a worker thread rather than invoke it on the UI
/// thread.
pub struct CodexClient {
    child: Child,
    connection: Option<AppServerConnection>,
    server_info: ServerInfo,
}

impl CodexClient {
    /// Starts the specified Codex executable in app-server stdio mode and
    /// completes the initialization handshake.
    pub fn spawn(executable: impl AsRef<OsStr>) -> Result<Self, CodexError> {
        let executable = PathBuf::from(executable.as_ref());
        let mut child = Command::new(&executable)
            .args(["app-server", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|source| CodexError::Spawn {
                executable: executable.clone(),
                source,
            })?;

        let setup_result = (|| {
            let input = child
                .stdin
                .take()
                .ok_or(CodexError::MissingPipe("standard input"))?;
            let output = child
                .stdout
                .take()
                .ok_or(CodexError::MissingPipe("standard output"))?;
            let mut connection = Connection::new(BufReader::new(output), BufWriter::new(input));
            let response: InitializeResponse =
                connection.request(INITIALIZE_METHOD, &InitializeParams::craftward())?;
            connection.initialized()?;
            Ok::<_, CodexError>((connection, ServerInfo::from(response)))
        })();

        match setup_result {
            Ok((connection, server_info)) => Ok(Self {
                child,
                connection: Some(connection),
                server_info,
            }),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                Err(error)
            }
        }
    }

    #[must_use]
    pub fn server_info(&self) -> &ServerInfo {
        &self.server_info
    }

    /// Lists persisted threads without triggering rollout scan-and-repair.
    pub fn list_threads(&mut self, options: &ThreadListOptions) -> Result<ThreadPage, CodexError> {
        let params =
            ThreadListParams::new(options.cursor.as_deref(), options.limit, options.archived);
        let response: ThreadListResponse = self
            .connection
            .as_mut()
            .expect("the connection exists while CodexClient is alive")
            .request(THREAD_LIST_METHOD, &params)?;
        Ok(ThreadPage {
            threads: response
                .data
                .into_iter()
                .map(|thread| thread.into_summary())
                .collect(),
            next_cursor: response.next_cursor,
        })
    }

    /// Reads one persisted thread, including its available turns and items.
    pub fn read_thread(&mut self, thread_id: &str) -> Result<Thread, CodexError> {
        let params = ThreadReadParams {
            thread_id,
            include_turns: true,
        };
        let response: ThreadReadResponse = self
            .connection
            .as_mut()
            .expect("the connection exists while CodexClient is alive")
            .request(THREAD_READ_METHOD, &params)?;
        response
            .thread
            .into_thread()
            .map_err(|source| CodexError::InvalidResponse {
                method: THREAD_READ_METHOD,
                source,
            })
    }
}

impl Drop for CodexClient {
    fn drop(&mut self) {
        // Closing stdin lets a healthy app-server observe EOF. Kill and reap the
        // child as a bounded fallback so dropping the client never leaves it
        // running or creates a zombie process.
        self.connection.take();
        if !matches!(self.child.try_wait(), Ok(Some(_))) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
