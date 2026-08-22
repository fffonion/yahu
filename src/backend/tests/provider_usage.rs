#[cfg(test)]
mod provider_usage_tests {
    use super::*;

    #[test]
    fn env_value_reads_hermes_env_file_without_leaking_secrets() {
        // provider_env_value prefers the process env; use a name that cannot
        // exist in the environment to exercise the .env fallback path.
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join(".env"),
            "# comment\nYAHU_TEST_MANAGEMENT_KEY=\"sk-or-mgmt-1\"\n",
        )
        .unwrap();
        assert_eq!(
            provider_env_value(temp.path(), "YAHU_TEST_MANAGEMENT_KEY"),
            "sk-or-mgmt-1"
        );
        assert_eq!(provider_env_value(temp.path(), "YAHU_TEST_MISSING_KEY"), "");
    }

    #[test]
    fn auth_json_pool_reads_labels_and_tokens() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("auth.json"),
            serde_json::json!({
                "credential_pool": {
                    "commandcode": [
                        {"access_token": "tok-1", "label": "acct-a"},
                        {"api_key": "tok-2"},
                        {"token": "", "label": "empty"}
                    ]
                }
            })
            .to_string(),
        )
        .unwrap();
        let pool = auth_json_credential_pool(temp.path(), "commandcode");
        assert_eq!(pool.len(), 2);
        assert_eq!(pool[0], ("acct-a".to_string(), "tok-1".to_string()));
        assert_eq!(pool[1].0, "账号2");
        assert_eq!(auth_json_credential_pool(temp.path(), "missing"), Vec::new());
    }

    #[test]
    fn opencode_window_parses_solidjs_ssr_object() {
        let html = r#"rollingUsage:$R[12]={usagePercent:12.30,resetInSec:3600.5};weeklyUsage:$R[13]={usagePercent:88,resetInSec:60}"#;
        let rolling = opencode_window(html, "rolling").unwrap();
        assert!((rolling.0 - 12.3).abs() < 0.001);
        assert!((rolling.1 - 3600.5).abs() < 0.001);
        let weekly = opencode_window(html, "weekly").unwrap();
        assert!((weekly.0 - 88.0).abs() < 0.001);
        assert!(opencode_window(html, "monthly").is_none());
    }

    #[test]
    fn codex_backend_url_uses_wham_for_chatgpt_base() {
        assert_eq!(
            codex_backend_usage_url("https://chatgpt.com/backend-api/codex"),
            "https://chatgpt.com/backend-api/wham/usage"
        );
        assert_eq!(
            codex_backend_usage_url(""),
            "https://chatgpt.com/backend-api/wham/usage"
        );
        assert_eq!(
            codex_backend_usage_url("https://relay.example/v1/codex"),
            "https://relay.example/v1/api/codex/usage"
        );
    }

    #[test]
    fn commandcode_plan_total_matches_plan_prefixes() {
        assert_eq!(commandcode_plan_total("individual-goat"), Some(70.0));
        assert_eq!(commandcode_plan_total("INDIVIDUAL_PRO_V1"), Some(80.0));
        assert_eq!(commandcode_plan_total("unknown"), None);
    }

    #[test]
    fn jwt_account_id_extracts_chatgpt_claim() {
        // payload: {"https://api.openai.com/auth":{"chatgpt_account_id":"acc-7"}}
        let token = format!(
            "h.{}.s",
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(r#"{"https://api.openai.com/auth":{"chatgpt_account_id":"acc-7"}}"#)
        );
        assert_eq!(jwt_chatgpt_account_id(&token).as_deref(), Some("acc-7"));
        assert_eq!(jwt_chatgpt_account_id("not.a.jwt"), None);
    }

    #[test]
    fn deepseek_hit_rate_rejects_invalid_counters() {
        assert_eq!(deepseek_cache_hit_rate(75.0, 25.0).as_deref(), Some("75.0%"));
        assert_eq!(deepseek_cache_hit_rate(0.0, 0.0), None);
        assert_eq!(deepseek_cache_hit_rate(-1.0, 10.0), None);
    }

    #[test]
    fn mimo_period_and_months_cover_cycle_spanning_two_months() {
        let end = chrono::NaiveDate::from_ymd_opt(2026, 6, 27).unwrap();
        let start = end - chrono::Duration::days(30);
        let months = mimo_months_in_range(start, end);
        assert_eq!(months.len(), 2);
        assert_eq!(months[0], (2026, 5));
        assert_eq!(months[1], (2026, 6));
    }

    #[tokio::test]
    async fn provider_usage_handler_serves_cached_payload_within_ttl_without_refetching() {
        let temp = tempfile::tempdir().unwrap();
        let state = Arc::new(test_app_state("http://127.0.0.1:1".to_string(), temp.path()));
        let cached = ProviderUsagePayload {
            fetched_at: unix_now_seconds(),
            sections: vec![ProviderUsageSection {
                provider: "openrouter".into(),
                title: "OpenRouter API 用量".into(),
                description: "余额 **$9.00**".into(),
                rows: vec![ProviderUsageRow {
                    label: "gpt-test".into(),
                    hit_rate: None,
                    input: Some("100".into()),
                    output: Some("50".into()),
                    cost_or_pct: Some("$1.20".into()),
                }],
                windows: Vec::new(),
                errors: Vec::new(),
            }],
        };
        *state.provider_usage_cache.payload.write().await = Some(cached.clone());
        *state.provider_usage_cache.fetched_at.write().await = Some(Instant::now());

        // Second handler call must serve the same cached object without any
        // network access (the api url points at an unroutable port).
        let first = provider_usage_handler(
            State(state.clone()),
            Query(ProviderUsageQuery { refresh: Some(false) }),
        )
        .await;
        assert_eq!(first.status(), StatusCode::OK);

        // Force refresh bypasses the cache. The handler must not panic even
        // when some providers hold valid credentials in the environment and
        // others fail; every section renders rows/windows or an error note.
        let forced = provider_usage_handler(
            State(state.clone()),
            Query(ProviderUsageQuery { refresh: Some(true) }),
        )
        .await;
        assert_eq!(forced.status(), StatusCode::OK);
        let body = axum::body::to_bytes(forced.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let sections = json["sections"].as_array().unwrap();
        // One section per provider; the handler must never panic regardless
        // of which credentials exist in the environment.
        assert_eq!(sections.len(), 10);
    }
}
