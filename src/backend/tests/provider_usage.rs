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
            "# comment\nYAHU_TEST_MANAGEMENT_KEY=\"[REDACTED]\"\n",
        )
        .unwrap();
        assert_eq!(
            provider_env_value(temp.path(), "YAHU_TEST_MANAGEMENT_KEY"),
            "[REDACTED]"
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
                        {"access_token": "[REDACTED]", "label": "acct-a"},
                        {"api_key": "[REDACTED]"},
                        {"token": "", "label": "empty"}
                    ]
                }
            })
            .to_string(),
        )
        .unwrap();
        let pool = auth_json_credential_pool(temp.path(), "commandcode");
        assert_eq!(pool.len(), 2);
        assert_eq!(pool[0], ("acct-a".to_string(), "[REDACTED]".to_string()));
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
    fn opencode_urls_include_plan_fallback_and_honor_override() {
        assert_eq!(
            opencode_dashboard_urls("workspace-x", ""),
            vec![
                "https://opencode.ai/workspace/workspace-x/usage",
                "https://opencode.ai/workspace/workspace-x/go",
            ]
        );
        assert_eq!(
            opencode_dashboard_urls("workspace-x", "https://example.test/custom"),
            vec!["https://example.test/custom"]
        );
    }

    #[test]
    fn opencode_percent_text_does_not_duplicate_percent_sign() {
        assert_eq!(opencode_percent_text(12.3), "12.3%");
        assert_eq!(opencode_percent_text(88.0), "88%");
    }

    #[test]
    fn opencode_empty_usage_page_is_detected_without_error() {
        let html = "<p>No usage data available for the selected period.</p>";
        assert!(opencode_has_no_usage_data(html));
        assert!(!opencode_has_no_usage_data("rollingUsage:$R[1]={usagePercent:1,resetInSec:2}"));
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
    fn codex_reset_credits_describe_count_and_relative_expiries() {
        let now = chrono::Utc::now();
        let payload = serde_json::json!({
            "available_count": 3,
            "applicable_available_count": 2,
            "credits": [
                {
                    "status": "available",
                    "expires_at": (now + chrono::Duration::minutes(90)).to_rfc3339()
                },
                {
                    "status": "available",
                    "expires_at": (now + chrono::Duration::days(3)).to_rfc3339()
                },
                {"status": "consumed", "expires_at": (now + chrono::Duration::days(1)).to_rfc3339()}
            ]
        });
        assert_eq!(
            codex_reset_credits_description(&payload),
            Some("Reset：3个；当前可用：2个；到期：2小时后、3天后".to_string())
        );
    }

    #[test]
    fn saturated_codex_account_uses_cached_windows_until_reset() {
        let now = chrono::Utc::now().timestamp();
        let section: ProviderUsageSection = serde_json::from_value(serde_json::json!({
            "provider": "codex",
            "title": "Codex 额度",
            "description": "",
            "rows": [],
            "errors": [],
            "windows": [
                {"window": "mayo 5h额度", "used": "100%", "reset": "旧值", "reset_at": now + 3600},
                {"window": "mayo 周额度", "used": "20%", "reset": "旧值", "reset_at": now + 7200},
                {"window": "me 周额度", "used": "100%", "reset": "旧值", "reset_at": now + 7200}
            ]
        }))
        .unwrap();
        assert!(provider_account_should_skip_upstream(&section, "mayo", true, now));
        assert!(!provider_account_should_skip_upstream(&section, "mayo", true, now + 3601));
        assert!(!provider_account_should_skip_upstream(&section, "missing", true, now));
    }

    #[test]
    fn cached_codex_account_refresh_updates_each_window_countdown() {
        let now = chrono::Utc::now().timestamp();
        let section: ProviderUsageSection = serde_json::from_value(serde_json::json!({
            "provider": "codex",
            "title": "Codex 额度",
            "description": "",
            "rows": [],
            "errors": [],
            "windows": [
                {"window": "mayo 5h额度", "used": "100%", "reset": "旧值", "reset_at": now + 3600},
                {"window": "mayo 周额度", "used": "100%", "reset": "旧值", "reset_at": now + 7200}
            ]
        }))
        .unwrap();
        let refreshed = provider_cached_account_section(&section, "mayo", true, now).unwrap();
        assert_eq!(refreshed.windows[0].reset.as_deref(), Some("1小时"));
        assert_eq!(refreshed.windows[1].reset.as_deref(), Some("2小时"));
    }

    #[test]
    fn generic_provider_saturated_window_refreshes_cached_reset_times() {
        let now = chrono::Utc::now().timestamp();
        let section: ProviderUsageSection = serde_json::from_value(serde_json::json!({
            "provider": "kimi",
            "title": "Kimi Code 额度",
            "description": "",
            "rows": [],
            "errors": [],
            "windows": [
                {"window": "5h额度", "used": "100%", "reset": "旧值", "reset_at": now + 3600},
                {"window": "周额度", "used": "20%", "reset": "旧值", "reset_at": now + 7200}
            ]
        }))
        .unwrap();
        assert!(provider_section_should_skip_upstream(&section, now));
        let refreshed = provider_cached_section(&section, now).unwrap();
        assert_eq!(refreshed.windows[0].reset.as_deref(), Some("1小时"));
        assert_eq!(refreshed.windows[1].reset.as_deref(), Some("2小时"));
        assert!(provider_cached_section(&section, now + 3601).is_none());
    }

    #[test]
    fn generic_provider_multi_account_skip_is_limited_to_the_saturated_account() {
        let now = chrono::Utc::now().timestamp();
        let section: ProviderUsageSection = serde_json::from_value(serde_json::json!({
            "provider": "commandcode",
            "title": "CommandCode 额度",
            "description": "",
            "rows": [],
            "errors": [],
            "windows": [
                {"window": "acct-a 5h额度", "used": "100%", "reset": "旧值", "reset_at": now + 3600},
                {"window": "acct-b 5h额度", "used": "100%", "reset": "旧值", "reset_at": now + 3600}
            ]
        }))
        .unwrap();
        assert!(provider_account_should_skip_upstream(&section, "acct-a", true, now));
        assert!(provider_account_should_skip_upstream(&section, "acct-b", true, now));
        let cached = provider_cached_account_section(&section, "acct-a", true, now).unwrap();
        assert_eq!(cached.windows.len(), 1);
        assert_eq!(cached.windows[0].window, "acct-a 5h额度");
        assert!(provider_cached_account_section(&section, "missing", true, now).is_none());
    }

    #[test]
    fn provider_reset_at_normalizes_millisecond_and_rfc3339_values() {
        let seconds = chrono::Utc::now().timestamp() + 3600;
        let millis = seconds * 1000;
        assert_eq!(provider_reset_at(&serde_json::json!({"reset": millis}), "reset"), Some(seconds));
        let rfc3339 = chrono::DateTime::from_timestamp(seconds, 0).unwrap().to_rfc3339();
        assert_eq!(provider_reset_at(&serde_json::json!({"reset": rfc3339}), "reset"), Some(seconds));
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
    fn provider_number_accepts_json_numbers_and_numeric_strings() {
        assert_eq!(provider_number(&serde_json::json!(12.5)), Some(12.5));
        assert_eq!(provider_number(&serde_json::json!("76.5465807600000000")), Some(76.54658076));
        assert_eq!(provider_number(&serde_json::json!("")), None);
        assert_eq!(provider_number(&serde_json::json!(true)), None);
    }

    #[test]
    fn deepseek_rows_keep_script_models_and_parse_string_counters() {
        let mut totals = HashMap::new();
        totals.insert(
            "deepseek-v4-pro".into(),
            DeepseekUsageCounters {
                cache_hit: 75.0,
                cache_miss: 25.0,
                response: 50.0,
                cost: 1.25,
            },
        );
        let rows = deepseek_rows(&totals);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "pro");
        assert_eq!(rows[0].input.as_deref(), Some("100"));
        assert_eq!(rows[0].output.as_deref(), Some("50"));
    }

    #[test]
    fn minimax_percent_fallback_handles_zero_count_quotas() {
        let model = serde_json::json!({
            "model_name": "general",
            "current_interval_total_count": 0,
            "current_weekly_total_count": 0,
            "current_interval_remaining_percent": 90,
            "current_weekly_remaining_percent": 86,
            "weekly_boost_permille": 1500,
        });
        assert_eq!(minimax_used_percent(&model, "interval"), Some(10.0));
        assert_eq!(minimax_used_percent(&model, "weekly"), Some(21.0));
    }

    #[test]
    fn minimax_summary_metrics_calculate_day_week_month_tokens() {
        let summary = serde_json::json!({
            "daily_token_usage": [10, 20, 30],
            "most_active_day": {"date": "2026-01-03", "token_count": 30}
        });
        let metrics = minimax_summary_metrics(
            &summary,
            chrono::NaiveDate::from_ymd_opt(2026, 1, 3).unwrap(),
        );
        assert_eq!(metrics, vec![("日".into(), 30.0), ("周".into(), 60.0), ("月".into(), 60.0)]);
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
            providers: Vec::new(),
            sections: vec![ProviderUsageSection {
                provider: "openrouter".into(),
                title: "OpenRouter API 用量".into(),
                description: "余额 **$9.00**".into(),
                captured_at: 1_700_000_000.0,
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
            Query(ProviderUsageQuery {
                refresh: Some(false),
                provider: None,
            }),
        )
        .await;
        assert_eq!(first.status(), StatusCode::OK);

        // Force refresh bypasses the cache. The handler must not panic even
        // when some providers hold valid credentials in the environment and
        // others fail; every section renders rows/windows or an error note.
        let forced = provider_usage_handler(
            State(state.clone()),
            Query(ProviderUsageQuery {
                refresh: Some(true),
                provider: None,
            }),
        )
        .await;
        assert_eq!(forced.status(), StatusCode::OK);
        let body = axum::body::to_bytes(forced.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: Value = serde_json::from_slice(&body).unwrap();
        let sections = json["sections"].as_array().unwrap();
        let providers = json["providers"].as_array().unwrap();
        assert_eq!(sections.len(), 1);
        assert_eq!(sections[0]["provider"], "openrouter");
        assert_eq!(providers.len(), 10);
    }
}
