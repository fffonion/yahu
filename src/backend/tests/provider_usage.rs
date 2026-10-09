#[cfg(test)]
mod provider_usage_tests {
    use super::*;

    #[test]
    fn provider_icon_urls_embed_newapi_variants_and_proxy_gstatic_sources() {
        assert!(NEWAPI_VARIANT_PROVIDERS.contains(&"agentrouter"));
        assert!(NEWAPI_VARIANT_PROVIDERS.contains(&"justwoker"));
        assert_eq!(provider_icon_url("justwoker"), None);
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
    fn vyceai_catalog_requires_browser_session_not_inference_key() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("config.yaml"),
            "custom_providers:\n  - name: vyceai\n    api_key: '[REDACTED]'\n",
        )
        .unwrap();
        let meta = provider_usage_catalog(temp.path())
            .into_iter()
            .find(|item| item.provider == "vyceai")
            .expect("Vyce AI usage card");
        assert!(meta.configured);
        assert!(!meta.query_ready);
        assert_eq!(meta.credential_hint, "VYCEAI_COOKIE");
        for (cookie, ready) in [
            ("cf_clearance=[REDACTED]", false),
            ("session=", false),
            ("session=[REDACTED]", true),
        ] {
            std::fs::write(temp.path().join(".env"), format!("VYCEAI_COOKIE='{cookie}'\n")).unwrap();
            let meta = provider_usage_catalog(temp.path())
                .into_iter()
                .find(|item| item.provider == "vyceai")
                .unwrap();
            assert_eq!(meta.query_ready, ready);
        }
        assert_eq!(
            provider_icon_url("vyceai"),
            Some("https://www.google.com/s2/favicons?domain=vyceai.com&sz=64".into())
        );
    }

    #[tokio::test]
    async fn vyceai_dashboard_uses_session_and_daily_deltas() {
        async fn dashboard(
            axum::extract::State(calls): axum::extract::State<Arc<std::sync::atomic::AtomicUsize>>,
            headers: axum::http::HeaderMap,
        ) -> axum::Json<Value> {
            let n = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as f64;
            assert!(headers["authorization"] == "Bearer [REDACTED]", "mock session header mismatch");
            assert_eq!(headers["accept"], "application/json");
            assert_eq!(headers["referer"], "https://vyceai.com/dashboard-v2");
            assert!(!headers.contains_key("cookie"));
            axum::Json(serde_json::json!({
                "user": {"id": "account-a", "email": "[REDACTED]", "totalBalance": 999},
                "keys": [{"key": "[REDACTED]", "totalSpent": 999}],
                "stats": {
                    "availableBalance": 12.34 + n * 10.0, "totalBalance": 99,
                    "totalSpent": 4.626 + n * 4.625, "totalRequests": 42.0 + n * 3.0,
                    "modelUsage": {
                        "deepseek-v4.1": {"inputTokens": 1200000.0 + n * 200.0, "outputTokens": (23000.0 + n * 20.0).to_string(), "cost": 1.125 + n * 1.125},
                        "grok-imagine-2": {"inputTokens": 0, "outputTokens": 0, "cost": (3.5 + n * 3.5).to_string()},
                        "free-model": {"inputTokens": 100.0 + n * 10.0, "outputTokens": 10.0 + n, "cost": 0}
                    }
                }
            }))
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route("/user/dashboard", axum::routing::get(dashboard))
            .with_state(Arc::new(std::sync::atomic::AtomicUsize::new(0)));
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join(".env"), "VYCEAI_COOKIE='session=[REDACTED]; cf_clearance=[REDACTED]'\n").unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let section = fetch_vyceai_usage_from_url(&state, &format!("http://{addr}/user/dashboard")).await;
        assert!(section.errors.is_empty());
        assert!(section.rows.is_empty());
        assert_eq!(section.windows[0].used.as_deref(), Some("0 / 0 / $0.00"));
        let section = fetch_vyceai_usage_from_url(&state, &format!("http://{addr}/user/dashboard")).await;
        assert_eq!(section.provider, "vyceai");
        assert!(section.errors.is_empty(), "{:?}", section.errors);
        assert_eq!(section.windows.len(), 1);
        assert_eq!(section.rows.len(), 3);
        assert_eq!(section.rows[0].label, "grok-imagine-2");
        assert_eq!(section.rows[0].input.as_deref(), Some("0"));
        assert_eq!(section.rows[0].cost_or_pct.as_deref(), Some("$3.50"));
        assert_eq!(section.rows[1].input.as_deref(), Some("200"));
        assert_eq!(section.rows[1].output.as_deref(), Some("20"));
        assert!(section.rows.iter().all(|row| row.hit_rate.is_none()));
        assert!(section.description.contains("余额 **$22.34**"));
        assert!(section.description.contains("今日调用次数 3"));
        assert_eq!(section.windows[0].used.as_deref(), Some("210 / 21 / $4.62"));
        let serialized = serde_json::to_string(&section).unwrap();
        assert!(!serialized.contains("[REDACTED]"));
        assert!(!serialized.contains("999"));
        assert!(serialized.contains("今日用量（差值）"));
    }

    #[tokio::test]
    async fn vyceai_failures_never_expose_dashboard_bodies() {
        let app = axum::Router::new()
            .route("/unauthorized", axum::routing::get(|| async {
                (axum::http::StatusCode::UNAUTHORIZED, "[REDACTED]")
            }))
            .route("/blocked", axum::routing::get(|| async {
                (axum::http::StatusCode::FORBIDDEN, "[REDACTED]")
            }))
            .route("/html", axum::routing::get(|| async { "<html>[REDACTED]</html>" }))
            .route("/invalid", axum::routing::get(|| async {
                axum::Json(serde_json::json!({"error": "[REDACTED]", "keys": [{"key": "[REDACTED]"}]}))
            }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let missing = fetch_provider_usage_section(&state, "vyceai", None, true).await;
        assert_eq!(missing.errors, ["VYCEAI_COOKIE 缺少 session"]);
        std::fs::write(temp.path().join(".env"), "VYCEAI_COOKIE='session=[REDACTED]'\n").unwrap();
        for (path, expected) in [
            ("unauthorized", "Vyce AI dashboard 返回 HTTP 401"),
            ("blocked", "Vyce AI dashboard 返回 HTTP 403"),
            ("html", "Vyce AI dashboard 未返回有效 JSON"),
            ("invalid", "Vyce AI dashboard 缺少有效的账户用量字段"),
        ] {
            let section = fetch_vyceai_usage_from_url(&state, &format!("http://{addr}/{path}")).await;
            assert_eq!(section.errors, [expected]);
            assert!(section.rows.is_empty());
            assert!(!serde_json::to_string(&section).unwrap().contains("[REDACTED]"));
        }
    }

    #[test]
    fn vyceai_dashboard_handles_empty_and_invalid_model_usage() {
        let mut payload = serde_json::json!({"stats": {
            "availableBalance": 0, "totalSpent": 0, "totalRequests": 0, "modelUsage": {}
        }});
        let empty = vyceai_usage_section(&payload);
        assert!(empty.errors.is_empty());
        assert!(empty.rows.is_empty());
        assert!(empty.description.contains("$0.00"));
        payload["stats"]["modelUsage"] = serde_json::json!({
            "negative": {"inputTokens": -1, "outputTokens": 0, "cost": 0},
            "missing": {"inputTokens": 1, "outputTokens": 2},
            "invalid": {"inputTokens": 1, "outputTokens": 2, "cost": "NaN"},
            "valid": {"inputTokens": 1, "outputTokens": 2, "cost": 0.001}
        });
        let partial = vyceai_usage_section(&payload);
        assert_eq!(partial.rows.len(), 1);
        assert_eq!(partial.rows[0].label, "valid");
        assert_eq!(partial.rows[0].cost_or_pct.as_deref(), Some("$0.0010"));
        assert_eq!(partial.errors, ["Vyce AI dashboard 存在无效的模型用量字段"]);
    }

    #[test]
    fn vyceai_small_nonzero_cost_is_not_displayed_as_free() {
        let payload = serde_json::json!({"stats": {
            "availableBalance": 0, "totalSpent": 0.00003985, "totalRequests": 1,
            "modelUsage": {"agnes-3.0-flash": {"inputTokens": 10, "outputTokens": 1, "cost": 0.00003985}}
        }});
        let section = vyceai_usage_section(&payload);
        assert_eq!(section.rows[0].cost_or_pct.as_deref(), Some("$0.00003985"));
    }

    #[test]
    fn vyceai_cumulative_summary_uses_compact_text() {
        let section = vyceai_usage_section(&serde_json::json!({"stats": {
            "availableBalance": 192.19, "totalSpent": 76.82, "totalRequests": 14965,
            "modelUsage": {}
        }}));
        assert_eq!(section.description, "余额 **$192.19**；累计费用 $76.82 · 调用次数 14965（累计）");
    }

    #[tokio::test]
    #[ignore = "requires VYCEAI_COOKIE in the request process"]
    async fn vyceai_live_dashboard_smoke() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let section = fetch_provider_usage_section(&state, "vyceai", None, true).await;
        assert!(section.errors.is_empty(), "{:?}", section.errors);
        assert!(!section.description.is_empty());
        assert!(section.captured_at > 0.0);
        println!("{}", serde_json::to_string(&section).unwrap());
    }

    fn vyceai_daily_test_payload(input: f64, output: f64, cost: f64, calls: f64, account: &str) -> Value {
        serde_json::json!({
            "user": {"id": account, "email": "[REDACTED]"},
            "keys": [{"key": "[REDACTED]"}],
            "stats": {
                "availableBalance": 12.34, "totalSpent": cost, "totalRequests": calls,
                "modelUsage": {"deepseek-v4.1": {"inputTokens": input, "outputTokens": output, "cost": cost}}
            }
        })
    }

    #[test]
    fn vyceai_daily_deltas_persist_and_do_not_replay_unchanged_counters() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let now = chrono::DateTime::parse_from_rfc3339("2030-01-02T02:00:00Z").unwrap().timestamp();
        let baseline = vyceai_daily_usage_section(&state, &vyceai_daily_test_payload(1000.0, 100.0, 10.0, 40.0, "account-a"), now);
        assert!(baseline.errors.is_empty());
        assert!(baseline.rows.is_empty());
        assert_eq!(baseline.windows[0].used.as_deref(), Some("0 / 0 / $0.00"));
        let current = vyceai_daily_test_payload(1200.0, 120.0, 12.0, 43.0, "account-a");
        let delta = vyceai_daily_usage_section(&state, &current, now + 300);
        assert!(delta.errors.is_empty());
        assert_eq!(delta.rows[0].input.as_deref(), Some("200"));
        assert_eq!(delta.rows[0].output.as_deref(), Some("20"));
        assert_eq!(delta.rows[0].cost_or_pct.as_deref(), Some("$2.00"));
        assert_eq!(delta.windows[0].used.as_deref(), Some("200 / 20 / $2.00"));
        assert!(delta.description.contains("今日调用次数 3"));
        assert!(delta.description.contains("10:00"));
        let restarted = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let repeated = vyceai_daily_usage_section(&restarted, &current, now + 600);
        assert_eq!(repeated.windows[0].used, delta.windows[0].used);
        assert!(repeated.description.contains("今日调用次数 3"));
        let persisted = std::fs::read_to_string(temp.path().join("state/vyceai-usage-snapshot.json")).unwrap();
        assert!(!persisted.contains("[REDACTED]"));
        assert!(!persisted.contains("keys"));
        assert!(!persisted.contains("email"));
    }

    #[test]
    fn vyceai_daily_rollover_and_account_switch_establish_new_baselines() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let before = chrono::DateTime::parse_from_rfc3339("2030-01-02T15:59:00Z").unwrap().timestamp();
        let after = chrono::DateTime::parse_from_rfc3339("2030-01-02T16:01:00Z").unwrap().timestamp();
        vyceai_daily_usage_section(&state, &vyceai_daily_test_payload(1000.0, 100.0, 10.0, 40.0, "account-a"), before);
        let rolled = vyceai_daily_usage_section(&state, &vyceai_daily_test_payload(2000.0, 200.0, 20.0, 80.0, "account-a"), after);
        assert_eq!(rolled.windows[0].used.as_deref(), Some("0 / 0 / $0.00"));
        assert!(rolled.description.contains("00:01"));
        let grown = vyceai_daily_usage_section(&state, &vyceai_daily_test_payload(2010.0, 201.0, 20.5, 81.0, "account-a"), after + 300);
        assert_eq!(grown.windows[0].used.as_deref(), Some("10 / 1 / $0.50"));
        let switched = vyceai_daily_usage_section(&state, &vyceai_daily_test_payload(9000.0, 900.0, 90.0, 900.0, "account-b"), after + 600);
        assert_eq!(switched.windows[0].used.as_deref(), Some("0 / 0 / $0.00"));
        let grown = vyceai_daily_usage_section(&state, &vyceai_daily_test_payload(9020.0, 902.0, 91.0, 902.0, "account-b"), after + 900);
        assert_eq!(grown.windows[0].used.as_deref(), Some("20 / 2 / $1.00"));
        assert!(grown.description.contains("今日调用次数 2"));
    }

    #[test]
    fn vyceai_counter_regressions_and_missing_models_never_replay_usage() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let now = chrono::DateTime::parse_from_rfc3339("2030-01-02T02:00:00Z").unwrap().timestamp();
        for (i, input) in [1000.0, 1100.0, 900.0, 1100.0].iter().enumerate() {
            let section = vyceai_daily_usage_section(&state, &vyceai_daily_test_payload(*input, 100.0, 10.0, 40.0, "account-a"), now + i as i64);
            if i > 0 { assert_eq!(section.rows[0].input.as_deref(), Some("100")); }
        }
        let mut payload = vyceai_daily_test_payload(1100.0, 100.0, 10.0, 40.0, "account-a");
        payload["stats"]["modelUsage"]["new-image"] = serde_json::json!({"inputTokens": 0, "outputTokens": 0, "cost": 2.0});
        let added = vyceai_daily_usage_section(&state, &payload, now + 4);
        assert_eq!(added.rows.len(), 2);
        assert_eq!(added.rows[0].label, "new-image");
        payload["stats"]["modelUsage"] = serde_json::json!({});
        let missing = vyceai_daily_usage_section(&state, &payload, now + 5);
        assert_eq!(missing.rows.len(), 2);
        payload["stats"]["modelUsage"]["new-image"] = serde_json::json!({"inputTokens": 0, "outputTokens": 0, "cost": 2.0});
        let returned = vyceai_daily_usage_section(&state, &payload, now + 6);
        assert_eq!(returned.rows[0].cost_or_pct.as_deref(), Some("$2.00"));
    }

    #[test]
    fn vyceai_invalid_responses_and_storage_failures_preserve_baselines() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let now = chrono::DateTime::parse_from_rfc3339("2030-01-02T02:00:00Z").unwrap().timestamp();
        let payload = vyceai_daily_test_payload(1000.0, 100.0, 10.0, 40.0, "account-a");
        vyceai_daily_usage_section(&state, &payload, now);
        let path = temp.path().join("state/vyceai-usage-snapshot.json");
        let previous = std::fs::read(&path).unwrap();
        let invalid = vyceai_daily_usage_section(&state, &serde_json::json!({"error":"[REDACTED]"}), now + 300);
        assert!(!invalid.errors.is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), previous);
        std::fs::write(&path, "invalid").unwrap();
        let corrupt = vyceai_daily_usage_section(&state, &payload, now + 600);
        assert_eq!(corrupt.errors, ["Vyce AI 用量快照无法解析"]);
        assert_eq!(std::fs::read(&path).unwrap(), b"invalid");
        let blocked = tempfile::tempdir().unwrap();
        std::fs::write(blocked.path().join("state"), "blocked").unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), blocked.path());
        let section = vyceai_daily_usage_section(&state, &payload, now);
        assert!(!section.errors.is_empty());
        assert!(section.rows.is_empty());
    }

    #[tokio::test]
    async fn vyceai_daily_caches_expire_at_shanghai_midnight_and_reject_legacy_totals() {
        let temp = tempfile::tempdir().unwrap();
        let state = Arc::new(test_app_state("http://127.0.0.1:1".into(), temp.path()));
        let now = chrono::Utc::now().timestamp();
        let yesterday = ProviderUsageSection {
            provider: "vyceai".into(), captured_at: (vyceai_day_start(now) - 1) as f64,
            windows: vec![ProviderUsageWindow { window: "今日用量（差值）".into(), used: Some("10 / 1 / $1.00".into()), reset: None, reset_at: None }],
            ..Default::default()
        };
        save_shared_provider_section(&state, &yesterday, now as f64);
        assert!(shared_cached_section(&state, "vyceai").is_none());
        *state.provider_usage_cache.payload.write().await = Some(ProviderUsagePayload {
            sections: vec![yesterday], ..Default::default()
        });
        *state.provider_usage_cache.fetched_at.write().await = Some(Instant::now());
        assert!(cached_provider_payload(&state, "vyceai").await.is_none());
        let response = provider_usage_handler(State(state.clone()), Query(ProviderUsageQuery { provider: None, refresh: None })).await;
        let body: Value = serde_json::from_slice(&axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
        assert!(!body["sections"].as_array().unwrap().iter().any(|section| section["provider"] == "vyceai"));
        let legacy = ProviderUsageSection { provider: "vyceai".into(), captured_at: now as f64, ..Default::default() };
        save_shared_provider_section(&state, &legacy, now as f64);
        assert!(shared_cached_section(&state, "vyceai").is_none());
    }

    #[test]
    fn vyceai_collector_is_aligned_to_midnight_and_runs_without_an_open_page() {
        let before = chrono::DateTime::parse_from_rfc3339("2030-01-02T15:59:59Z").unwrap().timestamp();
        assert_eq!(vyceai_next_snapshot_delay(before), Duration::from_secs(1));
        assert_eq!(vyceai_next_snapshot_delay(before + 1), Duration::from_secs(300));
        assert!(include_str!("../mod.rs").contains("tokio::spawn(run_vyceai_snapshot_collector(state.clone()))"));
    }

    #[tokio::test]
    async fn vyceai_concurrent_fetches_serialize_cumulative_snapshots() {
        async fn dashboard(axum::extract::State(calls): axum::extract::State<Arc<std::sync::atomic::AtomicUsize>>) -> axum::Json<Value> {
            let n = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst) as f64;
            if n == 0.0 { tokio::time::sleep(Duration::from_millis(30)).await; }
            axum::Json(vyceai_daily_test_payload(1000.0 + n * 10.0, 100.0 + n, 10.0 + n * 0.5, 40.0 + n, "account-a"))
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = axum::Router::new().route("/user/dashboard", axum::routing::get(dashboard))
            .with_state(Arc::new(std::sync::atomic::AtomicUsize::new(0)));
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join(".env"), "VYCEAI_COOKIE='session=[REDACTED]'\n").unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let url = format!("http://{addr}/user/dashboard");
        let (first, second) = tokio::join!(fetch_vyceai_usage_from_url(&state, &url), fetch_vyceai_usage_from_url(&state, &url));
        assert_eq!(first.windows[0].used.as_deref(), Some("0 / 0 / $0.00"));
        assert_eq!(second.windows[0].used.as_deref(), Some("10 / 1 / $0.50"));
        let third = fetch_vyceai_usage_from_url(&state, &url).await;
        assert_eq!(third.windows[0].used.as_deref(), Some("20 / 2 / $1.00"));
    }

    #[test]
    fn vyceai_daily_paid_image_with_tiny_cost_is_not_reported_as_zero() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(), temp.path());
        let now = chrono::DateTime::parse_from_rfc3339("2030-01-02T02:00:00Z").unwrap().timestamp();
        vyceai_daily_usage_section(&state, &vyceai_daily_test_payload(0.0, 0.0, 0.0, 1.0, "account-a"), now);
        let section = vyceai_daily_usage_section(&state, &vyceai_daily_test_payload(0.0, 0.0, 0.00003985, 2.0, "account-a"), now + 300);
        assert_eq!(section.windows[0].used.as_deref(), Some("0 / 0 / $0.00003985"));
        assert_eq!(section.rows[0].cost_or_pct.as_deref(), Some("$0.00003985"));
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
    fn codex_credit_uses_usage_balance_and_omits_zero_or_invalid_values() {
        for balance in [serde_json::json!(61894.9120765), serde_json::json!("61894.9120765000")] {
            let credit = codex_usage_credit("alpha", &serde_json::json!({"credits": {"balance": balance}})).unwrap();
            assert_eq!(credit.account, "alpha");
            assert_eq!(credit.balance, 61894.9120765);
        }
        for balance in [serde_json::json!(0), serde_json::json!("0.0000"), serde_json::json!(-1), serde_json::json!("NaN"), serde_json::json!("Infinity"), Value::Null] {
            assert!(codex_usage_credit("alpha", &serde_json::json!({"credits": {"balance": balance}})).is_none());
        }
        assert!(codex_usage_credit("alpha", &serde_json::json!({"credits": {"has_credits": true}})).is_none());
        assert!(codex_usage_credit("alpha", &serde_json::json!({"_reset_credits": {"available_count": 5}})).is_none());
    }

    #[tokio::test]
    async fn codex_credit_is_queried_cached_per_account_and_cleared_when_zero() {
        use axum::extract::State;
        use std::sync::atomic::{AtomicUsize, Ordering};
        async fn usage(State(calls): State<Arc<AtomicUsize>>, headers: axum::http::HeaderMap) -> (axum::http::StatusCode, Json<Value>) {
            assert!(headers["authorization"] == "Bearer [REDACTED]", "mock authorization mismatch");
            let n = calls.fetch_add(1, Ordering::SeqCst);
            if n == 2 { return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"unavailable"}))); }
            let balance = match n { 0 => "25.5", 1 => "0.0000", _ => "50.75" };
            (axum::http::StatusCode::OK, Json(serde_json::json!({
                "credits": {"balance":balance, "has_credits":balance != "0.0000"},
                "rate_limit": {"primary_window": {"used_percent":100.0,"limit_window_seconds":18000,"reset_at":chrono::Utc::now().timestamp()+3600}}
            })))
        }
        async fn resets(State(calls): State<Arc<AtomicUsize>>) -> (axum::http::StatusCode, Json<Value>) {
            if calls.load(Ordering::SeqCst) == 4 { return (axum::http::StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error":"unavailable"}))); }
            (axum::http::StatusCode::OK, Json(serde_json::json!({"available_count":0,"credits":[]})))
        }
        let calls = Arc::new(AtomicUsize::new(0));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = axum::Router::new().route("/api/codex/usage",get(usage)).route("/api/codex/rate-limit-reset-credits",get(resets)).with_state(calls.clone());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap(); });
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("auth.json"), r#"{"credential_pool":{"openai-codex":[{"label":"alpha","access_token":"[REDACTED]"}]}}"#).unwrap();
        let state = test_app_state("http://127.0.0.1:1".into(),temp.path());
        let url = format!("http://{address}/codex");
        let first = fetch_codex_usage_from_url(&state,None,false,&url).await;
        assert!(first.errors.is_empty());
        assert_eq!(first.credits.len(),1);
        assert_eq!(first.credits[0].account,"alpha");
        assert_eq!(first.credits[0].balance,25.5);
        let skipped = fetch_codex_usage_from_url(&state,Some(&first),false,&url).await;
        assert_eq!(skipped.credits[0].balance,25.5);
        assert_eq!(calls.load(Ordering::SeqCst),1);
        let zero = fetch_codex_usage_from_url(&state,Some(&first),true,&url).await;
        assert!(zero.errors.is_empty());
        assert!(zero.credits.is_empty());
        assert!(serde_json::to_value(&zero).unwrap().get("credits").is_none());
        let failed = fetch_codex_usage_from_url(&state,Some(&first),true,&url).await;
        assert!(!failed.errors.is_empty());
        assert_eq!(failed.credits[0].balance,25.5);
        let reset_failed = fetch_codex_usage_from_url(&state,Some(&first),true,&url).await;
        assert!(reset_failed.errors.is_empty());
        assert_eq!(reset_failed.credits[0].balance,50.75);
        assert!(reset_failed.description.contains("alpha：Reset：0个"));
        server.abort();
    }

    #[test]
    fn codex_credit_cache_is_account_scoped_and_old_sections_still_deserialize() {
        let now = chrono::Utc::now().timestamp();
        let cached = ProviderUsageSection {
            provider: "codex".into(),
            credits: vec![ProviderUsageCredit{account:"alpha".into(),balance:10.0},ProviderUsageCredit{account:"beta".into(),balance:20.0}],
            windows: ["alpha","beta"].into_iter().map(|label| ProviderUsageWindow{window:format!("{label} 5h额度"),used:Some("100%".into()),reset:None,reset_at:Some(now+3600)}).collect(),
            ..Default::default()
        };
        for (account,balance) in [("alpha",10.0),("beta",20.0)] {
            let section = provider_cached_account_section(&cached,account,true,now).unwrap();
            assert_eq!(section.credits.len(),1);
            assert_eq!(section.credits[0].account,account);
            assert_eq!(section.credits[0].balance,balance);
        }
        let old: ProviderUsageSection = serde_json::from_value(serde_json::json!({"provider":"codex","title":"Codex","description":"","rows":[],"windows":[],"errors":[]})).unwrap();
        assert!(old.credits.is_empty());
        assert!(serde_json::to_value(&old).unwrap().get("credits").is_none());
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
    fn minimax_catalog_requires_only_cookie_with_embedded_group_id() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join(".env"),
            "MINIMAX_COOKIE='_token=[REDACTED]; minimax_group_id_v2=[REDACTED]'\n",
        )
        .unwrap();
        let provider = provider_usage_catalog(temp.path())
            .into_iter()
            .find(|item| item.provider == "minimax")
            .unwrap();
        assert!(provider.configured);
        assert!(provider.query_ready);
        assert_eq!(provider.credential_hint, "MINIMAX_COOKIE");
        assert!(provider.setup_hint.contains("minimax_group_id_v2"));
        assert!(!provider.setup_hint.contains("MINIMAX_GROUP_ID"));
    }

    #[test]
    fn minimax_request_headers_derive_group_id_from_cookie() {
        let headers =
            minimax_request_headers("_token=[REDACTED]; minimax_group_id_v2=[REDACTED]").unwrap();
        assert!(headers
            .iter()
            .any(|(name, value)| name == "x-group-id" && value == "[REDACTED]"));
        assert!(headers
            .iter()
            .any(|(name, value)| name == "Cookie"
                && value == "_token=[REDACTED]; minimax_group_id_v2=[REDACTED]"));
        assert_eq!(minimax_request_headers(""), Err("缺少 MINIMAX_COOKIE"));
        assert_eq!(
            minimax_request_headers("_token=[REDACTED]"),
            Err("MINIMAX_COOKIE 缺少 minimax_group_id_v2")
        );
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
                credits: Vec::new(),
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
    fn justwoker_catalog_needs_account_credentials_even_with_inference_key() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join(".env"),
            "HERMES_CUSTOM_API_JUSTWOKER_ICU_API_KEY=[REDACTED]\n").unwrap();
        let provider = provider_usage_catalog(temp.path()).into_iter()
            .find(|item| item.provider == "justwoker").unwrap();
        assert!(provider.configured);
        assert_eq!(provider.query_ready,
            !std::env::var("JUSTWOKER_USER_ID").unwrap_or_default().is_empty()
                && !std::env::var("JUSTWOKER_ACCESS_TOKEN").unwrap_or_default().is_empty());
        assert_eq!(provider.title, "JustWoker 用量");
        std::fs::write(temp.path().join(".env"),
            "JUSTWOKER_USER_ID=123\nJUSTWOKER_SESSION_COOKIE=session=[REDACTED]\n").unwrap();
        let provider = provider_usage_catalog(temp.path()).into_iter()
            .find(|item| item.provider == "justwoker").unwrap();
        assert!(provider.query_ready);
    }

    #[test]
    fn justwoker_usage_section_keeps_its_own_provider_identity() {
        let section = newapi_usage_section("justwoker", "JustWoker 用量", &[],
            500_000.0, 1_000_000.0, 250_000.0, chrono::Utc::now());
        assert_eq!(section.provider, "justwoker");
        assert_eq!(section.title, "JustWoker 用量");
        assert_eq!(section.description, "余额 **$2.00**；累计已用 **$0.50**");
    }

    #[test]
    fn agentrouter_default_usage_host_changes_without_renaming_the_card() {
        let source = include_str!("../provider_usage.rs");
        assert!(source.contains("\"https://ps.air-outer.com\""));
        let temp = tempfile::tempdir().unwrap();
        let provider = provider_usage_catalog(temp.path())
            .into_iter()
            .find(|item| item.provider == "agentrouter")
            .unwrap();
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
        let section = newapi_usage_section(
            "agentrouter", "AgenRouter 用量", &records,
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
