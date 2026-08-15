// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(unix)]
#[path = "../../test-support/fake_codex_app_server.rs"]
mod fake_codex_app_server;

#[cfg(unix)]
mod unix {
    use std::path::Path;

    use super::fake_codex_app_server::{FakeCodexAppServer, ThreadStartScenario};
    use ward_codex::{
        CodexHistoryCancellation, CodexThreadWriter, ThreadRuntimeStatus, ThreadStartOptions,
    };

    #[tokio::test]
    async fn starts_a_thread_through_the_public_writer_seam() {
        let fake_app_server = FakeCodexAppServer::create(ThreadStartScenario {
            request_ephemeral: false,
            response_ephemeral: false,
        });

        let (writer, subscription) = CodexThreadWriter::start_with_cancellation(
            fake_app_server.executable(),
            CodexHistoryCancellation::new(),
            Path::new("/workspace"),
            ThreadStartOptions::default(),
        )
        .await
        .expect("the public writer seam should start a thread");

        assert_eq!(writer.thread_id(), "thread-new");
        assert_eq!(subscription.thread.summary.id, "thread-new");
        assert_eq!(subscription.thread.summary.cwd, Path::new("/workspace"));
        assert_eq!(subscription.runtime_status, ThreadRuntimeStatus::Idle);

        writer.shutdown().await;
    }
}
