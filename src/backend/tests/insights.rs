    #[test]
    fn insights_aggregates_recent_api_session_rows_by_model_without_db() {
        let ts = chrono::NaiveDate::from_ymd_opt(2026, 6, 9)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp() as f64;
        let rows = vec![
            serde_json::json!({"id":"s1","source":"telegram","model":"minimax/m3","last_active":ts,"input_tokens":100,"output_tokens":20,"cache_read_tokens":900,"cache_write_tokens":10,"reasoning_tokens":5,"api_call_count":2,"tool_call_count":3,"estimated_cost_usd":0.12}),
            serde_json::json!({"id":"s2","source":"api_server","model":"gpt-5.5","last_active":ts - 86400.0,"input_tokens":50,"output_tokens":10,"cache_read_tokens":0,"cache_write_tokens":0,"reasoning_tokens":0,"api_call_count":1,"tool_call_count":0,"estimated_cost_usd":0.02}),
        ];

        let body = aggregate_usage_insights_with_prices(&rows, ts, &ModelPriceCatalog::new(), 7);

        assert_eq!(body["window_days"], 7);
        assert_eq!(body["totals"]["input"], 150);
        assert_eq!(body["totals"]["output"], 30);
        assert_eq!(body["totals"]["cache_read"], 900);
        assert_eq!(body["daily"].as_array().unwrap().len(), 7);
        assert_eq!(body["models"][0]["model"], "minimax/m3");
        assert_eq!(body["periods"].as_array().unwrap().len(), 1);
        let seven_day = body["periods"].as_array().unwrap().iter().find(|item| item["days"] == 7).unwrap();
        assert_eq!(seven_day["sources"].as_array().unwrap().len(), 2);
        assert_eq!(seven_day["sources"][0]["source"], "telegram");
        assert_eq!(seven_day["sources"][1]["source"], "api_server");

        let one_day_body = aggregate_usage_insights_with_prices(&rows, ts, &ModelPriceCatalog::new(), 1);
        assert_eq!(one_day_body["window_days"], 1);
        assert_eq!(one_day_body["daily"].as_array().unwrap().len(), 1);
        assert_eq!(one_day_body["periods"].as_array().unwrap().len(), 1);
        let one_day = one_day_body["periods"].as_array().unwrap().iter().find(|item| item["days"] == 1).unwrap();
        assert_eq!(one_day["totals"]["input"], 100);
        assert!(one_day["totals"]["cache_hit_rate"].as_f64().unwrap() > 0.89);
        assert_eq!(one_day["sources"][0]["source"], "telegram");
        assert_eq!(one_day["sources"][0]["totals"]["input"], 100);
    }

    #[test]
    fn insights_attributes_session_totals_to_the_started_day() {
        let now = chrono::NaiveDate::from_ymd_opt(2026, 6, 9)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp() as f64;
        let started_at = now - (2.0 * 86_400.0);
        let rows = vec![serde_json::json!({
            "id": "long-lived",
            "source": "api_server",
            "model": "gpt-5.6-sol",
            "started_at": started_at,
            "last_active": now,
            "input_tokens": 100,
            "estimated_cost_usd": 1.25,
        })];

        let body = aggregate_usage_insights_with_prices(&rows, now, &ModelPriceCatalog::new(), 7);
        let daily = body["daily"].as_array().unwrap();
        let started_day = daily.iter().find(|item| item["date"] == "2026-06-07").unwrap();
        let today = daily.iter().find(|item| item["date"] == "2026-06-09").unwrap();

        assert_eq!(started_day["totals"]["input"], 100);
        assert_eq!(started_day["totals"]["cost_usd"], 1.25);
        assert_eq!(today["totals"]["input"], 0);
        assert_eq!(today["totals"]["cost_usd"], 0.0);
    }

    #[test]
    fn insights_defaults_to_seven_day_period_and_accepts_only_ui_periods() {
        assert_eq!(normalize_insights_period(None), 7);
        assert_eq!(normalize_insights_period(Some(7)), 7);
        assert_eq!(normalize_insights_period(Some(1)), 1);
        assert_eq!(normalize_insights_period(Some(30)), 30);
        assert_eq!(normalize_insights_period(Some(2)), 7);
    }

    #[test]
    fn insights_returns_recent_hourly_usage_buckets() {
        let ts = chrono::NaiveDate::from_ymd_opt(2026, 6, 9)
            .unwrap()
            .and_hms_opt(12, 30, 0)
            .unwrap()
            .and_utc()
            .timestamp() as f64;
        let rows = vec![
            serde_json::json!({"id":"s1","source":"telegram","model":"minimax/m3","last_active":ts,"input_tokens":100,"output_tokens":20}),
            serde_json::json!({"id":"s2","source":"telegram","model":"minimax/m3","last_active":ts - 3600.0,"input_tokens":50,"output_tokens":10}),
        ];

        let body = aggregate_usage_insights_with_prices(&rows, ts, &ModelPriceCatalog::new(), 1);
        let hourly = body["hourly"].as_array().unwrap();
        let model_hourly = body["models"][0]["hourly"].as_array().unwrap();

        assert_eq!(hourly.len(), 24);
        assert_eq!(hourly.last().unwrap()["hour"], "2026-06-09T12:00:00Z");
        assert_eq!(hourly.last().unwrap()["label"], "12:00");
        assert_eq!(hourly.last().unwrap()["totals"]["input"], 100);
        assert_eq!(hourly[22]["hour"], "2026-06-09T11:00:00Z");
        assert_eq!(hourly[22]["totals"]["output"], 10);
        assert_eq!(model_hourly.last().unwrap()["totals"]["total_tokens"], 120);
    }

    #[test]
    fn insights_estimates_cost_from_models_dev_catalog_when_api_rows_have_no_cost() {
        let ts = chrono::NaiveDate::from_ymd_opt(2026, 6, 9)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp() as f64;
        let rows = vec![
            serde_json::json!({"id":"m3","source":"telegram","model":"minimax/m3","last_active":ts,"input_tokens":1_000_000,"output_tokens":100_000,"cache_read_tokens":9_000_000,"cache_write_tokens":0,"reasoning_tokens":0}),
            serde_json::json!({"id":"unknown","source":"telegram","model":"unknown-model","last_active":ts,"input_tokens":500,"output_tokens":100,"cache_read_tokens":0,"cache_write_tokens":0,"reasoning_tokens":0}),
            serde_json::json!({"id":"actual","source":"telegram","model":"minimax/m3","last_active":ts,"input_tokens":1_000_000,"output_tokens":100_000,"cache_read_tokens":9_000_000,"actual_cost_usd":42.0}),
        ];
        let models_dev = serde_json::json!({
            "minimax": {
                "id": "minimax",
                "models": {
                    "MiniMax-M3": {
                        "id": "MiniMax-M3",
                        "cost": {"input": 0.6, "output": 2.4, "cache_read": 0.12}
                    }
                }
            }
        });
        let catalog = model_price_catalog_from_models_dev(&models_dev);

        let body = aggregate_usage_insights_with_prices(&rows, ts, &catalog, 1);
        let totals = &body["totals"];

        assert!((totals["estimated_cost_usd"].as_f64().unwrap() - 3.84).abs() < 0.000001);
        assert!((totals["cost_usd"].as_f64().unwrap() - 3.84).abs() < 0.000001);
        assert_eq!(totals["actual_cost_usd"], 42.0);
        assert_eq!(totals["unpriced_tokens"], 600);
        let one_day = body["periods"].as_array().unwrap().iter().find(|item| item["days"] == 1).unwrap();
        assert!((one_day["totals"]["cost_usd"].as_f64().unwrap() - 3.84).abs() < 0.000001);
    }

    #[test]
    fn insights_uses_catalog_api_price_for_full_tokens_instead_of_partial_session_estimate() {
        let ts = chrono::NaiveDate::from_ymd_opt(2026, 6, 9)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp() as f64;
        let rows = vec![serde_json::json!({
            "id": "partial-estimate",
            "source": "telegram",
            "model": "gpt-5.6-sol",
            "provider": "openai-codex",
            "last_active": ts,
            "input_tokens": 1_000_000,
            "output_tokens": 100_000,
            "cache_read_tokens": 9_000_000,
            "estimated_cost_usd": 7.5
        })];
        let catalog = model_price_catalog_from_models_dev(&serde_json::json!({
            "openai": {
                "models": {
                    "gpt-5.6-sol": {
                        "id": "gpt-5.6-sol",
                        "cost": {"input": 5.0, "output": 30.0, "cache_read": 0.5}
                    }
                }
            }
        }));

        let body = aggregate_usage_insights_with_prices(&rows, ts, &catalog, 1);

        assert_eq!(body["totals"]["cost_usd"], 12.5);
        assert_eq!(body["totals"]["estimated_cost_usd"], 12.5);
        assert_eq!(body["totals"]["unpriced_tokens"], 0);
        assert_eq!(body["models"][0]["totals"]["cost_usd"], 12.5);
    }

    #[test]
    fn insights_leaves_tokens_unpriced_without_models_dev_price() {
        let ts = chrono::NaiveDate::from_ymd_opt(2026, 6, 9)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp() as f64;
        let rows = vec![
            serde_json::json!({"id":"m3","source":"telegram","model":"minimax/m3","last_active":ts,"input_tokens":10,"output_tokens":20,"cache_read_tokens":30,"cache_write_tokens":0,"reasoning_tokens":0}),
        ];
        let catalog = model_price_catalog_from_models_dev(&serde_json::json!({}));

        let body = aggregate_usage_insights_with_prices(&rows, ts, &catalog, 1);

        assert_eq!(body["totals"]["cost_usd"], 0.0);
        assert_eq!(body["totals"]["unpriced_tokens"], 60);
    }

    #[tokio::test]
    async fn insights_scans_past_resumed_old_sessions_for_recent_starts() {
        async fn sessions_page(
            axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
        ) -> axum::Json<serde_json::Value> {
            const NOW: f64 = 1_780_000_000.0;
            let offset = params.get("offset").and_then(|value| value.parse::<usize>().ok()).unwrap_or(0);
            if offset == 0 {
                let old = serde_json::json!({
                    "id": "resumed-old",
                    "source": "api_server",
                    "started_at": NOW - (40.0 * 86_400.0),
                    "last_active": NOW,
                    "input_tokens": 10,
                });
                axum::Json(serde_json::json!({"data": vec![old; INSIGHTS_PAGE_SIZE], "has_more": true}))
            } else {
                axum::Json(serde_json::json!({"data": [{
                    "id": "recent",
                    "source": "api_server",
                    "started_at": NOW - 86_400.0,
                    "last_active": NOW - 86_400.0,
                    "input_tokens": 20,
                }], "has_more": false}))
            }
        }

        let app = axum::Router::new().route("/api/sessions", get(sessions_page));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state(format!("http://{addr}"), temp.path());

        let rows = fetch_recent_sessions_for_insights(&state, 1_780_000_000.0, 7).await.unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "recent");
    }

    #[tokio::test]
    async fn insights_price_catalog_uses_cached_models_dev_payload() {
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());
        {
            let mut cache = state.model_price_cache.write().await;
            cache.fetched_at = Some(std::time::Instant::now());
            cache.body = Some(serde_json::json!({
                "minimax": {
                    "id": "minimax",
                    "models": {
                        "MiniMax-M3": {
                            "id": "MiniMax-M3",
                            "cost": {"input": 0.6, "output": 2.4, "cache_read": 0.12}
                        }
                    }
                }
            }));
        }

        let catalog = fetch_models_dev_price_catalog(&state).await.unwrap();
        let price = model_price_for_model(&catalog, "minimax/m3").unwrap();

        assert!((price.estimate(1_000_000, 100_000, 9_000_000, 0) - 1.92).abs() < 0.000001);
    }

    #[tokio::test]
    async fn insights_price_catalog_uses_stale_cache_when_models_dev_refresh_fails() {
        async fn unavailable() -> StatusCode {
            StatusCode::BAD_GATEWAY
        }

        let app = axum::Router::new().route("/api.json", get(unavailable));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());
        {
            let mut cache = state.model_price_cache.write().await;
            cache.fetched_at = Some(std::time::Instant::now() - MODEL_PRICE_CACHE_TTL - Duration::from_secs(1));
            cache.body = Some(serde_json::json!({
                "openai": {
                    "id": "openai",
                    "models": {
                        "gpt-5.5": {
                            "id": "gpt-5.5",
                            "cost": {"input": 5.0, "output": 30.0, "cache_read": 0.5}
                        }
                    }
                }
            }));
        }

        let catalog = fetch_models_dev_price_catalog_from_url(&state, &format!("http://{addr}/api.json")).await.unwrap();
        let price = model_price_for_model(&catalog, "gpt-5.5").unwrap();

        assert!((price.estimate(1_000_000, 100_000, 9_000_000, 0) - 12.5).abs() < 0.000001);
    }

    #[tokio::test]
    async fn chat_messages_page_injects_turn_duration_on_assistant_without_preceding_user_in_page() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, parent_session_id TEXT, started_at REAL, end_reason TEXT, source TEXT);
             CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT,
                tool_call_id TEXT,
                tool_calls TEXT,
                tool_name TEXT,
                timestamp REAL NOT NULL,
                token_count INTEGER,
                finish_reason TEXT,
                reasoning TEXT,
                reasoning_content TEXT,
                active INTEGER NOT NULL DEFAULT 1
             );",
        ).unwrap();
        // user1(ts=100) → tool1(ts=105) → assistant1 content(ts=110)
        // → user2(ts=200) → tool2(ts=205) → assistant2 content(ts=212.5)
        // → user3(ts=300)
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,end_reason,source) VALUES ('s1',NULL,1,NULL,'telegram')", []).unwrap();
        for (id, role, content, ts) in [
            (1i64, "user", "user1 msg", 100.0f64),
            (2, "assistant", "", 105.0),  // tool-calls assistant (empty content)
            (3, "tool", "tool1 output", 105.0),
            (4, "assistant", "reply1", 110.0),
            (5, "user", "user2 msg", 200.0),
            (6, "assistant", "", 205.0),  // tool-calls assistant
            (7, "tool", "tool2 output", 205.0),
            (8, "assistant", "reply2", 212.5),
            (9, "user", "user3 msg", 300.0),
        ] {
            conn.execute(
                "INSERT INTO messages (id,session_id,role,content,timestamp,active) VALUES (?1,'s1',?2,?3,?4,1)",
                rusqlite::params![id, role, content, ts],
            ).unwrap();
        }
        drop(conn);
        let state = Arc::new(test_app_state("http://127.0.0.1:1".to_string(), temp.path()));

        // Request only 3 messages from the tail — user2 and its turn are NOT in this window
        let resp = chat_messages_page(
            State(state),
            AxumPath("s1".to_string()),
            Query(ChatMessagesQuery { before: None, after: None, around: None, limit: Some(3), view: Some("full".to_string()) }),
        ).await;
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let page: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(page["total"], 9);
        assert_eq!(page["data"].as_array().unwrap().len(), 3);
        // The last 3 messages: tool2 (id=7), assistant reply2 (id=8), user3 (id=9)
        // user2 (ts=200 → id=5) is NOT in the page
        let data = page["data"].as_array().unwrap();
        assert_eq!(data[0]["id"], 7);   // tool2 — no duration
        assert!(data[0].get("duration_ms").is_none(), "tool must not have duration_ms");
        assert_eq!(data[1]["id"], 8);   // assistant reply2 — MUST have duration_ms
        assert_eq!(data[1]["role"], "assistant");
        assert!(data[1].get("duration_ms").is_some(), "assistant with content must have duration_ms");
        assert_eq!(data[1]["duration_ms"], 12500.0); // (212.5 - 200) * 1000
        assert_eq!(data[1]["content"], "reply2");
        assert_eq!(data[2]["id"], 9);   // user3 — no duration
        assert!(data[2].get("duration_ms").is_none(), "user must not have duration_ms");
    }
