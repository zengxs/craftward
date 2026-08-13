// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

//! Command-line entry points shared by the standalone Ward executable and
//! Craftward's embedded command-line mode.

mod commands;

use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;

use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};

use commands::CodexArguments;

/// The result of asking the embedded command-line module to handle an
/// invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CliDisposition {
    /// No command-line arguments were supplied, so the GUI may start.
    NotRequested,
    /// The invocation was handled as a command-line request.
    Exit(i32),
}

#[derive(Debug, Parser)]
#[command(
    name = "ward",
    version,
    about = "Manage Craftward environments and agents",
    subcommand_required = true
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Inspect or interact with a Codex app-server.
    Codex(CodexArguments),
}

/// Runs a standalone CLI invocation and returns its process exit code.
///
/// An invocation without a subcommand displays help successfully. Clap writes
/// help and parse errors directly to the process streams so its normal terminal
/// detection and color behavior remain intact.
pub async fn run<I, T>(args: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    run_with_output(args, ProcessOutput::new()).await
}

/// Handles an embedded CLI invocation when arguments were supplied.
///
/// The GUI executable calls this before constructing its GUI application. A
/// no-argument invocation is deliberately left unhandled so normal GUI startup
/// remains the default.
pub async fn try_run<I, T>(args: I) -> CliDisposition
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    try_run_with_output(args, ProcessOutput::new()).await
}

async fn run_with_output<I, T, O>(args: I, mut output: O) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
    O: CliOutput,
{
    let args = collect_arguments(args);
    if args.len() <= 1 {
        return write_help(&args, &mut output);
    }
    run_arguments(args, &mut output).await
}

async fn try_run_with_output<I, T, O>(args: I, mut output: O) -> CliDisposition
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
    O: CliOutput,
{
    let args = collect_arguments(args);
    if args.len() <= 1 {
        return CliDisposition::NotRequested;
    }
    CliDisposition::Exit(run_arguments(args, &mut output).await)
}

fn collect_arguments<I, T>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    args.into_iter().map(Into::into).collect()
}

async fn run_arguments(args: Vec<OsString>, output: &mut impl CliOutput) -> i32 {
    let cli = match parse_arguments(args) {
        Ok(cli) => cli,
        Err(error) => {
            let exit_code = error.exit_code();
            return if output.print_clap_error(&error).is_ok() {
                exit_code
            } else {
                1
            };
        }
    };

    let result = match cli.command {
        Command::Codex(arguments) => arguments.run(output.stdout()).await,
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(output.stderr(), "error: {error}");
            1
        }
    }
}

fn parse_arguments(args: Vec<OsString>) -> Result<Cli, clap::Error> {
    let mut matches = command_for(&args).try_get_matches_from(args)?;
    Cli::from_arg_matches_mut(&mut matches)
}

fn write_help(args: &[OsString], output: &mut impl CliOutput) -> i32 {
    let mut command = command_for(args);
    if output.print_long_help(&mut command).is_ok() {
        0
    } else {
        1
    }
}

trait CliOutput {
    fn print_clap_error(&mut self, error: &clap::Error) -> io::Result<()>;
    fn print_long_help(&mut self, command: &mut clap::Command) -> io::Result<()>;
    fn stdout(&mut self) -> &mut dyn Write;
    fn stderr(&mut self) -> &mut dyn Write;
}

struct ProcessOutput {
    stdout: io::Stdout,
    stderr: io::Stderr,
}

impl ProcessOutput {
    fn new() -> Self {
        Self {
            stdout: io::stdout(),
            stderr: io::stderr(),
        }
    }
}

impl CliOutput for ProcessOutput {
    fn print_clap_error(&mut self, error: &clap::Error) -> io::Result<()> {
        error.print()
    }

    fn print_long_help(&mut self, command: &mut clap::Command) -> io::Result<()> {
        command.print_long_help()?;
        writeln!(self.stdout)
    }

    fn stdout(&mut self) -> &mut dyn Write {
        &mut self.stdout
    }

    fn stderr(&mut self) -> &mut dyn Write {
        &mut self.stderr
    }
}

fn command_for(args: &[OsString]) -> clap::Command {
    let name = args
        .first()
        .and_then(|argument| Path::new(argument).file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "ward".to_owned());
    Cli::command().name(name.clone()).bin_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct CapturedOutput<'a> {
        stdout: &'a mut dyn Write,
        stderr: &'a mut dyn Write,
    }

    impl CliOutput for CapturedOutput<'_> {
        fn print_clap_error(&mut self, error: &clap::Error) -> io::Result<()> {
            let destination: &mut dyn Write = match error.kind() {
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    &mut *self.stdout
                }
                _ => &mut *self.stderr,
            };
            destination.write_all(error.to_string().as_bytes())
        }

        fn print_long_help(&mut self, command: &mut clap::Command) -> io::Result<()> {
            command.write_long_help(&mut self.stdout)?;
            writeln!(self.stdout)
        }

        fn stdout(&mut self) -> &mut dyn Write {
            self.stdout
        }

        fn stderr(&mut self) -> &mut dyn Write {
            self.stderr
        }
    }

    async fn run_captured<I, T>(args: I, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        run_with_output(args, CapturedOutput { stdout, stderr }).await
    }

    async fn try_run_captured<I, T>(
        args: I,
        stdout: &mut dyn Write,
        stderr: &mut dyn Write,
    ) -> CliDisposition
    where
        I: IntoIterator<Item = T>,
        T: Into<OsString>,
    {
        try_run_with_output(args, CapturedOutput { stdout, stderr }).await
    }

    #[tokio::test]
    async fn leaves_an_argument_free_embedded_invocation_for_the_gui() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let disposition = try_run_captured(["Craftward"], &mut stdout, &mut stderr).await;

        assert_eq!(disposition, CliDisposition::NotRequested);
        assert!(stdout.is_empty());
        assert!(stderr.is_empty());
    }

    #[tokio::test]
    async fn handles_embedded_help_without_starting_a_command() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let disposition = try_run_captured(["Craftward", "--help"], &mut stdout, &mut stderr).await;

        assert_eq!(disposition, CliDisposition::Exit(0));
        assert!(
            String::from_utf8(stdout)
                .unwrap()
                .contains("Usage: Craftward")
        );
        assert!(stderr.is_empty());
    }

    #[tokio::test]
    async fn displays_help_for_an_argument_free_standalone_invocation() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let exit_code = run_captured(["ward"], &mut stdout, &mut stderr).await;

        assert_eq!(exit_code, 0);
        assert!(String::from_utf8(stdout).unwrap().contains("Usage: ward"));
        assert!(stderr.is_empty());
    }

    #[tokio::test]
    async fn uses_the_actual_embedded_executable_name_for_version_output() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let disposition =
            try_run_captured(["Craftward", "--version"], &mut stdout, &mut stderr).await;

        assert_eq!(disposition, CliDisposition::Exit(0));
        assert_eq!(
            String::from_utf8(stdout).unwrap(),
            format!("Craftward {}\n", env!("CARGO_PKG_VERSION"))
        );
        assert!(stderr.is_empty());
    }

    #[tokio::test]
    async fn handles_invalid_embedded_arguments_as_a_cli_error() {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();

        let disposition =
            try_run_captured(["Craftward", "--unknown"], &mut stdout, &mut stderr).await;

        assert_eq!(disposition, CliDisposition::Exit(2));
        assert!(stdout.is_empty());
        assert!(
            String::from_utf8(stderr)
                .unwrap()
                .contains("unexpected argument '--unknown'")
        );
    }
}
