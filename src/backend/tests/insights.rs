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
    fn insights_keeps_same_model_separate_per_provider() {
        let ts = chrono::NaiveDate::from_ymd_opt(2026, 6, 9)
            .unwrap()
            .and_hms_opt(12, 0, 0)
            .unwrap()
            .and_utc()
            .timestamp() as f64;
        let rows = vec![
            serde_json::json!({"id":"provider-a","source":"telegram","model":"shared-model","provider":"provider-a","started_at":ts,"input_tokens":100}),
            serde_json::json!({"id":"provider-b","source":"telegram","model":"shared-model","provider":"provider-b","started_at":ts,"input_tokens":200}),
        ];

        let body = aggregate_usage_insights_with_prices(&rows, ts, &ModelPriceCatalog::new(), 1);
        let models = body["models"].as_array().unwrap();

        assert_eq!(models.len(), 2);
        assert_eq!(models[0]["model"], "shared-model");
        assert_eq!(models[0]["provider"], "provider-b");
        assert_eq!(models[1]["provider"], "provider-a");
    }

    #[test]
    fn named_custom_provider_uses_config_name_and_falls_back_to_custom_on_errors() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join("config.yaml"),
            "custom_providers:\n  - name: ark\n    base_url: https://ark.example/v1/\n",
        )
        .unwrap();
        let aliases = custom_provider_aliases(temp.path());
        assert_eq!(resolve_provider_name("custom", Some("https://ark.example/v1"), &aliases), "ark");
        assert_eq!(resolve_provider_name("custom", Some("https://other.example/v1"), &aliases), "custom");

        std::fs::write(temp.path().join("config.yaml"), "custom_providers: [broken").unwrap();
        let aliases = custom_provider_aliases(temp.path());
        assert_eq!(resolve_provider_name("custom", Some("https://ark.example/v1"), &aliases), "custom");
        assert_eq!(resolve_provider_name("custom:ark", None, &aliases), "ark");
    }

    #[test]
    fn insights_prefers_gateway_runtime_provider_over_stale_billing_provider() {
        let config = r#"{"gateway_runtime":{"provider":"opencode-go","base_url":"https://opencode.ai/zen/go/v1"}}"#;
        assert_eq!(
            resolve_session_provider("deepseek", Some("https://api.deepseek.com/v1"), Some(config), &HashMap::new()),
            "opencode-go"
        );
    }

    #[test]
    fn insights_backfills_snapshot_provider_from_session_or_unique_model_route() {
        let temp = tempfile::tempdir().unwrap();
        let state_path = temp.path().join("state.db");
        let snapshot_path = temp.path().join("yahu-insights-usage.db");
        let state_conn = rusqlite::Connection::open(&state_path).unwrap();
        state_conn
            .execute_batch(
                "CREATE TABLE sessions (
                     id TEXT PRIMARY KEY,
                     model TEXT,
                     billing_provider TEXT,
                     billing_base_url TEXT,
                     model_config TEXT
                 );
                 INSERT INTO sessions(id, model, billing_provider) VALUES
                     ('known-session', 'unique-model', 'provider-a'),
                     ('ambiguous-a', 'ambiguous-model', 'provider-a'),
                     ('ambiguous-b', 'ambiguous-model', 'provider-b');",
            )
            .unwrap();
        drop(state_conn);

        let snapshot_conn = rusqlite::Connection::open(&snapshot_path).unwrap();
        prepare_insights_snapshot_db(&snapshot_conn).unwrap();
        let json = |id: &str, model: &str| serde_json::json!({"id": id, "model": model});
        let unique = serde_json::to_string(&json("legacy-unique", "unique-model")).unwrap();
        let ambiguous = serde_json::to_string(&json("legacy-ambiguous", "ambiguous-model")).unwrap();
        let unique_counter = serde_json::to_string(&serde_json::json!({"session_id":"legacy-unique","model":"unique-model"})).unwrap();
        let ambiguous_counter = serde_json::to_string(&serde_json::json!({"session_id":"legacy-ambiguous","model":"ambiguous-model"})).unwrap();
        snapshot_conn
            .execute(
                "INSERT INTO insights_events(captured_at, row_json) VALUES(0, ?1)",
                [&unique],
            )
            .unwrap();
        snapshot_conn
            .execute(
                "INSERT INTO insights_events(captured_at, row_json) VALUES(0, ?1)",
                [&ambiguous],
            )
            .unwrap();
        snapshot_conn
            .execute(
                "INSERT INTO insights_baselines(session_id, captured_at, last_seen, counters_json) VALUES(?1, 0, 0, ?2)",
                rusqlite::params!["legacy-unique", &unique_counter],
            )
            .unwrap();
        snapshot_conn
            .execute(
                "INSERT INTO insights_baselines(session_id, captured_at, last_seen, counters_json) VALUES(?1, 0, 0, ?2)",
                rusqlite::params!["legacy-ambiguous", &ambiguous_counter],
            )
            .unwrap();
        snapshot_conn
            .execute(
                "INSERT INTO insights_initial_baselines(session_id, counters_json) VALUES(?1, ?2)",
                rusqlite::params!["legacy-unique", &unique_counter],
            )
            .unwrap();
        snapshot_conn
            .execute(
                "INSERT INTO insights_initial_baselines(session_id, counters_json) VALUES(?1, ?2)",
                rusqlite::params!["legacy-ambiguous", &ambiguous_counter],
            )
            .unwrap();
        drop(snapshot_conn);

        let changed = backfill_snapshot_providers(&snapshot_path, &state_path).unwrap();
        assert_eq!(changed, 6);
        let conn = rusqlite::Connection::open(&snapshot_path).unwrap();
        for (table, column) in [
            ("insights_events", "row_json"),
            ("insights_baselines", "counters_json"),
            ("insights_initial_baselines", "counters_json"),
        ] {
            let rows = conn
                .prepare(&format!("SELECT {column} FROM {table} ORDER BY rowid"))
                .unwrap()
                .query_map([], |row| row.get::<_, String>(0))
                .unwrap()
                .map(|row| serde_json::from_str::<serde_json::Value>(&row.unwrap()).unwrap())
                .collect::<Vec<_>>();
            assert_eq!(rows[0]["provider"], "provider-a");
            assert_eq!(rows[1]["provider"], "unknown");
        }
    }

    #[test]
    fn insights_historical_backfill_subtracts_yahu_deltas_and_runs_once() {
        let temp = tempfile::tempdir().unwrap();
        let state_path = temp.path().join("state.db");
        let snapshot_path = temp.path().join("yahu-insights-usage.db");
        let state_conn = rusqlite::Connection::open(&state_path).unwrap();
        state_conn
            .execute_batch(
                "CREATE TABLE sessions (
                     id TEXT PRIMARY KEY,
                     source TEXT NOT NULL,
                     started_at REAL NOT NULL,
                     archived INTEGER NOT NULL DEFAULT 0,
                     end_reason TEXT
                 );
                 CREATE TABLE session_model_usage (
                     session_id TEXT NOT NULL,
                     model TEXT NOT NULL,
                     billing_provider TEXT NOT NULL DEFAULT '',
                     billing_base_url TEXT,
                     billing_mode TEXT NOT NULL DEFAULT '',
                     task TEXT NOT NULL DEFAULT '',
                     api_call_count INTEGER NOT NULL DEFAULT 0,
                     input_tokens INTEGER NOT NULL DEFAULT 0,
                     output_tokens INTEGER NOT NULL DEFAULT 0,
                     cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                     cache_write_tokens INTEGER NOT NULL DEFAULT 0,
                     reasoning_tokens INTEGER NOT NULL DEFAULT 0,
                     estimated_cost_usd REAL NOT NULL DEFAULT 0,
                     actual_cost_usd REAL NOT NULL DEFAULT 0,
                     PRIMARY KEY(session_id, model, billing_provider, billing_base_url, billing_mode, task)
                 );
                 INSERT INTO sessions(id, source, started_at) VALUES
                     ('old-session', 'telegram', 500),
                     ('new-session', 'telegram', 1100);
                 INSERT INTO session_model_usage(session_id, model, billing_provider, input_tokens, output_tokens, api_call_count)
                 VALUES ('old-session', 'gpt-5.6-sol', 'provider-a', 100, 20, 10),
                        ('new-session', 'gpt-5.6-sol', 'provider-a', 80, 10, 8);",
            )
            .unwrap();
        drop(state_conn);

        let snapshot_conn = rusqlite::Connection::open(&snapshot_path).unwrap();
        prepare_insights_snapshot_db(&snapshot_conn).unwrap();
        snapshot_conn
            .execute(
                "INSERT INTO insights_meta(key, value) VALUES('coverage_started_at', 1000)",
                [],
            )
            .unwrap();
        let id = usage_counter_id("old-session", "gpt-5.6-sol", "provider-a", "", "", "");
        let post_coverage = serde_json::json!({
            "id": id,
            "root_session_id": "old-session",
            "source": "telegram",
            "model": "gpt-5.6-sol",
            "provider": "provider-a",
            "started_at": 1500,
            "input_tokens": 30,
            "output_tokens": 5,
            "api_call_count": 3
        });
        snapshot_conn
            .execute(
                "INSERT INTO insights_events(captured_at, row_json) VALUES(1500, ?1)",
                [serde_json::to_string(&post_coverage).unwrap()],
            )
            .unwrap();
        drop(snapshot_conn);

        assert_eq!(backfill_historical_insights(&snapshot_path, &state_path).unwrap(), 1);
        let conn = rusqlite::Connection::open(&snapshot_path).unwrap();
        let rows = conn
            .prepare("SELECT captured_at, row_json FROM insights_events ORDER BY captured_at")
            .unwrap()
            .query_map([], |row| Ok((row.get::<_, f64>(0)?, row.get::<_, String>(1)?)))
            .unwrap()
            .map(|row| row.unwrap())
            .collect::<Vec<_>>();
        assert_eq!(rows.len(), 2);
        let historical: serde_json::Value = serde_json::from_str(&rows[0].1).unwrap();
        assert_eq!(rows[0].0, 500.0);
        assert_eq!(historical["input_tokens"], 70);
        assert_eq!(historical["output_tokens"], 15);
        assert_eq!(historical["api_call_count"], 7);
        assert_eq!(backfill_historical_insights(&snapshot_path, &state_path).unwrap(), 0);
        assert_eq!(conn.query_row("SELECT COUNT(*) FROM insights_events", [], |row| row.get::<_, i64>(0)).unwrap(), 2);
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
    fn insights_one_day_window_includes_previous_calendar_day_hours() {
        let now = chrono::NaiveDate::from_ymd_opt(2026, 6, 9)
            .unwrap()
            .and_hms_opt(1, 30, 0)
            .unwrap()
            .and_utc()
            .timestamp() as f64;
        let rows = vec![
            serde_json::json!({
                "id": "previous-evening",
                "source": "telegram",
                "model": "gpt-5.3-codex-spark",
                "started_at": now - (3.0 * 3600.0),
                "input_tokens": 70,
                "output_tokens": 30
            }),
            serde_json::json!({
                "id": "current-day",
                "source": "api_server",
                "model": "gpt-5.6-sol",
                "started_at": now - 1800.0,
                "input_tokens": 40,
                "output_tokens": 10
            }),
        ];

        let body = aggregate_usage_insights_with_prices(&rows, now, &ModelPriceCatalog::new(), 1);

        assert_eq!(body["totals"]["total_tokens"], 150);
        assert_eq!(body["models"].as_array().unwrap().len(), 2);
        assert_eq!(body["daily"][0]["totals"]["total_tokens"], 150);
        assert_eq!(
            body["hourly"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|hour| hour["totals"]["total_tokens"].as_i64())
                .sum::<i64>(),
            150
        );
        let one_day = &body["periods"][0];
        assert_eq!(one_day["totals"]["total_tokens"], 150);
        assert_eq!(one_day["models"].as_array().unwrap().len(), 2);
        assert_eq!(one_day["sources"].as_array().unwrap().len(), 2);
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

    #[test]
    fn insights_snapshot_uses_started_day_fallback_before_coverage_without_double_counting() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("insights-usage.db");
        let first = chrono::NaiveDate::from_ymd_opt(2026, 7, 12)
            .unwrap()
            .and_hms_opt(15, 55, 0)
            .unwrap()
            .and_utc()
            .timestamp() as f64;
        let second = chrono::NaiveDate::from_ymd_opt(2026, 7, 12)
            .unwrap()
            .and_hms_opt(16, 5, 0)
            .unwrap()
            .and_utc()
            .timestamp() as f64;
        let first_rows = vec![serde_json::json!({
            "id": "long-session",
            "source": "telegram",
            "model": "gpt-5.6-sol",
            "started_at": first - 172_800.0,
            "input_tokens": 100,
            "output_tokens": 20,
            "cache_read_tokens": 1_000,
            "api_call_count": 4
        })];
        let second_rows = vec![serde_json::json!({
            "id": "long-session",
            "source": "telegram",
            "model": "gpt-5.6-sol",
            "started_at": first - 172_800.0,
            "input_tokens": 150,
            "output_tokens": 25,
            "cache_read_tokens": 1_100,
            "api_call_count": 6
        })];

        persist_insights_snapshot(&path, first, &first_rows).unwrap();
        persist_insights_snapshot(&path, second, &second_rows).unwrap();
        let (usage_rows, coverage_started_at, latest_snapshot_at) =
            load_insights_usage_rows(&path, first - (7.0 * 86_400.0)).unwrap();

        assert_eq!(coverage_started_at, Some(first));
        assert_eq!(latest_snapshot_at, Some(second));
        assert_eq!(usage_rows.len(), 2);
        let fallback = usage_rows
            .iter()
            .find(|row| row["started_at"] == first - 172_800.0)
            .unwrap();
        let event = usage_rows.iter().find(|row| row["started_at"] == second).unwrap();
        assert_eq!(fallback["input_tokens"], 100);
        assert_eq!(fallback["output_tokens"], 20);
        assert_eq!(fallback["cache_read_tokens"], 1_000);
        assert_eq!(event["input_tokens"], 50);
        assert_eq!(event["output_tokens"], 5);
        assert_eq!(event["cache_read_tokens"], 100);
        assert_eq!(event["api_call_count"], 2);

        let body = aggregate_usage_insights_with_prices_at_offset(
            &usage_rows,
            second + 3600.0,
            &ModelPriceCatalog::new(),
            7,
            -480,
        );
        let daily = body["daily"].as_array().unwrap();
        let fallback_day = daily.iter().find(|item| item["date"] == "2026-07-10").unwrap();
        let event_day = daily.iter().find(|item| item["date"] == "2026-07-13").unwrap();
        assert_eq!(fallback_day["totals"]["total_tokens"], 1_120);
        assert_eq!(event_day["totals"]["total_tokens"], 155);
        assert_eq!(body["totals"]["total_tokens"], 1_275);
    }

    #[test]
    fn empty_insights_snapshot_records_the_latest_collection_time() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("usage.db");
        let captured_at = 1_789_000_000.0;

        persist_insights_snapshot(&path, captured_at, &[]).unwrap();
        let (rows, coverage_started_at, latest_snapshot_at) =
            load_insights_usage_rows(&path, captured_at - 86_400.0).unwrap();

        assert!(rows.is_empty());
        assert_eq!(coverage_started_at, Some(captured_at));
        assert_eq!(latest_snapshot_at, Some(captured_at));
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
    async fn insights_incremental_capture_uses_message_cursor_and_records_resumed_old_session() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        const CURSOR: f64 = 1_780_000_000.0;

        async fn sessions_page(
            State(calls): State<Arc<AtomicUsize>>,
            Query(params): Query<std::collections::HashMap<String, String>>,
        ) -> Json<serde_json::Value> {
            calls.fetch_add(1, Ordering::SeqCst);
            let offset = params
                .get("offset")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(0);
            if offset == 0 {
                Json(serde_json::json!({
                    "data": [
                        {
                            "id": "resumed-old",
                            "source": "api_server",
                            "model": "gpt-5.6-sol",
                            "started_at": CURSOR - (40.0 * 86_400.0),
                            "last_active": CURSOR + 10.0,
                            "input_tokens": 150
                        },
                        {
                            "id": "below-overlap",
                            "source": "api_server",
                            "model": "gpt-5.6-sol",
                            "started_at": CURSOR - (20.0 * 86_400.0),
                            "last_active": CURSOR - 61.0,
                            "input_tokens": 999
                        }
                    ],
                    "has_more": true
                }))
            } else {
                Json(serde_json::json!({
                    "data": [{
                        "id": "should-not-be-fetched",
                        "source": "api_server",
                        "started_at": CURSOR - 86_400.0,
                        "last_active": CURSOR - 120.0,
                        "input_tokens": 999
                    }],
                    "has_more": false
                }))
            }
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/api/sessions", get(sessions_page))
            .with_state(calls.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let temp = tempfile::tempdir().unwrap();
        let snapshot_path = temp.path().join(INSIGHTS_SNAPSHOT_DB);
        persist_insights_snapshot(
            &snapshot_path,
            CURSOR,
            &[
                serde_json::json!({
                    "id": "resumed-old",
                    "source": "api_server",
                    "model": "gpt-5.6-sol",
                    "started_at": CURSOR - (40.0 * 86_400.0),
                    "last_active": CURSOR - 300.0,
                    "input_tokens": 100
                }),
                serde_json::json!({
                    "id": "deleted-stale",
                    "source": "api_server",
                    "model": "gpt-5.6-sol",
                    "started_at": CURSOR - (40.0 * 86_400.0),
                    "input_tokens": 77
                }),
            ],
        )
        .unwrap();
        let snapshot_conn = rusqlite::Connection::open(&snapshot_path).unwrap();
        snapshot_conn
            .execute(
                "UPDATE insights_baselines SET last_seen = ?1 WHERE session_id = 'deleted-stale'",
                [CURSOR - (8.0 * 86_400.0)],
            )
            .unwrap();
        snapshot_conn
            .execute(
                "INSERT INTO insights_meta(key, value) VALUES('last_message_id', 100)",
                [],
            )
            .unwrap();

        let state_db = rusqlite::Connection::open(temp.path().join("state.db")).unwrap();
        state_db
            .execute_batch(
                "CREATE TABLE sessions (
                     id TEXT PRIMARY KEY,
                     source TEXT NOT NULL,
                     model TEXT,
                     billing_provider TEXT,
                     billing_base_url TEXT,
                     model_config TEXT,
                     parent_session_id TEXT,
                     started_at REAL NOT NULL,
                     ended_at REAL,
                     end_reason TEXT,
                     tool_call_count INTEGER DEFAULT 0,
                     input_tokens INTEGER DEFAULT 0,
                     output_tokens INTEGER DEFAULT 0,
                     cache_read_tokens INTEGER DEFAULT 0,
                     cache_write_tokens INTEGER DEFAULT 0,
                     reasoning_tokens INTEGER DEFAULT 0,
                     estimated_cost_usd REAL DEFAULT 0,
                     actual_cost_usd REAL DEFAULT 0,
                     api_call_count INTEGER DEFAULT 0,
                     archived INTEGER DEFAULT 0
                 );
                 CREATE TABLE messages (
                     id INTEGER PRIMARY KEY,
                     session_id TEXT NOT NULL,
                     timestamp REAL NOT NULL
                 );
                 INSERT INTO sessions(
                     id, source, model, started_at, input_tokens
                 ) VALUES(
                     'resumed-old', 'api_server', 'gpt-5.6-sol',
                     1776544000.0, 150
                 );
                 INSERT INTO messages(id, session_id, timestamp)
                 VALUES(101, 'resumed-old', 1780000010.0);",
            )
            .unwrap();
        let state = test_app_state(format!("http://{addr}"), temp.path());

        capture_insights_snapshot(&state, CURSOR + 300.0)
            .await
            .unwrap();

        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let (rows, _, latest_snapshot_at) =
            load_insights_usage_rows(&snapshot_path, CURSOR - (45.0 * 86_400.0)).unwrap();
        let delta = rows
            .iter()
            .find(|row| row["started_at"] == CURSOR + 300.0)
            .unwrap();
        assert_eq!(delta["input_tokens"], 50);
        assert_eq!(latest_snapshot_at, Some(CURSOR + 300.0));
        assert_eq!(
            snapshot_conn
                .query_row(
                    "SELECT COUNT(*) FROM insights_baselines WHERE session_id = 'deleted-stale'",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .unwrap(),
            0
        );
        assert_eq!(
            snapshot_conn
                .query_row(
                    "SELECT value FROM insights_meta WHERE key = 'last_baseline_cleanup_at'",
                    [],
                    |row| row.get::<_, f64>(0),
                )
                .unwrap(),
            CURSOR + 300.0
        );
    }

    #[test]
    fn insights_keeps_idle_session_baselines_for_future_incremental_deltas() {
        const START: f64 = 1_780_000_000.0;
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join(INSIGHTS_SNAPSHOT_DB);
        let initial = serde_json::json!({
            "id": "idle-then-resumed",
            "source": "api_server",
            "model": "gpt-5.6-sol",
            "started_at": START - (90.0 * 86_400.0),
            "input_tokens": 100
        });
        persist_insights_snapshot(&path, START, &[initial]).unwrap();
        persist_insights_snapshot(&path, START + (36.0 * 86_400.0), &[]).unwrap();
        let resumed = serde_json::json!({
            "id": "idle-then-resumed",
            "source": "api_server",
            "model": "gpt-5.6-sol",
            "started_at": START - (90.0 * 86_400.0),
            "input_tokens": 150
        });
        let resumed_at = START + (36.0 * 86_400.0) + 300.0;
        persist_insights_snapshot(&path, resumed_at, &[resumed]).unwrap();

        let (rows, _, _) = load_insights_usage_rows(&path, START + (35.0 * 86_400.0)).unwrap();
        let delta = rows
            .iter()
            .find(|row| row["started_at"] == resumed_at)
            .unwrap();
        assert_eq!(delta["input_tokens"], 50);
    }

    #[test]
    fn insights_weekly_cleanup_removes_only_deleted_stale_session_baselines() {
        const NOW: f64 = 1_800_000_000.0;
        let temp = tempfile::tempdir().unwrap();
        let snapshot_path = temp.path().join(INSIGHTS_SNAPSHOT_DB);
        let state_db_path = temp.path().join("state.db");
        let rows = [
            ("existing-stale", 10),
            ("deleted-stale", 20),
            ("deleted-recent", 30),
        ]
        .into_iter()
        .map(|(id, input)| {
            serde_json::json!({
                "id": id,
                "source": "api_server",
                "model": "gpt-5.6-sol",
                "started_at": NOW - (30.0 * 86_400.0),
                "input_tokens": input
            })
        })
        .collect::<Vec<_>>();
        persist_insights_snapshot(&snapshot_path, NOW - (8.0 * 86_400.0), &rows).unwrap();
        let snapshot_conn = rusqlite::Connection::open(&snapshot_path).unwrap();
        snapshot_conn
            .execute(
                "UPDATE insights_baselines
                 SET last_seen = CASE session_id
                     WHEN 'deleted-recent' THEN ?1
                     ELSE ?2
                 END",
                rusqlite::params![NOW - (6.0 * 86_400.0), NOW - (8.0 * 86_400.0)],
            )
            .unwrap();
        let state_conn = rusqlite::Connection::open(&state_db_path).unwrap();
        state_conn
            .execute_batch(
                "CREATE TABLE sessions (id TEXT PRIMARY KEY);
                 INSERT INTO sessions(id) VALUES('existing-stale');",
            )
            .unwrap();

        assert_eq!(
            cleanup_deleted_insights_baselines(&snapshot_path, &state_db_path, NOW).unwrap(),
            1
        );
        let remaining = snapshot_conn
            .prepare("SELECT session_id FROM insights_baselines ORDER BY session_id")
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(remaining, vec!["deleted-recent", "existing-stale"]);
        let initial_remaining = snapshot_conn
            .query_row(
                "SELECT COUNT(*) FROM insights_initial_baselines",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(initial_remaining, 2);

        assert_eq!(
            cleanup_deleted_insights_baselines(
                &snapshot_path,
                &state_db_path,
                NOW + (6.0 * 86_400.0),
            )
            .unwrap(),
            0
        );
        assert_eq!(
            snapshot_conn
                .query_row("SELECT COUNT(*) FROM insights_baselines", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
            2
        );

        assert_eq!(
            cleanup_deleted_insights_baselines(
                &snapshot_path,
                &state_db_path,
                NOW + (7.0 * 86_400.0),
            )
            .unwrap(),
            1
        );
        assert_eq!(
            snapshot_conn
                .query_row(
                    "SELECT session_id FROM insights_baselines",
                    [],
                    |row| row.get::<_, String>(0),
                )
                .unwrap(),
            "existing-stale"
        );
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
    async fn insights_serves_a_persisted_snapshot_without_waiting_for_collection() {
        async fn slow_sessions() -> Json<serde_json::Value> {
            sleep(Duration::from_secs(5)).await;
            Json(serde_json::json!({"data": [], "has_more": false}))
        }

        let app = Router::new().route("/api/sessions", get(slow_sessions));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let temp = tempfile::tempdir().unwrap();
        let now = unix_now_seconds();
        let snapshot_path = temp.path().join(INSIGHTS_SNAPSHOT_DB);
        persist_insights_snapshot(
            &snapshot_path,
            now - INSIGHTS_SNAPSHOT_INTERVAL.as_secs_f64() - 1.0,
            &[serde_json::json!({
                "id": "persisted-session",
                "source": "telegram",
                "model": "gpt-5.6-sol",
                "started_at": now - 60.0,
                "input_tokens": 100
            })],
        )
        .unwrap();
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));
        {
            let mut cache = state.model_price_cache.write().await;
            cache.fetched_at = Some(std::time::Instant::now());
            cache.body = Some(serde_json::json!({}));
        }

        let response = timeout(
            Duration::from_millis(500),
            insights_usage(
                State(state),
                Query(InsightsUsageQuery {
                    period: Some(7),
                    days: None,
                    tz_offset: Some(0),
                    refresh: None,
                }),
            ),
        )
        .await
        .expect("persisted Insights data should return before collection finishes");

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn insights_reuses_a_fresh_snapshot_without_starting_collection() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        async fn counted_sessions(
            State(calls): State<Arc<AtomicUsize>>,
        ) -> Json<serde_json::Value> {
            calls.fetch_add(1, Ordering::SeqCst);
            Json(serde_json::json!({"data": [], "has_more": false}))
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/api/sessions", get(counted_sessions))
            .with_state(calls.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let temp = tempfile::tempdir().unwrap();
        let now = unix_now_seconds();
        persist_insights_snapshot(
            &temp.path().join(INSIGHTS_SNAPSHOT_DB),
            now,
            &[serde_json::json!({
                "id": "fresh-session",
                "source": "telegram",
                "model": "gpt-5.6-sol",
                "started_at": now,
                "input_tokens": 100
            })],
        )
        .unwrap();
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));
        {
            let mut cache = state.model_price_cache.write().await;
            cache.fetched_at = Some(std::time::Instant::now());
            cache.body = Some(serde_json::json!({}));
        }

        let response = insights_usage(
            State(state),
            Query(InsightsUsageQuery {
                period: Some(7),
                days: None,
                tz_offset: Some(0),
                refresh: None,
            }),
        )
        .await;
        sleep(Duration::from_millis(100)).await;

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn explicit_insights_refresh_collects_even_when_the_snapshot_is_fresh() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        async fn counted_sessions(
            State(calls): State<Arc<AtomicUsize>>,
        ) -> Json<serde_json::Value> {
            calls.fetch_add(1, Ordering::SeqCst);
            Json(serde_json::json!({
                "data": [{
                    "id": "fresh-session",
                    "source": "telegram",
                    "model": "gpt-5.6-sol",
                    "started_at": unix_now_seconds(),
                    "input_tokens": 200
                }],
                "has_more": false
            }))
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/api/sessions", get(counted_sessions))
            .with_state(calls.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let temp = tempfile::tempdir().unwrap();
        let now = unix_now_seconds();
        persist_insights_snapshot(
            &temp.path().join(INSIGHTS_SNAPSHOT_DB),
            now,
            &[serde_json::json!({
                "id": "fresh-session",
                "source": "telegram",
                "model": "gpt-5.6-sol",
                "started_at": now,
                "input_tokens": 100
            })],
        )
        .unwrap();
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));
        {
            let mut cache = state.model_price_cache.write().await;
            cache.fetched_at = Some(std::time::Instant::now());
            cache.body = Some(serde_json::json!({}));
        }

        let response = insights_usage(
            State(state),
            Query(InsightsUsageQuery {
                period: Some(7),
                days: None,
                tz_offset: Some(0),
                refresh: Some(true),
            }),
        )
        .await;
        sleep(Duration::from_millis(100)).await;

        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value = serde_json::from_slice(
            &to_bytes(response.into_body(), usize::MAX).await.unwrap(),
        )
        .unwrap();
        assert_eq!(body["totals"]["input"], 200);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn concurrent_insights_requests_share_one_snapshot_collection() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        async fn counted_slow_sessions(
            State(calls): State<Arc<AtomicUsize>>,
        ) -> Json<serde_json::Value> {
            calls.fetch_add(1, Ordering::SeqCst);
            sleep(Duration::from_millis(150)).await;
            Json(serde_json::json!({"data": [], "has_more": false}))
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/api/sessions", get(counted_slow_sessions))
            .with_state(calls.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let temp = tempfile::tempdir().unwrap();
        let now = unix_now_seconds();
        persist_insights_snapshot(
            &temp.path().join(INSIGHTS_SNAPSHOT_DB),
            now - INSIGHTS_SNAPSHOT_INTERVAL.as_secs_f64() - 1.0,
            &[serde_json::json!({
                "id": "stale-session",
                "source": "telegram",
                "model": "gpt-5.6-sol",
                "started_at": now,
                "input_tokens": 100
            })],
        )
        .unwrap();
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));
        {
            let mut cache = state.model_price_cache.write().await;
            cache.fetched_at = Some(std::time::Instant::now());
            cache.body = Some(serde_json::json!({}));
        }

        let first = insights_usage(
            State(state.clone()),
            Query(InsightsUsageQuery {
                period: Some(1),
                days: None,
                tz_offset: Some(0),
                refresh: None,
            }),
        );
        let second = insights_usage(
            State(state),
            Query(InsightsUsageQuery {
                period: Some(30),
                days: None,
                tz_offset: Some(0),
                refresh: None,
            }),
        );
        let (first, second) = tokio::join!(first, second);
        sleep(Duration::from_millis(250)).await;

        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(second.status(), StatusCode::OK);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
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

    #[test]
    fn insights_reads_per_model_usage_rows_instead_of_session_totals() {
        let temp = tempfile::tempdir().unwrap();
        let state_db = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&state_db).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                 id TEXT PRIMARY KEY,
                 source TEXT NOT NULL,
                 model TEXT,
                 billing_provider TEXT,
                 billing_base_url TEXT,
                 model_config TEXT,
                 started_at REAL NOT NULL,
                 end_reason TEXT,
                 archived INTEGER DEFAULT 0
             );
             CREATE TABLE messages (id INTEGER PRIMARY KEY, session_id TEXT NOT NULL);
             CREATE TABLE session_model_usage (
                 session_id TEXT NOT NULL,
                 model TEXT NOT NULL,
                 billing_provider TEXT NOT NULL DEFAULT '',
                 billing_base_url TEXT NOT NULL DEFAULT '',
                 billing_mode TEXT NOT NULL DEFAULT '',
                 task TEXT NOT NULL DEFAULT '',
                 api_call_count INTEGER NOT NULL DEFAULT 0,
                 input_tokens INTEGER NOT NULL DEFAULT 0,
                 output_tokens INTEGER NOT NULL DEFAULT 0,
                 cache_read_tokens INTEGER NOT NULL DEFAULT 0,
                 cache_write_tokens INTEGER NOT NULL DEFAULT 0,
                 reasoning_tokens INTEGER NOT NULL DEFAULT 0,
                 estimated_cost_usd REAL NOT NULL DEFAULT 0,
                 actual_cost_usd REAL NOT NULL DEFAULT 0,
                 PRIMARY KEY(session_id, model, billing_provider, billing_base_url, billing_mode, task)
             );
             INSERT INTO sessions(id, source, model, billing_provider, started_at)
             VALUES('s1', 'telegram', 'grok-4.5', 'deepseek', 1785900000);
             INSERT INTO messages(id, session_id) VALUES(1, 's1');
             INSERT INTO session_model_usage(
                 session_id, model, billing_provider, billing_base_url, input_tokens, output_tokens, cache_read_tokens
             ) VALUES
                 ('s1', 'grok-4.5', 'xai-oauth', 'https://api.x.ai/v1', 1000, 100, 9000),
                 ('s1', 'grok-4.5', 'opencode-go', 'https://opencode.ai/zen/go/v1', 2000, 200, 8000);",
        )
        .unwrap();

        let (rows, high_water) = fetch_changed_sessions_for_insights(&state_db, 0).unwrap();
        assert_eq!(high_water, 1);
        assert_eq!(rows.len(), 2);
        assert_eq!(
            rows.iter().find(|row| row["provider"] == "xai-oauth").unwrap()["input_tokens"],
            1000
        );
        assert_eq!(
            rows.iter().find(|row| row["provider"] == "opencode-go").unwrap()["input_tokens"],
            2000
        );
        assert!(rows.iter().all(|row| row["root_session_id"] == "s1"));
    }
