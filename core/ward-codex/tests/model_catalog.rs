// Copyright (C) 2026 Xiangsong Zeng
// SPDX-License-Identifier: GPL-3.0-or-later

use ward_codex::CodexClient;
use ward_codex_test_support::FakeCodexAppServer;

#[tokio::test]
async fn lists_the_complete_visible_model_catalog_through_the_public_client() {
    let fake_app_server = FakeCodexAppServer::default();
    let mut client = CodexClient::connect(fake_app_server.source())
        .await
        .expect("the public client should connect");

    let catalog = client
        .list_models()
        .await
        .expect("the complete visible model catalog should load");

    assert_eq!(catalog.models.len(), 2);

    let balanced = &catalog.models[0];
    assert_eq!(balanced.id, "balanced");
    assert_eq!(balanced.model, "gpt-balanced");
    assert_eq!(balanced.display_name, "Balanced");
    assert_eq!(balanced.description, "Balances capability and speed.");
    assert!(balanced.is_default);
    assert_eq!(balanced.default_reasoning_effort.as_str(), "medium");
    assert_eq!(balanced.supported_reasoning_efforts.len(), 3);
    assert_eq!(
        balanced.supported_reasoning_efforts[0].effort.as_str(),
        "low"
    );
    assert_eq!(
        balanced.supported_reasoning_efforts[0].description,
        "Faster responses"
    );

    let fast = &catalog.models[1];
    assert_eq!(fast.id, "fast");
    assert_eq!(fast.model, "gpt-fast");
    assert_eq!(fast.display_name, "Fast");
    assert!(!fast.is_default);
    assert_eq!(fast.default_reasoning_effort.as_str(), "low");

    client.shutdown().await;
}
