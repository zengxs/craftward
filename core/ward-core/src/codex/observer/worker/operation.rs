// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use std::future::Future;

use tokio::sync::mpsc::Receiver;
use ward_codex::CodexHistoryCancellation;

use super::super::commands::{CommandUpdate, ObserverCommand, merge_command};

pub(super) enum OperationDrive<T> {
    Completed {
        output: T,
        deferred: Option<CommandUpdate>,
    },
    Stop,
}

pub(super) async fn drive_operation<F>(
    operation: F,
    receiver: &mut Receiver<ObserverCommand>,
    cancellation: &CodexHistoryCancellation,
) -> OperationDrive<F::Output>
where
    F: Future,
{
    tokio::pin!(operation);
    let mut deferred = CommandUpdate::default();

    loop {
        if cancellation.is_cancelled() {
            let _ = operation.as_mut().await;
            return OperationDrive::Stop;
        }
        tokio::select! {
            _ = cancellation.cancelled() => {
                let _ = operation.as_mut().await;
                return OperationDrive::Stop;
            },
            command = receiver.recv() => {
                let keep_running = command
                    .is_some_and(|command| merge_command(&mut deferred, command));
                if !keep_running {
                    cancellation.cancel();
                    let _ = operation.as_mut().await;
                    return OperationDrive::Stop;
                }
            },
            output = &mut operation => {
                return OperationDrive::Completed {
                    output,
                    deferred: (!deferred.is_empty()).then_some(deferred),
                };
            }
        }
    }
}
