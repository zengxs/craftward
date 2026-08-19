// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use super::*;

#[tokio::test]
async fn accepts_and_coalesces_commands_while_an_operation_is_in_flight() {
    let (sender, mut receiver) = mpsc::channel(COMMAND_QUEUE_CAPACITY);
    let cancellation = CodexHistoryCancellation::new();
    let (start_commands, commands_started) = oneshot::channel();
    let (commands_sent, wait_for_commands) = oneshot::channel();
    let producer = tokio::spawn(async move {
        commands_started.await.unwrap();
        sender
            .send(ObserverCommand::Watch("thread-2".to_owned()))
            .await
            .unwrap();
        sender.send(ObserverCommand::Refresh).await.unwrap();
        commands_sent.send(()).unwrap();
        std::future::pending::<()>().await;
    });
    let operation = async move {
        start_commands.send(()).unwrap();
        wait_for_commands.await.unwrap();
        tokio::time::sleep(Duration::from_millis(10)).await;
        42
    };

    let result = drive_operation(operation, &mut receiver, &cancellation).await;
    producer.abort();

    let OperationDrive::Completed { output, deferred } = result else {
        panic!("the operation should complete");
    };
    assert_eq!(output, 42);
    assert_eq!(
        deferred,
        Some(CommandUpdate {
            watched_thread: Some("thread-2".to_owned()),
            refresh: true,
            ..CommandUpdate::default()
        })
    );
}
