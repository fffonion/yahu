use std::path::Path;

#[test]
fn provider_usage_sources_have_cohesive_owners() {
    let backend = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/backend");
    let entry = std::fs::read_to_string(backend.join("provider_usage.rs")).unwrap();
    let owners = [
        ("types", "struct ProviderUsageSection"),
        ("credentials", "fn provider_env_value"),
        ("catalog", "fn provider_usage_catalog"),
        ("http", "async fn provider_http_get_json"),
        ("formatting", "fn fmt_provider_money"),
        ("cache", "fn shared_cached_section"),
        ("daily", "fn provider_observed_daily"),
        ("api", "async fn provider_usage_handler"),
        ("icons", "async fn provider_icon"),
        ("newapi", "async fn fetch_newapi_usage"),
        ("openrouter", "async fn fetch_openrouter_usage"),
        ("deepseek", "async fn fetch_deepseek_usage"),
        ("atlascloud", "async fn fetch_atlascloud_usage"),
        ("minimax", "async fn fetch_minimax_usage"),
        ("stepfun", "async fn fetch_stepfun_usage"),
        ("kimi", "async fn fetch_kimi_usage"),
        ("mimo", "async fn fetch_mimo_usage"),
        ("sensenova", "async fn fetch_sensenova_usage"),
        ("opencode", "async fn fetch_opencode_usage"),
        ("commandcode", "async fn fetch_commandcode_usage"),
        ("codex", "async fn fetch_codex_usage"),
        ("grok", "async fn fetch_grok_usage"),
        ("zed", "async fn fetch_zed_pro_usage"),
        ("vyceai", "async fn fetch_vyceai_usage_from_url"),
        ("tests", "mod zed_pro_tests"),
    ];
    for (owner, symbol) in owners {
        let relative = format!("provider_usage/{owner}.rs");
        assert!(
            entry.contains(&format!("include!(\"{relative}\");")),
            "provider usage entry point must include {owner}"
        );
        let source = std::fs::read_to_string(backend.join(relative)).unwrap();
        assert!(source.contains(symbol), "{owner} must own {symbol}");
        assert!(!entry.contains(symbol), "entry point must not own {symbol}");
    }
    assert_eq!(entry.matches("include!(").count(), owners.len());
}
