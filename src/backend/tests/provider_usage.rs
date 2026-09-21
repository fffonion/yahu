#[cfg(test)]
mod provider_usage_tests {
    use super::*;

    #[test]
    fn provider_icon_urls_embed_newapi_variants_and_proxy_gstatic_sources() {
        assert!(NEWAPI_VARIANT_PROVIDERS.contains(&"agentrouter"));
        assert!(NEWAPI_LOGO_BYTES.starts_with(&[0x89, b'P', b'N', b'G']));
        assert_eq!(provider_icon_url("agentrouter"), None);
        assert_eq!(
            provider_icon_url("openrouter"),
            Some("https://www.google.com/s2/favicons?domain=openrouter.ai&sz=64".to_string())
        );
        assert_eq!(
            provider_icon_url("stepfun"),
            Some("https://www.google.com/s2/favicons?domain=stepfun.com&sz=64".to_string())
        );
        assert_eq!(
            provider_icon_url("custom-provider"),
            Some("https://www.google.com/s2/favicons?domain=custom-provider&sz=64".to_string())
        );
        assert!(provider_icon_url("../internal").is_none());
    }

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
    fn opencode_catalog_reuses_inference_key_env_for_usage() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join(".env"),
            "OPENCODE_GO_API_KEY=[REDACTED]\n",
        )
        .unwrap();

        let provider = provider_usage_catalog(temp.path())
            .into_iter()
            .find(|item| item.provider == "opencode")
            .unwrap();

        assert!(provider.configured);
        assert!(provider.query_ready);
        assert_eq!(provider.credential_hint, "OPENCODE_GO_API_KEY");
        assert!(provider.setup_hint.contains("Go 订阅"));
        assert!(!provider.setup_hint.contains("service account"));
        assert!(!provider.setup_hint.contains("Cookie"));
    }

    #[tokio::test]
    async fn opencode_go_usage_403_requires_a_go_plan_key() {
        async fn forbidden(
            headers: axum::http::HeaderMap,
            axum::extract::Query(params): axum::extract::Query<
                std::collections::HashMap<String, String>,
            >,
        ) -> axum::http::StatusCode {
            assert_eq!(
                headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok()),
                Some("Bearer [REDACTED]")
            );
            assert_eq!(
                headers.get("accept").and_then(|value| value.to_str().ok()),
                Some("application/json")
            );
            assert!(params.is_empty());
            axum::http::StatusCode::FORBIDDEN
        }

        let app = axum::Router::new().route(
            "/zen/go/v1/usage",
            axum::routing::get(forbidden),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join(".env"),
            "OPENCODE_GO_API_KEY=[REDACTED]\n",
        )
        .unwrap();
        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());

        let section = fetch_opencode_usage_from_url(
            &state,
            &format!("http://{addr}/zen/go/v1/usage"),
        )
        .await;

        assert!(section.windows.is_empty());
        assert_eq!(
            section.errors,
            vec!["OpenCode Go 用量查询返回 403：当前 OPENCODE_GO_API_KEY 无 Go Plan 用量权限，需要使用关联 Go 订阅、权限更高的 key。"]
        );
    }

    #[tokio::test]
    async fn opencode_go_usage_builds_three_quota_windows() {
        async fn usage(
            axum::extract::State(calls): axum::extract::State<
                std::sync::Arc<std::sync::atomic::AtomicUsize>,
            >,
            headers: axum::http::HeaderMap,
            axum::extract::Query(params): axum::extract::Query<
                std::collections::HashMap<String, String>,
            >,
        ) -> axum::Json<serde_json::Value> {
            calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            assert_eq!(
                headers
                    .get("authorization")
                    .and_then(|value| value.to_str().ok()),
                Some("Bearer [REDACTED]")
            );
            assert_eq!(
                headers.get("accept").and_then(|value| value.to_str().ok()),
                Some("application/json")
            );
            assert!(params.is_empty());
            axum::Json(serde_json::json!({
                "usage": {
                    "rolling": {
                        "status": "ok",
                        "percent": 37.5,
                        "resetsAt": "2030-01-01T01:02:03.000Z"
                    },
                    "weekly": {
                        "status": "ok",
                        "percent": 64,
                        "resetsAt": "2030-01-08T00:00:00.000Z"
                    },
                    "monthly": {
                        "status": "rate-limited",
                        "percent": 100,
                        "resetsAt": "2030-02-01T00:00:00.000Z"
                    }
                }
            }))
        }

        let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let app = axum::Router::new()
            .route("/zen/go/v1/usage", axum::routing::get(usage))
            .with_state(calls.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join(".env"),
            "OPENCODE_GO_API_KEY=[REDACTED]\n",
        )
        .unwrap();
        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());

        let section = fetch_opencode_usage_from_url(
            &state,
            &format!("http://{addr}/zen/go/v1/usage"),
        )
        .await;

        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(section.title, "OpenCode Go 用量");
        assert_eq!(section.errors, Vec::<String>::new());
        assert_eq!(section.windows.len(), 3);
        assert_eq!(section.windows[0].window, "5h额度");
        assert_eq!(section.windows[0].used.as_deref(), Some("37.5%"));
        assert_eq!(section.windows[1].window, "周额度");
        assert_eq!(section.windows[1].used.as_deref(), Some("64%"));
        assert_eq!(section.windows[2].window, "月额度");
        assert_eq!(section.windows[2].used.as_deref(), Some("100%"));
        assert_eq!(
            section.windows[0].reset_at,
            Some(
                chrono::DateTime::parse_from_rfc3339("2030-01-01T01:02:03.000Z")
                    .unwrap()
                    .timestamp()
            )
        );
    }


    #[test]
    fn codex_reset_failure_reuses_cached_description_for_each_account() {
        let cached = ProviderUsageSection {
            provider: "codex".into(),
            description: "mayo：Reset：3个；到期：2小时后；me：Reset：1个".into(),
            ..Default::default()
        };
        let labels = vec!["mayo".to_string(), "me".to_string()];
        assert_eq!(
            codex_cached_reset_description(Some(&cached), "mayo", &labels),
            Some("mayo：Reset：3个；到期：2小时后".to_string())
        );
        assert_eq!(
            codex_cached_reset_description(Some(&cached), "me", &labels),
            Some("me：Reset：1个".to_string())
        );
        assert_eq!(
            codex_cached_reset_description(Some(&cached), "other", &labels),
            None
        );
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
    fn commandcode_99_and_100_percent_windows_use_quota_wall_cache_policy() {
        let now = 1_800_000_000;
        let section: ProviderUsageSection = serde_json::from_value(serde_json::json!({
            "provider": "commandcode",
            "title": "CommandCode 额度",
            "description": "",
            "rows": [],
            "errors": [],
            "windows": [
                {"window": "acct-a 5h额度", "used": "99%", "reset": "旧值", "reset_at": now + 3600},
                {"window": "acct-b 5h额度", "used": "100%", "reset": "旧值", "reset_at": now + 3600},
                {"window": "acct-c 5h额度", "used": "20%", "reset": "旧值", "reset_at": now + 3600}
            ]
        }))
        .unwrap();

        assert!(provider_account_should_skip_upstream(&section, "acct-a", true, now));
        assert!(provider_account_should_skip_upstream(&section, "acct-b", true, now));
        assert!(!provider_account_should_skip_upstream(&section, "acct-c", true, now));
        let cached = provider_cached_account_section(&section, "acct-b", true, now).unwrap();
        assert_eq!(cached.windows[0].window, "acct-b 5h额度");
        assert_eq!(cached.windows[0].reset.as_deref(), Some("1小时"));
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
    fn commandcode_cache_plan_uses_credits_only_before_period_reset() {
        let now = 1_800_000_000;
        let cached = CommandCodeAccountCache {
            token_fingerprint: "fp".into(),
            whoami_cached: true,
            org_id: None,
            current_period_end: Some(now + 3600),
            total_cost: Some(12.5),
            ..Default::default()
        };
        assert_eq!(
            commandcode_cache_plan(Some(&cached), "fp", now),
            CommandCodeCachePlan {
                refresh_whoami: false,
                refresh_subscription: false,
                refresh_summary: false,
            }
        );
    }

    #[test]
    fn commandcode_cache_plan_refetches_period_data_after_expiry_or_token_change() {
        let now = 1_800_000_000;
        let expired = CommandCodeAccountCache {
            token_fingerprint: "fp".into(),
            whoami_cached: true,
            org_id: Some("org-1".into()),
            current_period_end: Some(now - 1),
            total_cost: Some(12.5),
            ..Default::default()
        };
        let expired_plan = commandcode_cache_plan(Some(&expired), "fp", now);
        assert!(expired_plan.refresh_subscription);
        assert!(expired_plan.refresh_summary);
        assert!(!expired_plan.refresh_whoami);

        let token_changed = commandcode_cache_plan(Some(&expired), "new-fp", now);
        assert!(token_changed.refresh_whoami);
        assert!(token_changed.refresh_subscription);
        assert!(token_changed.refresh_summary);
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
        assert_eq!(providers.len(), provider_usage_catalog(temp.path()).len());
    }

    #[test]
    fn commandcode_billing_requests_share_one_parallel_stage() {
        let source = include_str!("../provider_usage.rs");
        assert!(source.contains("let (credits_result, subscription_result) = tokio::join!("));
    }

    #[test]
    fn zed_session_cookie_header_accepts_value_or_cookie_header() {
        assert_eq!(
            zed_session_cookie_header("[REDACTED]"),
            Some("zed.session=[REDACTED]".into())
        );
        assert_eq!(
            zed_session_cookie_header("zed.session=[REDACTED]"),
            Some("zed.session=[REDACTED]".into())
        );
        assert_eq!(
            zed_session_cookie_header("Cookie: zed.session=[REDACTED]; __cf_bm=[REDACTED]"),
            Some("zed.session=[REDACTED]; __cf_bm=[REDACTED]".into())
        );
    }

    #[test]
    fn zed_billing_snapshot_parses_student_token_spend() {
        let payload = serde_json::json!({
            "plan": "token_based_zed_student",
            "current_usage": {
                "token_spend": {
                    "spend_in_cents": 7,
                    "limit_in_cents": 1000
                }
            },
            "portal_url": "https://example.test/portal?token=[REDACTED]"
        });
        let (description, window) = zed_billing_snapshot(&payload).unwrap();
        assert_eq!(
            description,
            "Student · 余额 **$0.07/$10.00** · 超额：否"
        );
        let window = window.unwrap();
        assert_eq!(window.window, "Hosted AI 月额度");
        assert_eq!(window.used.as_deref(), Some("0.7%"));
        assert!(window.reset.is_none());
    }

    #[test]
    fn zed_billing_snapshot_marks_overage() {
        let payload = serde_json::json!({
            "plan": "token_based_zed_student",
            "current_usage": {
                "token_spend_in_cents": 1200,
                "token_spend_limit_in_cents": 1000
            }
        });
        let (description, window) = zed_billing_snapshot(&payload).unwrap();
        assert!(description.contains("超额：是"));
        assert_eq!(window.unwrap().used.as_deref(), Some("120.0%"));
    }

    #[test]
    fn zed_billing_description_replaces_stale_balance() {
        let merged = zed_merge_billing_description(
            "Student · 余额 **$0.06/$10.00** · 账期 9/1–10/1",
            "Student · 余额 **$0.07/$10.00** · 超额：否",
        );
        assert_eq!(
            merged,
            "Student · 余额 **$0.07/$10.00** · 超额：否 · 账期 9/1–10/1"
        );
    }

    #[test]
    fn agentrouter_catalog_requires_newapi_user_and_session_cookie() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join(".env"),
            "AGENTROUTER_SESSION_COOKIE=Cookie: session=[REDACTED]\nAGENTROUTER_USER_ID=641019\n",
        )
        .unwrap();
        let provider = provider_usage_catalog(temp.path())
            .into_iter()
            .find(|item| item.provider == "agentrouter")
            .unwrap();
        assert!(provider.configured);
        assert!(provider.query_ready);
        assert_eq!(provider.title, "AgentRouter 用量");
    }

    #[test]
    fn openrouter_catalog_title_omits_api_suffix() {
        let temp = tempfile::tempdir().unwrap();
        let provider = provider_usage_catalog(temp.path())
            .into_iter()
            .find(|item| item.provider == "openrouter")
            .unwrap();
        assert_eq!(provider.title, "OpenRouter 用量");
    }

    #[test]
    fn agentrouter_usage_rows_convert_newapi_quota_to_dollars() {
        let records = vec![
            NewApiUsageRecord {
                model_name: "gpt-a".into(),
                count: 5.0,
                quota: 4_000.0,
                token_used: 300.0,
                created_at: chrono::Utc::now().timestamp() - 2 * 24 * 60 * 60,
            },
        ];
        let section = agentrouter_usage_section(
            &records,
            500_000.0,
            62_004_855.0,
            495_145.0,
            chrono::Utc::now(),
        );
        assert_eq!(section.provider, "agentrouter");
        assert_eq!(section.rows[0].input.as_deref(), Some("300"));
        assert_eq!(section.rows[0].output.as_deref(), Some("5"));
        assert_eq!(section.rows[0].cost_or_pct.as_deref(), Some("$0.01"));
        assert_eq!(section.description, "余额 **$124.01**；累计已用 **$0.99**");
        assert_eq!(section.windows[0].window, "今日用量/费用");
        assert_eq!(section.windows[0].used.as_deref(), Some("$0.00"));
    }

    #[test]
    fn agentrouter_quota_per_unit_cache_survives_reload() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());
        save_newapi_quota_per_unit(&state, "agentrouter", 500_000.0, 1_800_000_000.0);
        assert_eq!(
            cached_newapi_quota_per_unit(&state, "agentrouter", 1_800_000_001.0),
            Some(500_000.0)
        );
    }

    #[test]
    fn stepfun_catalog_requires_web_cookie_for_query_ready() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join(".env"),
            "STEPFUN_API_KEY=[REDACTED]\n",
        )
        .unwrap();
        let process_cookie = first_provider_env_value(
            std::path::Path::new("/path/that/does/not/exist"),
            &["STEPFUN_COOKIE", "STEPFUN_WEB_COOKIE"],
        );
        let provider = provider_usage_catalog(temp.path())
            .into_iter()
            .find(|item| item.provider == "stepfun")
            .unwrap();
        assert!(provider.configured);
        assert_eq!(provider.query_ready, !process_cookie.is_empty());
        assert!(provider.setup_hint.contains("网页 Cookie"));
    }

    #[test]
    fn stepfun_cookie_webid_is_extracted_without_normalizing_the_cookie() {
        let cookie = "INGRESSCOOKIE=opaque; Oasis-Webid=[REDACTED]; Oasis-Token=[REDACTED]";
        assert_eq!(stepfun_cookie_value(cookie, "Oasis-Webid"), Some("[REDACTED]".into()));
        assert_eq!(stepfun_cookie_value(cookie, "Oasis-Token"), Some("[REDACTED]".into()));
        assert_eq!(stepfun_cookie_value(cookie, "Missing"), None);
    }

    #[test]
    fn stepfun_request_headers_match_browser_rpc_requirements() {
        let headers = stepfun_request_headers_for_token(
            "Oasis-Webid=web-id; Oasis-Token=[REDACTED]",
            "[REDACTED]",
        );
        let names = headers
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<std::collections::HashSet<_>>();
        for name in [
            "accept",
            "accept-language",
            "cache-control",
            "connect-protocol-version",
            "content-type",
            "cookie",
            "dnt",
            "oasis-appid",
            "oasis-platform",
            "oasis-webid",
            "origin",
            "pragma",
            "priority",
            "referer",
            "sec-ch-ua",
            "sec-ch-ua-mobile",
            "sec-ch-ua-platform",
            "sec-fetch-dest",
            "sec-fetch-mode",
            "sec-fetch-site",
            "user-agent",
        ] {
            assert!(names.contains(name), "missing StepFun header: {name}");
        }
        assert!(headers.iter().any(|(name, value)| {
            name == "cookie"
                && value.contains("Oasis-Webid=web-id")
                && value.contains("Oasis-Token=[REDACTED]")
        }));
        assert!(headers
            .iter()
            .any(|(name, value)| name == "oasis-webid" && value == "web-id"));
        assert!(headers
            .iter()
            .any(|(name, value)| name == "oasis-token" && value == "[REDACTED]"));
    }

    #[test]
    fn stepfun_browser_token_uses_refresh_device_id_and_auth_headers() {
        use base64::Engine as _;

        let jwt = |device_id: &str| {
            let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
                serde_json::json!({"device_id": device_id}).to_string(),
            );
            format!("header.{payload}.signature")
        };
        let access = jwt("access-device");
        let refresh = jwt("refresh-device");
        let token = format!("{access}...{refresh}");
        let headers = stepfun_request_headers_for_token(
            "INGRESSCOOKIE=opaque; Oasis-Webid=stale-device; other=value",
            &token,
        );

        assert_eq!(stepfun_token_device_id(&token), Some("refresh-device".into()));
        assert!(headers.iter().any(|(name, value)| {
            name == "oasis-webid" && value == "refresh-device"
        }));
        assert!(headers.iter().any(|(name, value)| {
            name == "oasis-token" && value == &token
        }));
        let cookie = headers
            .iter()
            .find(|(name, _)| name == "cookie")
            .map(|(_, value)| value)
            .unwrap();
        assert!(cookie.contains("Oasis-Webid=refresh-device"));
        assert!(cookie.contains(&format!("Oasis-Token={token}")));
        assert!(!cookie.contains("Oasis-Webid=stale-device"));
    }

    #[test]
    fn stepfun_refresh_response_rebuilds_access_refresh_pair() {
        let response = serde_json::json!({
            "status": 1,
            "accessToken": {"raw": "[REDACTED]"},
            "refreshToken": {"raw": "[REDACTED]"}
        });
        let expected = "[REDACTED]...[REDACTED]";
        assert_eq!(stepfun_combined_token_from_response(&response).unwrap(), expected);
    }
    #[test]
    fn stepfun_usage_section_shows_today_summary_and_plan_percent() {
        let now = chrono::DateTime::parse_from_rfc3339("2026-09-20T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let today_from = chrono::DateTime::parse_from_rfc3339("2026-09-20T01:00:00Z")
            .unwrap()
            .timestamp_millis();
        let today_to = chrono::DateTime::parse_from_rfc3339("2026-09-20T02:00:00Z")
            .unwrap()
            .timestamp_millis();
        let yesterday = chrono::DateTime::parse_from_rfc3339("2026-09-19T01:00:00Z")
            .unwrap()
            .timestamp_millis();
        let response = serde_json::json!({
            "status": 1,
            "total": 2,
            "records": [
                {"fromTime": today_from, "toTime": today_to, "modelId": "step-5-preview", "calls": 3, "creditConsumed": 2_000_000, "modelType": 1},
                {"fromTime": yesterday, "toTime": yesterday + 3_600_000_i64, "modelId": "step-5-preview", "calls": 7, "creditConsumed": 9_000_000, "modelType": 1}
            ]
        });
        let plan_response = serde_json::json!({
            "status": 1,
            "plan_credit_rate_limit": {
                "subscription_credit_left_rate": 0.976,
                "subscription_credit_reset_time": "2026-09-30T16:00:00Z"
            }
        });
        let (records, total) = stepfun_response_records(&response).unwrap();
        let plan = stepfun_plan_rate_limit_response(&plan_response)
            .unwrap()
            .unwrap();
        let section = stepfun_usage_section_from_records_at(&records, total, now, Some(plan));
        assert_eq!(section.description, "今日 2M · 调用次数 3");
        assert_eq!(section.windows.len(), 1);
        assert_eq!(section.windows[0].window, "Plan 已用");
        assert_eq!(section.windows[0].used.as_deref(), Some("2%"));
        assert!(section.windows[0].reset_at.is_some());
        assert_eq!(section.rows[0].label, "step-5-preview");
        assert_eq!(section.rows[0].input.as_deref(), Some("10"));
    }

    #[test]
    fn stepfun_plan_rate_limit_falls_back_to_credit_buckets() {
        let response = serde_json::json!({
            "status": 1,
            "planCreditRateLimit": {
                "creditBuckets": [{"creditTotal": "1000000", "creditResidual": "750000"}]
            }
        });
        let plan = stepfun_plan_rate_limit_response(&response)
            .unwrap()
            .unwrap();
        assert_eq!(plan.used_percent, 25.0);
    }

    #[test]
    fn stepfun_usage_response_aggregates_calls_credit_and_time_ranges() {
        let response = serde_json::json!({
            "status": 1,
            "desc": "ok",
            "total": 3,
            "records": [
                {"fromTime": "1700000000000", "toTime": "1700003600000", "modelId": "step-1", "calls": "3", "creditConsumed": "1.25", "modelType": 1},
                {"fromTime": 1700003600000_i64, "toTime": 1700007200000_i64, "modelId": "step-1", "calls": 2, "creditConsumed": 0.75, "modelType": 1},
                {"fromTime": 1700000000000_i64, "toTime": 1700007200000_i64, "modelId": "step-2", "calls": 1, "creditConsumed": 9.5, "modelType": 2}
            ]
        });
        let (records, total) = stepfun_response_records(&response).unwrap();
        let section = stepfun_usage_section_from_records_at(
            &records,
            total,
            chrono::Utc::now(),
            None,
        );
        assert_eq!(section.provider, "stepfun");
        assert_eq!(section.description, "今日 0M · 调用次数 0");
        assert!(section.windows.is_empty());
        assert_eq!(section.rows[0].label, "step-2");
        assert_eq!(section.rows[0].input.as_deref(), Some("1"));
        assert_eq!(section.rows[0].output.as_deref(), Some("9.50"));
        assert_eq!(section.rows[0].cost_or_pct.as_deref(), Some("2"));
        assert_eq!(section.rows[1].input.as_deref(), Some("5"));
        assert_eq!(section.rows[1].output.as_deref(), Some("2.00"));
        assert!(section.rows[1].hit_rate.as_deref().unwrap().contains("11-14"));
    }

    #[test]
    fn stepfun_nonzero_status_is_reported_without_exposing_response_data() {
        let response = serde_json::json!({"status": 7, "desc": "session expired", "records": []});
        let error = stepfun_response_records(&response).unwrap_err();
        assert_eq!(error, "session expired");
    }
}
