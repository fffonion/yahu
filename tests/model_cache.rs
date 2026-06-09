use serde_json::json;
use std::time::{Duration, Instant};

#[test]
fn model_inventory_payload_flattens_hermes_picker_providers_without_api_server_placeholder() {
    let payload = json!({
        "providers": [
            {"slug": "minimax-cn", "name": "MiniMax CN", "models": ["MiniMax-M3", "MiniMax-M2.7"]},
            {"slug": "openrouter", "name": "OpenRouter", "models": ["anthropic/claude-sonnet-4.6"]}
        ],
        "model": "MiniMax-M3"
    });

    let flattened = hermes_headless_webui::flatten_model_options(&payload);

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
            {"id": "MiniMax-M3", "object": "model", "provider": "minimax-cn"},
            {"id": "openrouter/gpt-oss-120b", "object": "model", "provider": "openrouter", "label": "OpenRouter · openrouter/gpt-oss-120b"}
        ]
    });

    let cached = hermes_headless_webui::model_cache_payload_from_source(&payload, "api_server");
    let data = cached["data"].as_array().unwrap();

    assert_eq!(cached["source"], "api_server");
    assert_eq!(data.len(), 2);
    assert!(data.iter().all(|row| row["id"] != "hermes-agent"));
    assert_eq!(data[0]["id"], "MiniMax-M3");
}

#[test]
fn model_cache_returns_fresh_cached_body() {
    let mut cache = hermes_headless_webui::ModelCache::default();
    cache.fetched_at = Some(Instant::now());
    cache.body = Some(json!({"object": "list", "data": [{"id": "MiniMax-M3"}]}));

    let cached = hermes_headless_webui::fresh_model_cache_body(&cache, Duration::from_secs(300));

    assert_eq!(cached.unwrap()["data"][0]["id"], "MiniMax-M3");
}
