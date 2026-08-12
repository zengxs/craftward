// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::OsStr;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, Weak};

use crate::protocol::{
    Connection, INITIALIZE_METHOD, InitializeParams, InitializeResponse, THREAD_LIST_METHOD,
    THREAD_READ_METHOD, ThreadListParams, ThreadListResponse, ThreadReadParams, ThreadReadResponse,
};
use crate::{CodexError, ServerInfo, Thread, ThreadPage};

type AppServerConnection = Connection<BufReader<ChildStdout>, BufWriter<ChildStdin>>;

#[derive(Clone, Default)]
pub(crate) struct ProcessInterrupt {
    inner: Arc<ProcessInterruptState>,
}

#[derive(Default)]
struct ProcessInterruptState {
    interrupted: std::sync::atomic::AtomicBool,
    child: Mutex<Option<Weak<Mutex<Child>>>>,
}

impl ProcessInterrupt {
    pub(crate) fn interrupt(&self) {
        self.inner
            .interrupted
            .store(true, std::sync::atomic::Ordering::Release);
        let child = self
            .inner
            .child
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .as_ref()
            .and_then(Weak::upgrade);
        if let Some(child) = child {
            terminate_child(&child);
        }
    }

    pub(crate) fn is_interrupted(&self) -> bool {
        self.inner
            .interrupted
            .load(std::sync::atomic::Ordering::Acquire)
    }

    fn register(&self, child: &Arc<Mutex<Child>>) -> bool {
        let mut active_child = self
            .inner
            .child
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *active_child = Some(Arc::downgrade(child));
        let interrupted = self.is_interrupted();
        drop(active_child);
        if interrupted {
            terminate_child(child);
        }
        interrupted
    }
}

fn terminate_child(child: &Arc<Mutex<Child>>) {
    let mut child = child
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if matches!(child.try_wait(), Ok(Some(_))) {
        return;
    }
    if child.kill().is_ok() {
        let _ = child.wait();
    }
}

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
    child: Arc<Mutex<Child>>,
    connection: Option<AppServerConnection>,
    server_info: ServerInfo,
}

impl CodexClient {
    /// Starts the specified Codex executable in app-server stdio mode and
    /// completes the initialization handshake.
    pub fn spawn(executable: impl AsRef<OsStr>) -> Result<Self, CodexError> {
        Self::spawn_inner(executable.as_ref(), None)
    }

    pub(crate) fn spawn_interruptible(
        executable: impl AsRef<OsStr>,
        interrupt: &ProcessInterrupt,
    ) -> Result<Self, CodexError> {
        Self::spawn_inner(executable.as_ref(), Some(interrupt))
    }

    fn spawn_inner(
        executable: &OsStr,
        interrupt: Option<&ProcessInterrupt>,
    ) -> Result<Self, CodexError> {
        if interrupt.is_some_and(ProcessInterrupt::is_interrupted) {
            return Err(CodexError::Interrupted);
        }
        let executable = PathBuf::from(executable);
        let child = Command::new(&executable)
            .args(["app-server", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|source| CodexError::Spawn {
                executable: executable.clone(),
                source,
            })?;

        let child = Arc::new(Mutex::new(child));
        if interrupt.is_some_and(|interrupt| interrupt.register(&child)) {
            return Err(CodexError::Interrupted);
        }

        let setup_result = (|| {
            let (input, output) = {
                let mut child = child
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                let input = child
                    .stdin
                    .take()
                    .ok_or(CodexError::MissingPipe("standard input"))?;
                let output = child
                    .stdout
                    .take()
                    .ok_or(CodexError::MissingPipe("standard output"))?;
                (input, output)
            };
            let mut connection = Connection::new(BufReader::new(output), BufWriter::new(input));
            let response: InitializeResponse =
                connection.request(INITIALIZE_METHOD, &InitializeParams::craftward())?;
            connection.initialized()?;
            Ok::<_, CodexError>((connection, ServerInfo::from(response)))
        })();

        match setup_result {
            Ok(_) if interrupt.is_some_and(ProcessInterrupt::is_interrupted) => {
                terminate_child(&child);
                Err(CodexError::Interrupted)
            }
            Ok((connection, server_info)) => Ok(Self {
                child,
                connection: Some(connection),
                server_info,
            }),
            Err(error) => {
                terminate_child(&child);
                if interrupt.is_some_and(ProcessInterrupt::is_interrupted) {
                    Err(CodexError::Interrupted)
                } else {
                    Err(error)
                }
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
        terminate_child(&self.child);
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead as _, BufReader, Write as _};
    use std::time::Duration;

    use super::*;

    const HELPER_ENVIRONMENT_VARIABLE: &str = "CRAFTWARD_CODEX_INTERRUPT_HELPER";
    const HELPER_READY_LINE: &str = "craftward-interrupt-helper-ready";

    #[test]
    fn interrupt_helper_process() {
        if std::env::var_os(HELPER_ENVIRONMENT_VARIABLE).is_none() {
            return;
        }
        println!("{HELPER_READY_LINE}");
        std::io::stdout().flush().unwrap();
        std::thread::sleep(Duration::from_secs(60));
    }

    fn running_helper() -> Arc<Mutex<Child>> {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "client::tests::interrupt_helper_process",
                "--nocapture",
            ])
            .env(HELPER_ENVIRONMENT_VARIABLE, "1")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        loop {
            line.clear();
            assert_ne!(output.read_line(&mut line).unwrap(), 0);
            if line.trim() == HELPER_READY_LINE {
                break;
            }
        }
        Arc::new(Mutex::new(child))
    }

    #[test]
    fn interrupt_terminates_the_registered_child() {
        let child = running_helper();
        let interrupt = ProcessInterrupt::default();
        assert!(!interrupt.register(&child));

        interrupt.interrupt();

        assert!(matches!(
            child
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .try_wait(),
            Ok(Some(_))
        ));
    }

    #[test]
    fn a_preexisting_interrupt_terminates_a_newly_registered_child() {
        let interrupt = ProcessInterrupt::default();
        interrupt.interrupt();
        let child = running_helper();

        assert!(interrupt.register(&child));
        assert!(matches!(
            child
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .try_wait(),
            Ok(Some(_))
        ));
    }
}
