// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_FAKE_ID: AtomicU64 = AtomicU64::new(0);

const FAKE_APP_SERVER: &str = r#"#!/bin/sh
set -eu

fail() {
    printf '%s\n' "$1" >&2
    exit 1
}

[ "${1-}" = "app-server" ] || fail "expected the app-server subcommand"
[ "${2-}" = "--stdio" ] || fail "expected stdio transport"

IFS= read -r initialize_request
case "$initialize_request" in
    *'"method":"initialize"'*) ;;
    *) fail "expected an initialize request" ;;
esac
printf '%s\n' '{"id":1,"result":{"codexHome":"/codex-home","platformFamily":"unix","platformOs":"test","userAgent":"fake-codex"}}'

IFS= read -r initialized_notification
[ "$initialized_notification" = '{"method":"initialized"}' ] || fail "expected an initialized notification"

IFS= read -r thread_start_request
case "$thread_start_request" in
    *'"method":"thread/start"'*) ;;
    *) fail "expected a thread/start request" ;;
esac
case "$thread_start_request" in
    *'"cwd":"/workspace"'*) ;;
    *) fail "expected /workspace as the working directory" ;;
esac
@REQUEST_PERSISTENCE_CHECK@
printf '%s\n' '{"id":2,"result":{"model":"gpt-5.6-sol","thread":{"id":"thread-new","name":null,"preview":"","cwd":"/workspace","createdAt":10,"updatedAt":10,"ephemeral":@RESPONSE_EPHEMERAL@,"status":{"type":"idle"},"turns":[]}}}'

while IFS= read -r _request; do
    :
done
"#;

const EXPECT_EPHEMERAL_REQUEST: &str = r#"case "$thread_start_request" in
    *'"ephemeral":true'*) ;;
    *) fail "expected an ephemeral thread" ;;
esac"#;

const EXPECT_PERSISTED_REQUEST: &str = r#"case "$thread_start_request" in
    *'"ephemeral":true'*) fail "expected a persisted thread" ;;
    *) ;;
esac"#;

#[derive(Clone, Copy)]
pub(crate) struct ThreadStartScenario {
    pub(crate) request_ephemeral: bool,
    pub(crate) response_ephemeral: bool,
}

impl ThreadStartScenario {
    fn request_persistence_check(self) -> &'static str {
        if self.request_ephemeral {
            EXPECT_EPHEMERAL_REQUEST
        } else {
            EXPECT_PERSISTED_REQUEST
        }
    }

    fn response_ephemeral(self) -> &'static str {
        if self.response_ephemeral {
            "true"
        } else {
            "false"
        }
    }
}

pub(crate) struct FakeCodexAppServer {
    directory: PathBuf,
    executable: PathBuf,
}

impl FakeCodexAppServer {
    pub(crate) fn create(scenario: ThreadStartScenario) -> Self {
        let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .expect("the crate should be inside the repository");
        let fake_id = NEXT_FAKE_ID.fetch_add(1, Ordering::Relaxed);
        let directory = repository_root.join(".tmp").join(format!(
            "fake-codex-app-server-{}-{fake_id}",
            std::process::id()
        ));
        fs::create_dir_all(&directory).expect("the fake app-server directory should exist");

        let executable = directory.join("codex");
        let script = FAKE_APP_SERVER
            .replace(
                "@REQUEST_PERSISTENCE_CHECK@",
                scenario.request_persistence_check(),
            )
            .replace("@RESPONSE_EPHEMERAL@", scenario.response_ephemeral());
        fs::write(&executable, script).expect("the fake app-server should be written");
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o700))
            .expect("the fake app-server should be executable");

        Self {
            directory,
            executable,
        }
    }

    pub(crate) fn executable(&self) -> &Path {
        &self.executable
    }
}

impl Drop for FakeCodexAppServer {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}
