// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::HashMap;
use std::io::{self, Write};
use std::num::{NonZeroU32, NonZeroU64};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clap::Args;
use ward_codex::{
    AgentMessagePhase, CodexHistorySession, Thread, ThreadItem, ThreadPoll, UserInput,
};

use super::CommandResult;

#[derive(Debug, Args)]
pub(super) struct WatchArguments {
    /// Identifier of the persisted thread to observe.
    thread_id: String,
    /// Delay between polls in milliseconds.
    #[arg(long, default_value_t = NonZeroU64::new(1_000).unwrap(), value_name = "MILLISECONDS")]
    interval_ms: NonZeroU64,
    /// Stop after this many polls instead of watching indefinitely.
    #[arg(long, value_name = "COUNT")]
    polls: Option<NonZeroU32>,
}

pub(super) async fn run(
    session: &mut CodexHistorySession,
    arguments: WatchArguments,
    output: &mut dyn Write,
) -> CommandResult {
    let interval = Duration::from_millis(arguments.interval_ms.get());
    let mut polls_remaining = arguments.polls.map(NonZeroU32::get);
    let mut previous = None;
    writeln!(
        output,
        "Watching thread {} every {} ms",
        arguments.thread_id,
        arguments.interval_ms.get()
    )?;

    loop {
        let current = match session.poll_thread(&arguments.thread_id).await? {
            ThreadPoll::Baseline(thread) => {
                display_snapshot("baseline", None, &thread, output)?;
                Some(thread)
            }
            ThreadPoll::Changed(thread) => {
                display_snapshot("change", previous.as_ref(), &thread, output)?;
                Some(thread)
            }
            ThreadPoll::Unchanged => None,
            _ => None,
        };
        if current.is_some() {
            previous = current;
        }

        if let Some(remaining) = polls_remaining.as_mut() {
            *remaining -= 1;
            if *remaining == 0 {
                break;
            }
        }
        tokio::time::sleep(interval).await;
    }

    Ok(())
}

fn display_snapshot(
    label: &str,
    previous: Option<&Thread>,
    current: &Thread,
    output: &mut dyn Write,
) -> io::Result<()> {
    let observed_at_unix_milliseconds = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let item_count = current
        .turns
        .iter()
        .map(|turn| turn.items.len())
        .sum::<usize>();
    writeln!(
        output,
        "[{observed_at_unix_milliseconds}] {label}: history-updated={} turns={} items={item_count}",
        current.summary.updated_at_unix_seconds,
        current.turns.len()
    )?;

    let Some(previous) = previous else {
        return Ok(());
    };
    let previous_turns = previous
        .turns
        .iter()
        .map(|turn| (turn.id.as_str(), &turn.status))
        .collect::<HashMap<_, _>>();
    for turn in &current.turns {
        match previous_turns.get(turn.id.as_str()) {
            None => writeln!(output, "  new turn {} status={:?}", turn.id, turn.status)?,
            Some(status) if **status != turn.status => writeln!(
                output,
                "  changed turn {} status={:?}->{:?}",
                turn.id, status, turn.status
            )?,
            Some(_) => {}
        }
    }

    let previous_items = previous
        .turns
        .iter()
        .flat_map(|turn| &turn.items)
        .filter_map(|item| item_id(item).map(|id| (id, item)))
        .collect::<HashMap<_, _>>();
    for item in current.turns.iter().flat_map(|turn| &turn.items) {
        let Some(id) = item_id(item) else {
            continue;
        };
        let change = match previous_items.get(id) {
            None => "new",
            Some(previous_item) if *previous_item != item => "changed",
            Some(_) => continue,
        };
        display_item_change(change, item, output)?;
    }
    Ok(())
}

fn item_id(item: &ThreadItem) -> Option<&str> {
    match item {
        ThreadItem::UserMessage { id, .. }
        | ThreadItem::AgentMessage { id, .. }
        | ThreadItem::Other { id, .. } => Some(id),
        _ => None,
    }
}

fn display_item_change(change: &str, item: &ThreadItem, output: &mut dyn Write) -> io::Result<()> {
    match item {
        ThreadItem::UserMessage { id, content } => {
            let text = content
                .iter()
                .map(user_input_text)
                .collect::<Vec<_>>()
                .join(" ");
            writeln!(
                output,
                "  {change} user-message {id} bytes={} preview={}",
                text.len(),
                preview(&text)
            )?;
        }
        ThreadItem::AgentMessage { id, text, phase } => writeln!(
            output,
            "  {change} agent-message {id} phase={} bytes={} preview={}",
            phase_name(phase.as_ref()),
            text.len(),
            preview(text)
        )?,
        ThreadItem::Other { id, kind } => {
            writeln!(output, "  {change} item {id} type={kind}")?;
        }
        _ => {}
    }
    Ok(())
}

fn phase_name(phase: Option<&AgentMessagePhase>) -> &str {
    match phase {
        None => "unspecified",
        Some(AgentMessagePhase::Commentary) => "commentary",
        Some(AgentMessagePhase::FinalAnswer) => "final-answer",
        Some(AgentMessagePhase::Unknown(_)) => "other",
        Some(_) => "other",
    }
}

fn preview(value: &str) -> String {
    const CHARACTER_LIMIT: usize = 96;
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut characters = value.chars();
    let preview = characters
        .by_ref()
        .take(CHARACTER_LIMIT)
        .collect::<String>();
    if characters.next().is_some() {
        format!("{preview}…")
    } else {
        preview
    }
}

fn user_input_text(input: &UserInput) -> String {
    match input {
        UserInput::Text(text) => text.clone(),
        UserInput::Image { url } => format!("[image: {url}]"),
        UserInput::LocalImage { path } => format!("[image: {}]", path.display()),
        UserInput::Audio { url } => format!("[audio: {url}]"),
        UserInput::LocalAudio { path } => format!("[audio: {}]", path.display()),
        UserInput::Skill { name, path } => format!("[skill: {name} ({})]", path.display()),
        UserInput::Mention { name, path } => {
            format!("[mention: {name} ({})]", path.display())
        }
        UserInput::Other { kind } => format!("[{kind}]"),
        _ => "[unsupported input]".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use ward_codex::{ThreadSummary, Turn, TurnStatus};

    use super::*;

    fn observed_thread(agent_text: &str, include_new_message: bool) -> Thread {
        let mut items = vec![ThreadItem::AgentMessage {
            id: "agent-1".to_owned(),
            text: agent_text.to_owned(),
            phase: Some(AgentMessagePhase::Commentary),
        }];
        if include_new_message {
            items.push(ThreadItem::UserMessage {
                id: "user-2".to_owned(),
                content: vec![UserInput::Text("Continue".to_owned())],
            });
        }
        Thread {
            summary: ThreadSummary {
                id: "thread-1".to_owned(),
                name: Some("Example".to_owned()),
                preview: "Hello".to_owned(),
                cwd: PathBuf::from("/workspace"),
                created_at_unix_seconds: 10,
                updated_at_unix_seconds: 20,
            },
            turns: vec![Turn {
                id: "turn-1".to_owned(),
                status: TurnStatus::InProgress,
                timing: Default::default(),
                items,
            }],
        }
    }

    #[test]
    fn reports_only_new_and_changed_items_after_the_baseline() {
        let previous = observed_thread("Working", false);
        let current = observed_thread("Working on it", true);
        let mut output = Vec::new();

        display_snapshot("change", Some(&previous), &current, &mut output).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("changed agent-message agent-1"));
        assert!(output.contains("new user-message user-2"));
        assert!(!output.contains("new turn"));
    }

    #[test]
    fn truncates_watch_previews_on_character_boundaries() {
        let value = "界".repeat(100);

        let value = preview(&value);

        assert_eq!(value.chars().count(), 97);
        assert!(value.ends_with('…'));
    }
}
