use serde_json::json;
use std::time::{Duration, Instant, UNIX_EPOCH};

#[test]
fn model_inventory_payload_flattens_hermes_picker_providers_without_api_server_placeholder() {
    let payload = json!({
        "providers": [
            {"slug": "minimax-cn", "name": "MiniMax CN", "models": ["MiniMax-M3", "MiniMax-M2.7"]},
            {"slug": "openrouter", "name": "OpenRouter", "models": ["anthropic/claude-sonnet-4.6"]}
        ],
        "model": "MiniMax-M3"
    });

    let flattened = yet_another_hermes_ui::flatten_model_options(&payload);

    assert_eq!(flattened[0]["id"], "MiniMax-M3");
    assert_eq!(flattened[0]["provider"], "minimax-cn");
    assert_eq!(flattened[0]["label"], "MiniMax CN · MiniMax-M3");
    assert!(flattened.iter().all(|row| row["id"] != "hermes-agent"));
}

#[test]
fn api_server_model_fallback_is_flattened_and_filters_hermes_agent_placeholder() {
    let payload = json!({
        "object": "list",
        "data": [
            {"id": "hermes-agent", "object": "model"},
            {"id": "MiniMax-M3", "object": "model", "provider": "minimax-cn", "context_length": 1000000},
            {"id": "openrouter/gpt-oss-120b", "object": "model", "provider": "openrouter", "label": "OpenRouter · openrouter/gpt-oss-120b", "limit": {"context": 131072}}
        ]
    });

    let cached = yet_another_hermes_ui::model_cache_payload_from_source(&payload, "api_server");
    let data = cached["data"].as_array().unwrap();

    assert_eq!(cached["source"], "api_server");
    assert_eq!(data.len(), 2);
    assert!(data.iter().all(|row| row["id"] != "hermes-agent"));
    assert_eq!(data[0]["id"], "MiniMax-M3");
    assert_eq!(data[0]["context_length"], 1000000);
    assert_eq!(data[1]["context_length"], 131072);
}

#[test]
fn provider_inventory_context_lengths_are_preserved_from_capabilities() {
    let payload = json!({
        "providers": [
            {
                "slug": "openrouter",
                "name": "OpenRouter",
                "models": ["anthropic/claude-sonnet-4.6"],
                "capabilities": {
                    "anthropic/claude-sonnet-4.6": {"context_length": 200000}
                }
            }
        ]
    });

    let flattened = yet_another_hermes_ui::flatten_model_options(&payload);

    assert_eq!(flattened[0]["context_length"], 200000);
}

#[test]
fn models_cache_backend_falls_back_when_api_server_only_returns_placeholder() {
    let source = include_str!("../src/backend/models.rs");

    assert!(source.contains("fetch_api_server_models(&state)"));
    assert!(source.contains("no non-placeholder models returned"));
    assert!(source.contains("load_hermes_model_inventory(&state)"));
    assert!(source.contains("hermes_cli.inventory"));
    assert!(source.contains("Command::new(python)"));
    assert!(!source.contains("fetch_dashboard_models"));
    assert!(!source.contains("/api/model/options"));
    assert!(!source.contains("HERMES_DASHBOARD"));
}

#[test]
fn model_cache_placeholder_only_payload_is_not_usable() {
    let payload = yet_another_hermes_ui::model_cache_payload_from_source(
        &json!({"object": "list", "data": [{"id": "hermes-agent", "object": "model"}]}),
        "api_server",
    );

    assert!(!yet_another_hermes_ui::model_cache_payload_has_models(
        &payload
    ));
}

#[test]
fn model_cache_non_placeholder_payload_is_usable() {
    let payload = yet_another_hermes_ui::model_cache_payload_from_source(
        &json!({"object": "list", "data": [{"id": "MiniMax-M3", "object": "model"}]}),
        "api_server",
    );

    assert!(yet_another_hermes_ui::model_cache_payload_has_models(
        &payload
    ));
}

#[test]
fn model_cache_returns_fresh_cached_body() {
    let cache = yet_another_hermes_ui::ModelCache {
        fetched_at: Some(Instant::now()),
        body: Some(json!({"object": "list", "data": [{"id": "MiniMax-M3"}]})),
    };

    let cached = yet_another_hermes_ui::fresh_model_cache_body(&cache, Duration::from_secs(300));

    assert_eq!(cached.unwrap()["data"][0]["id"], "MiniMax-M3");
}

#[test]
fn persisted_model_cache_survives_a_process_restart_without_extending_its_ttl() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("model-inventory.json");
    let body = json!({"object": "list", "data": [{"id": "MiniMax-M3", "provider": "minimax-cn"}]});
    let saved_at = UNIX_EPOCH + Duration::from_secs(1_000);

    yet_another_hermes_ui::persist_model_cache_body(&path, &body, saved_at).unwrap();

    let after_restart = yet_another_hermes_ui::fresh_persisted_model_cache_body(
        &path,
        Duration::from_secs(300),
        UNIX_EPOCH + Duration::from_secs(1_299),
    );
    assert_eq!(after_restart.unwrap()["data"][0]["id"], "MiniMax-M3");
    assert!(
        yet_another_hermes_ui::fresh_persisted_model_cache_body(
            &path,
            Duration::from_secs(300),
            UNIX_EPOCH + Duration::from_secs(1_301),
        )
        .is_none()
    );
}
