// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::OsStr;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::process::{Child, Command};

use crate::CodexError;

pub(crate) type AppServerReader = Box<dyn AsyncRead + Send + Unpin>;
pub(crate) type AppServerWriter = Box<dyn AsyncWrite + Send + Unpin>;

type ShutdownFuture = Pin<Box<dyn Future<Output = ()> + Send>>;
type ShutdownCallback = Box<dyn FnOnce() -> ShutdownFuture + Send>;

/// One connected stdin/stdout-style transport for a Codex app-server.
///
/// The transport owns the cleanup callback for whatever backs the streams.
/// Production connections use a child process, while tests can use an
/// in-memory task without changing the protocol client.
pub struct CodexAppServerTransport {
    reader: AppServerReader,
    writer: AppServerWriter,
    shutdown: AppServerShutdown,
}

impl CodexAppServerTransport {
    /// Creates a transport from one readable stream, one writable stream, and
    /// an asynchronous cleanup operation.
    pub fn new<R, W, S, F>(reader: R, writer: W, shutdown: S) -> Self
    where
        R: AsyncRead + Send + Unpin + 'static,
        W: AsyncWrite + Send + Unpin + 'static,
        S: FnOnce() -> F + Send + 'static,
        F: Future<Output = ()> + Send + 'static,
    {
        Self {
            reader: Box::new(reader),
            writer: Box::new(writer),
            shutdown: AppServerShutdown(Some(Box::new(move || Box::pin(shutdown())))),
        }
    }

    pub(crate) fn into_parts(self) -> (AppServerReader, AppServerWriter, AppServerShutdown) {
        (self.reader, self.writer, self.shutdown)
    }
}

pub(crate) struct AppServerShutdown(Option<ShutdownCallback>);

impl AppServerShutdown {
    pub(crate) async fn shutdown(&mut self) {
        if let Some(shutdown) = self.0.take() {
            shutdown().await;
        }
    }
}

/// Opens a fresh transport to one logical Codex app-server source.
///
/// A connector is synchronous because it only creates the transport. Protocol
/// initialization remains asynchronous and is always performed by
/// [`crate::CodexClient`].
pub trait CodexAppServerConnector: Send + Sync {
    fn connect(&self) -> Result<CodexAppServerTransport, CodexError>;
}

/// A cloneable source of independent Codex app-server connections.
///
/// History readers and thread writers connect separately through the same
/// source. Use [`CodexAppServerSource::executable`] in production or
/// [`CodexAppServerSource::with_connector`] to provide another stdin/stdout-style
/// adapter.
#[derive(Clone)]
pub struct CodexAppServerSource {
    connector: Arc<dyn CodexAppServerConnector>,
}

impl CodexAppServerSource {
    /// Creates a source that starts the executable in `app-server --stdio`
    /// mode for every connection.
    #[must_use]
    pub fn executable(executable: impl AsRef<OsStr>) -> Self {
        Self::with_connector(ExecutableConnector {
            executable: PathBuf::from(executable.as_ref()),
        })
    }

    /// Creates a source backed by a custom transport connector.
    #[must_use]
    pub fn with_connector(connector: impl CodexAppServerConnector + 'static) -> Self {
        Self {
            connector: Arc::new(connector),
        }
    }

    pub(crate) fn connect(&self) -> Result<CodexAppServerTransport, CodexError> {
        self.connector.connect()
    }
}

impl From<PathBuf> for CodexAppServerSource {
    fn from(executable: PathBuf) -> Self {
        Self::executable(executable)
    }
}

struct ExecutableConnector {
    executable: PathBuf,
}

impl CodexAppServerConnector for ExecutableConnector {
    fn connect(&self) -> Result<CodexAppServerTransport, CodexError> {
        let mut child = Command::new(&self.executable)
            .args(["app-server", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .kill_on_drop(true)
            .spawn()
            .map_err(|source| CodexError::Spawn {
                executable: self.executable.clone(),
                source,
            })?;
        let input = child
            .stdin
            .take()
            .ok_or(CodexError::MissingPipe("standard input"))?;
        let output = child
            .stdout
            .take()
            .ok_or(CodexError::MissingPipe("standard output"))?;

        Ok(CodexAppServerTransport::new(
            output,
            input,
            move || async move {
                terminate_child(&mut child).await;
            },
        ))
    }
}

pub(crate) async fn terminate_child(child: &mut Child) {
    if matches!(child.try_wait(), Ok(Some(_))) {
        return;
    }
    let _ = child.kill().await;
}
