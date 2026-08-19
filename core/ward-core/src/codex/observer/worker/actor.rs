// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc::Receiver;
use tokio::time::Instant as TokioInstant;
use ward_codex::{CodexAppServerSource, CodexHistoryCancellation};

use super::super::ObserverOperationGate;
use super::super::commands::{
    CommandUpdate, DrainedCommands, ObserverCommand, WriteAccessRequest, drain_commands,
    merge_command,
};
use super::super::events::HistoryEventSink;
use super::operation::{OperationDrive, drive_operation};
use super::state::ObserverState;
use super::writer::{LIVE_DELTA_EMIT_INTERVAL, ObserverWake, WriterStreamUpdate};

const THREAD_PAGE_POLL_INTERVAL: Duration = Duration::from_secs(2);
const CONVERSATION_POLL_INTERVAL: Duration = Duration::from_millis(500);
const HISTORY_ERROR_RETRY_INTERVAL: Duration = Duration::from_secs(2);

fn finish_exclusive_operation<T>(
    operation: OperationDrive<T>,
    active_operation: &ObserverOperationGate,
    following: &mut CommandUpdate,
) -> Result<T, ()> {
    active_operation.release();
    let OperationDrive::Completed { output, deferred } = operation else {
        return Err(());
    };
    if let Some(deferred) = deferred {
        following.merge(deferred);
    }
    Ok(output)
}

fn watch_created_thread(
    thread_id: String,
    now: TokioInstant,
    watched_thread: &mut Option<String>,
    conversation_due: &mut Option<TokioInstant>,
    live_emit_due: &mut Option<TokioInstant>,
) {
    *watched_thread = Some(thread_id);
    *conversation_due = Some(now + CONVERSATION_POLL_INTERVAL);
    *live_emit_due = None;
}

pub(in crate::codex::observer) async fn run_observer(
    source: CodexAppServerSource,
    mut receiver: Receiver<ObserverCommand>,
    sink: HistoryEventSink,
    cancellation: CodexHistoryCancellation,
    active_operation: Arc<ObserverOperationGate>,
) {
    let mut state = ObserverState::new(source, cancellation.clone());
    let mut watched_thread: Option<String> = None;
    let mut model_catalog_due = Some(TokioInstant::now());
    let mut threads_due = TokioInstant::now();
    let mut conversation_due: Option<TokioInstant> = None;
    let mut live_emit_due: Option<TokioInstant> = None;
    let mut deferred_update = None;

    'observer: loop {
        if deferred_update.is_none() {
            let now = TokioInstant::now();
            if live_emit_due.is_some_and(|due| now >= due) {
                if let Some(thread_id) = watched_thread.as_deref() {
                    state.flush_pending_live_conversation(thread_id, &sink);
                }
                live_emit_due = None;
            }
            if now >= threads_due {
                let OperationDrive::Completed {
                    output: succeeded,
                    deferred,
                } = drive_operation(state.poll_threads(&sink), &mut receiver, &cancellation).await
                else {
                    break;
                };
                threads_due = TokioInstant::now()
                    + if succeeded {
                        THREAD_PAGE_POLL_INTERVAL
                    } else {
                        HISTORY_ERROR_RETRY_INTERVAL
                    };
                deferred_update = deferred;
            }
            if deferred_update.is_none() && model_catalog_due.is_some_and(|due| now >= due) {
                let OperationDrive::Completed {
                    output: succeeded,
                    deferred,
                } = drive_operation(
                    state.load_model_catalog(&sink),
                    &mut receiver,
                    &cancellation,
                )
                .await
                else {
                    break;
                };
                model_catalog_due =
                    (!succeeded).then(|| TokioInstant::now() + HISTORY_ERROR_RETRY_INTERVAL);
                deferred_update = deferred;
            }

            if deferred_update.is_none()
                && let (Some(thread_id), Some(due)) = (watched_thread.as_deref(), conversation_due)
                && TokioInstant::now() >= due
            {
                let thread_id = thread_id.to_owned();
                let OperationDrive::Completed {
                    output: succeeded,
                    deferred,
                } = drive_operation(
                    state.poll_conversation(&thread_id, &sink),
                    &mut receiver,
                    &cancellation,
                )
                .await
                else {
                    break;
                };
                conversation_due = Some(
                    TokioInstant::now()
                        + if succeeded {
                            CONVERSATION_POLL_INTERVAL
                        } else {
                            HISTORY_ERROR_RETRY_INTERVAL
                        },
                );
                deferred_update = deferred;
            }
        }

        let drained = if let Some(update) = deferred_update.take() {
            Some(DrainedCommands::Update(update))
        } else {
            let next_due = [
                model_catalog_due,
                Some(threads_due),
                conversation_due,
                live_emit_due,
            ]
            .into_iter()
            .flatten()
            .min()
            .expect("the thread poll always has a deadline");
            let sleep = tokio::time::sleep_until(next_due);
            tokio::pin!(sleep);
            let wake = tokio::select! {
                biased;
                _ = cancellation.cancelled() => ObserverWake::Cancelled,
                command = receiver.recv() => ObserverWake::Command(command),
                update = state.next_writer_update() => ObserverWake::Writer(Box::new(update)),
                () = &mut sleep => ObserverWake::Timer,
            };
            match wake {
                ObserverWake::Cancelled | ObserverWake::Command(None) => None,
                ObserverWake::Command(Some(command)) => {
                    Some(drain_commands(command, &mut receiver))
                }
                ObserverWake::Writer(update) => {
                    match *update {
                        WriterStreamUpdate::Event { thread_id, event } => {
                            if watched_thread.as_deref() == Some(thread_id.as_str()) {
                                state.accept_writer_event(&thread_id, event, &sink);
                                if state.has_pending_conversation_emit() {
                                    live_emit_due.get_or_insert_with(|| {
                                        TokioInstant::now() + LIVE_DELTA_EMIT_INTERVAL
                                    });
                                } else {
                                    live_emit_due = None;
                                }
                            }
                        }
                        WriterStreamUpdate::Error { thread_id, error } => {
                            if cancellation.is_cancelled() {
                                break;
                            }
                            state.fail_writer_stream(&thread_id, error, &sink).await;
                            live_emit_due = None;
                        }
                    }
                    continue;
                }
                ObserverWake::Timer => continue,
            }
        };
        match drained {
            Some(drained) => match drained {
                DrainedCommands::Stop => break,
                DrainedCommands::Update(mut update) => {
                    if update.is_exclusive_operation_only()
                        && let Ok(command) = receiver.try_recv()
                    {
                        if !merge_command(&mut update, command) {
                            break 'observer;
                        }
                        while let Ok(command) = receiver.try_recv() {
                            if !merge_command(&mut update, command) {
                                break 'observer;
                            }
                        }
                    }
                    let now = TokioInstant::now();
                    let CommandUpdate {
                        watched_thread: requested_thread,
                        thread_list_scope,
                        refresh,
                        write_access,
                        thread_rename,
                        thread_fork,
                        thread_lifecycle,
                        thread_start,
                        turn,
                        controls,
                    } = update;
                    let mut following = CommandUpdate::default();

                    if let Some(scope) = thread_list_scope
                        && state.set_thread_list_scope(scope)
                    {
                        let OperationDrive::Completed { deferred, .. } =
                            drive_operation(state.select_thread(), &mut receiver, &cancellation)
                                .await
                        else {
                            break 'observer;
                        };
                        if let Some(deferred) = deferred {
                            following.merge(deferred);
                        }
                        watched_thread = None;
                        threads_due = now;
                        conversation_due = None;
                        live_emit_due = None;
                    }
                    if let Some(thread_id) = requested_thread {
                        let OperationDrive::Completed { deferred, .. } =
                            drive_operation(state.select_thread(), &mut receiver, &cancellation)
                                .await
                        else {
                            break 'observer;
                        };
                        if let Some(deferred) = deferred {
                            following.merge(deferred);
                        }
                        watched_thread = Some(thread_id);
                        if let Some(thread_id) = watched_thread.as_deref() {
                            sink.emit_pending_interactions(thread_id, std::iter::empty());
                        }
                        conversation_due = Some(now);
                        live_emit_due = None;
                    }
                    if refresh {
                        state.refresh_model_catalog();
                        state.refresh();
                        model_catalog_due = Some(now);
                        threads_due = now;
                        if watched_thread.is_some() {
                            conversation_due = Some(now);
                        }
                    }
                    if let Some(request) = thread_rename {
                        let renamed_thread_id = request.thread_id.clone();
                        let OperationDrive::Completed {
                            output: renamed,
                            deferred,
                        } = drive_operation(
                            state.rename_thread(*request, &sink),
                            &mut receiver,
                            &cancellation,
                        )
                        .await
                        else {
                            break 'observer;
                        };
                        if let Some(deferred) = deferred {
                            following.merge(deferred);
                        }
                        if renamed {
                            threads_due = now;
                            if watched_thread.as_deref() == Some(renamed_thread_id.as_str()) {
                                conversation_due = Some(now);
                            }
                        }
                    }
                    if let Some(request) = thread_fork {
                        let Ok(forked_thread_id) = finish_exclusive_operation(
                            drive_operation(
                                state.fork_thread(*request, &sink),
                                &mut receiver,
                                &cancellation,
                            )
                            .await,
                            &active_operation,
                            &mut following,
                        ) else {
                            break 'observer;
                        };
                        threads_due = now;
                        if let Some(thread_id) = forked_thread_id {
                            watch_created_thread(
                                thread_id,
                                now,
                                &mut watched_thread,
                                &mut conversation_due,
                                &mut live_emit_due,
                            );
                        }
                    }
                    for request in thread_lifecycle {
                        let changed_thread_id = request.thread_id.clone();
                        let OperationDrive::Completed {
                            output: changed,
                            deferred,
                        } = drive_operation(
                            state.change_thread_lifecycle(request, &sink),
                            &mut receiver,
                            &cancellation,
                        )
                        .await
                        else {
                            break 'observer;
                        };
                        if let Some(deferred) = deferred {
                            following.merge(deferred);
                        }
                        if changed {
                            if watched_thread.as_deref() == Some(changed_thread_id.as_str()) {
                                let OperationDrive::Completed { deferred, .. } = drive_operation(
                                    state.select_thread(),
                                    &mut receiver,
                                    &cancellation,
                                )
                                .await
                                else {
                                    break 'observer;
                                };
                                if let Some(deferred) = deferred {
                                    following.merge(deferred);
                                }
                                watched_thread = None;
                                conversation_due = None;
                                live_emit_due = None;
                            }
                            threads_due = now;
                        }
                    }
                    if let Some(request) = write_access {
                        match request {
                            WriteAccessRequest::Acquire(thread_id)
                                if watched_thread.as_deref() == Some(thread_id.as_str()) =>
                            {
                                let OperationDrive::Completed { deferred, .. } = drive_operation(
                                    state.acquire_write(&thread_id, &sink),
                                    &mut receiver,
                                    &cancellation,
                                )
                                .await
                                else {
                                    break 'observer;
                                };
                                if let Some(deferred) = deferred {
                                    following.merge(deferred);
                                }
                            }
                            WriteAccessRequest::Acquire(_) => {}
                            WriteAccessRequest::Release(thread_id) => {
                                let OperationDrive::Completed { deferred, .. } = drive_operation(
                                    state.release_write(&thread_id, &sink),
                                    &mut receiver,
                                    &cancellation,
                                )
                                .await
                                else {
                                    break 'observer;
                                };
                                if let Some(deferred) = deferred {
                                    following.merge(deferred);
                                }
                                live_emit_due = None;
                            }
                        }
                    }
                    if let Some(request) = thread_start {
                        let Ok(started_thread_id) = finish_exclusive_operation(
                            drive_operation(
                                state.start_thread(*request, &sink),
                                &mut receiver,
                                &cancellation,
                            )
                            .await,
                            &active_operation,
                            &mut following,
                        ) else {
                            break 'observer;
                        };
                        if let Some(thread_id) = started_thread_id {
                            let now = TokioInstant::now();
                            threads_due = now;
                            watch_created_thread(
                                thread_id,
                                now,
                                &mut watched_thread,
                                &mut conversation_due,
                                &mut live_emit_due,
                            );
                        }
                    }
                    if let Some(request) = turn {
                        if watched_thread.as_deref() != Some(request.thread_id.as_str()) {
                            let OperationDrive::Completed { deferred, .. } = drive_operation(
                                state.select_thread(),
                                &mut receiver,
                                &cancellation,
                            )
                            .await
                            else {
                                break 'observer;
                            };
                            if let Some(deferred) = deferred {
                                following.merge(deferred);
                            }
                            watched_thread = Some(request.thread_id.clone());
                        }
                        let OperationDrive::Completed {
                            output: succeeded,
                            deferred,
                        } = state
                            .run_turn(*request, &sink, &mut receiver, controls)
                            .await
                        else {
                            break 'observer;
                        };
                        active_operation.release();
                        if let Some(update) = deferred {
                            following.merge(update);
                        }
                        threads_due = TokioInstant::now();
                        conversation_due = Some(
                            TokioInstant::now()
                                + if succeeded {
                                    CONVERSATION_POLL_INTERVAL
                                } else {
                                    HISTORY_ERROR_RETRY_INTERVAL
                                },
                        );
                    } else {
                        state.apply_thread_controls(controls, &sink).await;
                    }
                    if !following.is_empty() {
                        deferred_update = Some(following);
                    }
                }
            },
            None => break,
        }
    }
    active_operation.release();
    state.shutdown().await;
}
