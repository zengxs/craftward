// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

#[cfg(unix)]
#[path = "../../test-support/fake_codex_app_server.rs"]
mod fake_codex_app_server;

#[cfg(unix)]
mod unix {
    use std::process::{Command, Output};

    use super::fake_codex_app_server::{FakeCodexAppServer, ThreadStartScenario};

    fn run_probe(scenario: ThreadStartScenario) -> Output {
        let fake_app_server = FakeCodexAppServer::create(scenario);

        Command::new(env!("CARGO_BIN_EXE_ward"))
            .args(["codex", "--codex"])
            .arg(fake_app_server.executable())
            .args(["probe-start", "--cwd", "/workspace"])
            .output()
            .expect("the Ward CLI should run")
    }

    #[test]
    fn probes_ephemeral_thread_start_through_the_cli() {
        let output = run_probe(ThreadStartScenario {
            request_ephemeral: true,
            response_ephemeral: true,
        });

        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).expect("stdout should be UTF-8"),
            concat!(
                "Started ephemeral thread thread-new\n",
                "Working directory: /workspace\n",
                "Runtime status: idle\n"
            )
        );
        assert_eq!(output.stderr, b"");
    }

    #[test]
    fn rejects_a_thread_start_that_is_not_confirmed_ephemeral() {
        let output = run_probe(ThreadStartScenario {
            request_ephemeral: true,
            response_ephemeral: false,
        });

        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .expect("stderr should be UTF-8")
                .contains("did not confirm an ephemeral thread")
        );
    }
}
