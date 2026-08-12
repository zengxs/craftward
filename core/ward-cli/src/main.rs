// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    let exit_code = ward_cli::run(env::args_os());
    ExitCode::from(u8::try_from(exit_code).unwrap_or(1))
}
