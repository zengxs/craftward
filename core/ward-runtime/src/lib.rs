// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tokio runtime ownership shared by Ward process entry points.

use std::future::Future;
use std::io;
use std::time::Duration;

use tokio::runtime::{Handle, Runtime};

const MAX_WORKER_THREADS: usize = 4;
const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// The sole Tokio runtime owner for one Ward process mode.
///
/// CLI entry points drive this owner with [`Self::block_on`]. The graphical
/// application keeps it alive while asynchronous handles are in use.
pub struct WardRuntime {
    runtime: Option<Runtime>,
}

impl WardRuntime {
    /// Creates a multi-threaded runtime capped at four worker threads.
    pub fn new() -> io::Result<Self> {
        let worker_threads = available_worker_threads();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(worker_threads)
            .thread_name("ward-runtime")
            .enable_all()
            .build()?;
        Ok(Self {
            runtime: Some(runtime),
        })
    }

    /// Returns a handle that may spawn work without owning the runtime.
    #[must_use]
    pub fn handle(&self) -> Handle {
        self.runtime().handle().clone()
    }

    /// Drives one root future to completion on this runtime.
    pub fn block_on<F: Future>(&self, future: F) -> F::Output {
        self.runtime().block_on(future)
    }

    fn runtime(&self) -> &Runtime {
        self.runtime
            .as_ref()
            .expect("the Ward runtime remains available until it is dropped")
    }
}

impl Drop for WardRuntime {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_timeout(SHUTDOWN_TIMEOUT);
        }
    }
}

fn available_worker_threads() -> usize {
    std::thread::available_parallelism()
        .map_or(2, usize::from)
        .min(MAX_WORKER_THREADS)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_count_is_nonzero_and_capped() {
        let worker_threads = available_worker_threads();

        assert!((1..=MAX_WORKER_THREADS).contains(&worker_threads));
    }

    #[test]
    fn drives_async_work() {
        let runtime = WardRuntime::new().unwrap();

        assert_eq!(runtime.block_on(async { 42 }), 42);
    }
}
