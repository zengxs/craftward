// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::error::Error;
use std::io::Write;
use std::path::PathBuf;

use clap::{Args, Subcommand};
use ward_codex::{
    CodexClient, CodexHistoryCancellation, CodexHistorySession, CodexThreadWriter, ThreadItem,
    ThreadListOptions, ThreadRuntimeStatus, ThreadStartOptions, ThreadSubscription, UserInput,
};

mod watch;

const CODEX_PATH_ENVIRONMENT_VARIABLE: &str = "CRAFTWARD_CODEX_PATH";

type CommandResult = Result<(), Box<dyn Error>>;

#[derive(Debug, Args)]
pub(crate) struct CodexArguments {
    /// Override the Codex executable used to start the app-server.
    ///
    /// Defaults to CRAFTWARD_CODEX_PATH, then `codex` from PATH.
    #[arg(long = "codex", value_name = "PATH")]
    executable: Option<PathBuf>,

    #[command(subcommand)]
    command: CodexCommand,
}

impl CodexArguments {
    pub(crate) async fn run(self, output: &mut dyn Write) -> CommandResult {
        let executable = resolve_codex_executable(self.executable);
        match self.command {
            CodexCommand::Check => {
                let mut client = CodexClient::spawn(executable).await?;
                let result = check(&mut client, output).await;
                client.shutdown().await;
                result
            }
            CodexCommand::List(arguments) => {
                let mut client = CodexClient::spawn(executable).await?;
                let result = list(&mut client, arguments, output).await;
                client.shutdown().await;
                result
            }
            CodexCommand::Read { thread_id } => {
                let mut client = CodexClient::spawn(executable).await?;
                let result = read(&mut client, &thread_id, output).await;
                client.shutdown().await;
                result
            }
            CodexCommand::Watch(arguments) => {
                let mut session = CodexHistorySession::spawn(executable).await?;
                let result = watch::run(&mut session, arguments, output).await;
                session.shutdown().await;
                result
            }
            CodexCommand::ProbeStart { working_directory } => {
                let (writer, subscription) = CodexThreadWriter::start_with_cancellation(
                    executable,
                    CodexHistoryCancellation::new(),
                    &working_directory,
                    ThreadStartOptions { ephemeral: true },
                )
                .await?;
                let result = display_started_thread(&subscription, output);
                writer.shutdown().await;
                result
            }
        }
    }
}

#[derive(Debug, Subcommand)]
enum CodexCommand {
    /// Verify that thread history can be listed and read.
    Check,
    /// List persisted Codex threads.
    List(ListArguments),
    /// Display the persisted conversation for one thread.
    Read {
        /// Identifier of the thread to read.
        thread_id: String,
    },
    /// Watch a persisted thread for changes made by another app-server.
    Watch(watch::WatchArguments),
    /// Start an ephemeral thread to verify writer connectivity.
    ProbeStart {
        /// Working directory to make visible to the app-server.
        #[arg(long = "cwd", value_name = "PATH")]
        working_directory: PathBuf,
    },
}

#[derive(Debug, Args)]
struct ListArguments {
    /// Maximum number of thread summaries to return.
    #[arg(long, default_value_t = 20)]
    limit: u32,
    /// List archived threads instead of active threads.
    #[arg(long)]
    archived: bool,
    /// Continue listing from an opaque app-server cursor.
    #[arg(long)]
    cursor: Option<String>,
}

async fn check(client: &mut CodexClient, output: &mut dyn Write) -> CommandResult {
    writeln!(output, "Connected to {}", client.server_info().user_agent)?;
    let page = client
        .list_threads(&ThreadListOptions {
            limit: Some(1),
            ..ThreadListOptions::default()
        })
        .await?;
    let Some(summary) = page.threads.first() else {
        writeln!(output, "The state database contains no threads")?;
        return Ok(());
    };
    let thread = client.read_thread(&summary.id).await?;
    let item_count = thread
        .turns
        .iter()
        .map(|turn| turn.items.len())
        .sum::<usize>();
    writeln!(
        output,
        "Read one thread with {} turns and {item_count} items",
        thread.turns.len()
    )?;
    Ok(())
}

async fn list(
    client: &mut CodexClient,
    arguments: ListArguments,
    output: &mut dyn Write,
) -> CommandResult {
    let page = client
        .list_threads(&ThreadListOptions {
            cursor: arguments.cursor,
            limit: Some(arguments.limit),
            archived: arguments.archived.then_some(true),
        })
        .await?;
    writeln!(output, "Found {} threads", page.threads.len())?;
    for thread in page.threads {
        let title = thread.name.as_deref().unwrap_or(&thread.preview);
        writeln!(output, "{}  {}", thread.id, one_line(title))?;
    }
    if let Some(cursor) = page.next_cursor {
        writeln!(output, "Next cursor: {cursor}")?;
    }
    Ok(())
}

async fn read(client: &mut CodexClient, thread_id: &str, output: &mut dyn Write) -> CommandResult {
    let thread = client.read_thread(thread_id).await?;
    let title = thread
        .summary
        .name
        .as_deref()
        .unwrap_or(&thread.summary.preview);
    writeln!(output, "{}\n", one_line(title))?;
    for turn in thread.turns {
        for item in turn.items {
            match item {
                ThreadItem::UserMessage { content, .. } => {
                    writeln!(output, "USER")?;
                    for input in content {
                        display_user_input(input, output)?;
                    }
                    writeln!(output)?;
                }
                ThreadItem::AgentMessage { text, .. } => {
                    writeln!(output, "AGENT\n{text}\n")?;
                }
                ThreadItem::Other { .. } => {}
                _ => {}
            }
        }
    }
    Ok(())
}

fn display_started_thread(
    subscription: &ThreadSubscription,
    output: &mut dyn Write,
) -> CommandResult {
    writeln!(
        output,
        "Started ephemeral thread {}",
        subscription.thread.summary.id
    )?;
    writeln!(
        output,
        "Working directory: {}",
        subscription.thread.summary.cwd.display()
    )?;
    writeln!(
        output,
        "Runtime status: {}",
        runtime_status_name(&subscription.runtime_status)
    )?;
    Ok(())
}

fn runtime_status_name(status: &ThreadRuntimeStatus) -> &str {
    match status {
        ThreadRuntimeStatus::NotLoaded => "not loaded",
        ThreadRuntimeStatus::Idle => "idle",
        ThreadRuntimeStatus::Active { .. } => "active",
        ThreadRuntimeStatus::SystemError => "system error",
        ThreadRuntimeStatus::Unknown(value) => value,
        _ => "unknown",
    }
}

fn display_user_input(input: UserInput, output: &mut dyn Write) -> CommandResult {
    match input {
        UserInput::Text(text) => writeln!(output, "{text}")?,
        UserInput::Image { url } => writeln!(output, "[image: {url}]")?,
        UserInput::LocalImage { path } => writeln!(output, "[image: {}]", path.display())?,
        UserInput::Audio { url } => writeln!(output, "[audio: {url}]")?,
        UserInput::LocalAudio { path } => writeln!(output, "[audio: {}]", path.display())?,
        UserInput::Skill { name, path } => {
            writeln!(output, "[skill: {name} ({})]", path.display())?;
        }
        UserInput::Mention { name, path } => {
            writeln!(output, "[mention: {name} ({})]", path.display())?;
        }
        UserInput::Other { kind } => writeln!(output, "[{kind}]")?,
        _ => writeln!(output, "[unsupported input]")?,
    }
    Ok(())
}

fn resolve_codex_executable(explicit: Option<PathBuf>) -> PathBuf {
    select_codex_executable(
        explicit,
        std::env::var_os(CODEX_PATH_ENVIRONMENT_VARIABLE).map(PathBuf::from),
    )
}

fn select_codex_executable(explicit: Option<PathBuf>, configured: Option<PathBuf>) -> PathBuf {
    explicit
        .or_else(|| configured.filter(|path| !path.as_os_str().is_empty()))
        .unwrap_or_else(|| PathBuf::from("codex"))
}

fn one_line(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_codex_path_takes_precedence_over_configuration() {
        let executable = select_codex_executable(
            Some(PathBuf::from("explicit-codex")),
            Some(PathBuf::from("configured-codex")),
        );

        assert_eq!(executable, PathBuf::from("explicit-codex"));
    }

    #[test]
    fn configured_codex_path_is_used_without_an_explicit_override() {
        let executable = select_codex_executable(None, Some(PathBuf::from("configured-codex")));

        assert_eq!(executable, PathBuf::from("configured-codex"));
    }

    #[test]
    fn codex_is_resolved_through_path_without_configuration() {
        assert_eq!(select_codex_executable(None, None), PathBuf::from("codex"));
        assert_eq!(
            select_codex_executable(None, Some(PathBuf::new())),
            PathBuf::from("codex")
        );
    }
}
