// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let exit_code = match ward_runtime::WardRuntime::new() {
        Ok(runtime) => runtime.block_on(ward_cli::run(env::args_os())),
        Err(error) => {
            eprintln!("error: failed to start the Ward async runtime: {error}");
            1
        }
    };
    ExitCode::from(u8::try_from(exit_code).unwrap_or(1))
}
