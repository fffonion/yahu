#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_token_round_trip_accepts_current_key() {
        let token = make_session_token("webui-secret");

        assert!(verify_session_token(&token, "webui-secret"));
        assert!(!verify_session_token(&token, "other-secret"));
    }

    #[test]
    fn frontmatter_value_reads_yaml_after_opening_blank_line() {
        let text =
            "---\nname: hermes-dashboard-webui\ndescription: \"Dashboard UI\"\n---\n# Body\n";

        assert_eq!(
            frontmatter_value(text, "name"),
            Some("hermes-dashboard-webui".to_string())
        );
        assert_eq!(
            frontmatter_value(text, "description"),
            Some("Dashboard UI".to_string())
        );
    }

    #[test]
    fn workspace_path_rejects_parent_escape() {
        let root = std::env::temp_dir().join(format!(
            "hermes-webui-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();

        let err = resolve_workspace_path(&root, "../outside.txt").unwrap_err();

        assert!(err.to_string().contains("invalid workspace path"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_path_resolves_child_file() {
        let root = std::env::temp_dir().join(format!(
            "hermes-webui-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos()
        ));
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("note.txt"), "hello").unwrap();

        let resolved = resolve_workspace_path(&root, "nested/note.txt").unwrap();

        assert_eq!(resolved, nested.join("note.txt").canonicalize().unwrap());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_destination_keeps_rename_inside_parent_directory() {
        let root = std::env::temp_dir().join(format!(
            "hermes-webui-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos()
        ));
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("old.txt"), "hello").unwrap();

        let target = workspace_destination_path(&root, "nested/old.txt", "new.txt").unwrap();

        assert_eq!(target, nested.join("new.txt"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_destination_rejects_path_like_new_names() {
        let root = std::env::temp_dir().join(format!(
            "hermes-webui-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("old.txt"), "hello").unwrap();

        let err = workspace_destination_path(&root, "old.txt", "../bad.txt").unwrap_err();

        assert!(
            err.to_string()
                .contains("new name must be a single file name")
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn proxy_header_filter_removes_browser_origin_headers() {
        assert!(!should_forward_proxy_header("origin"));
        assert!(!should_forward_proxy_header("referer"));
        assert!(!should_forward_proxy_header("sec-fetch-site"));
        assert!(!should_forward_proxy_header("sec-ch-ua-platform"));
        assert!(!should_forward_proxy_header("cookie"));
        assert!(should_forward_proxy_header("content-type"));
        assert!(should_forward_proxy_header("accept"));
    }

    #[test]
    fn zip_store_contains_selected_image_file() {
        let bytes =
            build_zip_store(&[("sample.png".to_string(), b"image-bytes".to_vec())]).unwrap();

        assert!(bytes.starts_with(b"PK\x03\x04"));
        assert!(
            bytes
                .windows("sample.png".len())
                .any(|w| w == b"sample.png")
        );
        assert!(bytes.ends_with(&[0, 0]));
    }

    #[test]
    fn image_filename_rejects_url_and_header_control_characters() {
        for name in [
            "bad?name.png",
            "bad#name.png",
            "bad%name.png",
            "bad\rname.png",
            "bad\nname.png",
            "bad\tname.png",
        ] {
            assert!(!is_safe_filename(name), "{name} should be rejected");
        }
        assert!(is_safe_filename("sample-image_01.png"));
    }

    #[test]
    fn image_file_urls_percent_encode_filename_segments() {
        let root = std::env::temp_dir().join(format!(
            "hermes-webui-image-url-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let filename = "sample image.png";
        std::fs::write(root.join(filename), b"image-bytes").unwrap();

        let metadata = file_metadata(&root, filename, false).unwrap();

        assert!(metadata.url.starts_with("/image-files/sample%20image.png?v="));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn chat_upload_data_url_decodes_base64_payload() {
        let bytes = decode_chat_upload_data_url("data:application/octet-stream;base64,AAFiYw==").unwrap();

        assert_eq!(bytes, b"\0\x01bc");
    }

    #[test]
    fn chat_upload_filename_sanitizer_keeps_saved_file_inside_cache() {
        let name = sanitize_chat_upload_filename("../bad/name with 控制?.pdf");

        assert_eq!(name, "name with ___.pdf");
        assert!(!name.contains('/'));
        assert!(!name.contains(".."));
    }

    fn test_app_state(api_url: String, root: &Path) -> AppState {
        let (updates, _) = broadcast::channel::<String>(1);
        let (deletes, _) = broadcast::channel::<String>(1);
        AppState {
            client: reqwest::Client::new(),
            api_url,
            api_key: None,
            auth_key: None,
            insecure: true,
            workspace: root.to_path_buf(),
            hermes_home: root.to_path_buf(),
            image_dir: root.to_path_buf(),
            updates,
            deletes,
            model_cache: Arc::new(RwLock::new(ModelCache::default())),
            model_price_cache: Arc::new(RwLock::new(ModelCache::default())),
            models_dev_url: "https://models.dev/api.json".to_string(),
            fx_cache: Arc::new(RwLock::new(ModelCache::default())),
            fx_url: "https://open.er-api.com/v6/latest/USD".to_string(),
        }
    }

    #[tokio::test]
    async fn session_search_uses_api_server_list_endpoint_without_state_db() {
        use std::collections::HashMap;

        async fn api_sessions(
            Query(query): Query<HashMap<String, String>>,
        ) -> Json<serde_json::Value> {
            assert_eq!(
                query.get("include_children").map(String::as_str),
                Some("false")
            );
            assert_eq!(query.get("offset").map(String::as_str), Some("0"));
            Json(serde_json::json!({
                "object": "list",
                "data": [
                    {"id":"s1","source":"telegram","model":"minimax/m3","title":"MiniMax billing","preview":"token cache math","started_at":1.0,"message_count":1},
                    {"id":"tool1","source":"tool","model":"minimax/m3","title":"Tool internal","preview":"cache","started_at":2.0,"message_count":1},
                    {"id":"s2","source":"api_server","model":"gpt-5.5","title":"Other","preview":"unrelated","started_at":3.0,"message_count":1}
                ],
                "has_more": false
            }))
        }

        let app = Router::new().route("/api/sessions", get(api_sessions));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state(format!("http://{addr}"), temp.path());

        let rows = fetch_sessions_from_api_server(&state, "cache", 10)
            .await
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "s1");
        assert_eq!(rows[0]["model"], "minimax/m3");
        assert_eq!(rows[0]["preview"], "token cache math");
    }

    #[test]
    fn session_watch_reads_api_server_data_and_legacy_messages_shapes() {
        let data_body = serde_json::json!({
            "object": "list",
            "data": [{"id": 1, "role": "user", "content": "hi"}],
        });
        let legacy_body = serde_json::json!({
            "session_id": "s1",
            "messages": [{"id": 2, "role": "assistant", "content": "hello"}],
        });

        assert_eq!(session_message_items(&data_body).len(), 1);
        assert_eq!(session_message_items(&legacy_body).len(), 1);
        assert_eq!(session_message_items(&legacy_body)[0]["id"], 2);
    }

    #[test]
    fn session_watch_emits_all_new_messages_in_id_order() {
        let items = vec![
            serde_json::json!({"id": 12, "role": "assistant", "content": "second"}),
            serde_json::json!({"id": 10, "role": "user", "content": "old"}),
            serde_json::json!({"id": 11, "role": "user", "content": "first"}),
        ];

        let (new_items, last_id) = unseen_session_messages(&items, 10);

        assert_eq!(last_id, 12);
        assert_eq!(new_items.len(), 2);
        assert_eq!(new_items[0]["id"], 11);
        assert_eq!(new_items[1]["id"], 12);
    }

    #[test]
    fn session_watch_emits_streaming_updates_for_existing_message_ids() {
        let initial = vec![serde_json::json!({"id": 20, "role": "assistant", "content": ""})];
        let mut state = session_message_watch_state(&initial);
        let updated = vec![serde_json::json!({"id": 20, "role": "assistant", "content": "partial stream"})];

        let new_items = changed_session_messages(&updated, &mut state);

        assert_eq!(new_items.len(), 1);
        assert_eq!(new_items[0]["id"], 20);
        assert_eq!(new_items[0]["content"], "partial stream");
        assert_eq!(state.last_id, 20);
    }

    #[tokio::test]
    async fn session_search_uses_api_server_messages_when_list_preview_does_not_match() {
        use std::collections::HashMap;

        async fn api_sessions(
            Query(query): Query<HashMap<String, String>>,
        ) -> Json<serde_json::Value> {
            assert_eq!(
                query.get("include_children").map(String::as_str),
                Some("false")
            );
            Json(serde_json::json!({
                "object": "list",
                "data": [
                    {"id":"s1","source":"telegram","model":"minimax/m3","title":"MiniMax billing","preview":"first prompt","started_at":1.0,"message_count":2},
                    {"id":"s2","source":"api_server","model":"gpt-5.5","title":"Other","preview":"unrelated","started_at":3.0,"message_count":1}
                ],
                "has_more": false
            }))
        }

        async fn api_messages(AxumPath(session_id): AxumPath<String>) -> Json<serde_json::Value> {
            let data = if session_id == "s1" {
                serde_json::json!([{ "id": 1, "role": "user", "content": "please do token cache math" }])
            } else {
                serde_json::json!([{ "id": 2, "role": "user", "content": "unrelated" }])
            };
            Json(serde_json::json!({"object":"list","data":data}))
        }

        let app = Router::new()
            .route("/api/sessions", get(api_sessions))
            .route("/api/sessions/{session_id}/messages", get(api_messages));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state(format!("http://{addr}"), temp.path());

        let rows = fetch_sessions_from_api_server(&state, "cache", 10)
            .await
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "s1");
    }

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

        let body = aggregate_usage_insights_with_prices(&rows, ts, &ModelPriceCatalog::new());

        assert_eq!(body["totals"]["input"], 150);
        assert_eq!(body["totals"]["output"], 30);
        assert_eq!(body["totals"]["cache_read"], 900);
        assert_eq!(body["models"][0]["model"], "minimax/m3");
        assert_eq!(body["periods"].as_array().unwrap().len(), 3);
        let one_day = body["periods"].as_array().unwrap().iter().find(|item| item["days"] == 1).unwrap();
        assert_eq!(one_day["totals"]["input"], 100);
        assert!(one_day["totals"]["cache_hit_rate"].as_f64().unwrap() > 0.89);
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

        let body = aggregate_usage_insights_with_prices(&rows, ts, &catalog);
        let totals = &body["totals"];

        assert!((totals["estimated_cost_usd"].as_f64().unwrap() - 3.84).abs() < 0.000001);
        assert!((totals["cost_usd"].as_f64().unwrap() - 43.92).abs() < 0.000001);
        assert_eq!(totals["actual_cost_usd"], 42.0);
        assert_eq!(totals["unpriced_tokens"], 600);
        let one_day = body["periods"].as_array().unwrap().iter().find(|item| item["days"] == 1).unwrap();
        assert!((one_day["totals"]["cost_usd"].as_f64().unwrap() - 43.92).abs() < 0.000001);
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

        let body = aggregate_usage_insights_with_prices(&rows, ts, &catalog);

        assert_eq!(body["totals"]["cost_usd"], 0.0);
        assert_eq!(body["totals"]["unpriced_tokens"], 60);
    }

    #[tokio::test]
    async fn insights_fetches_models_dev_price_catalog_from_configured_backend_url() {
        async fn models_dev(headers: HeaderMap) -> Json<serde_json::Value> {
            assert!(
                headers
                    .get(header::USER_AGENT)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .starts_with("yahu/")
            );
            Json(serde_json::json!({
                "minimax": {
                    "id": "minimax",
                    "models": {
                        "MiniMax-M3": {
                            "id": "MiniMax-M3",
                            "cost": {"input": 0.6, "output": 2.4, "cache_read": 0.12}
                        }
                    }
                }
            }))
        }

        let app = Router::new().route("/api.json", get(models_dev));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let temp = tempfile::tempdir().unwrap();
        let mut state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());
        state.models_dev_url = format!("http://{addr}/api.json");

        let catalog = fetch_models_dev_price_catalog(&state).await.unwrap();
        let price = model_price_for_model(&catalog, "minimax/m3").unwrap();

        assert!((price.estimate(1_000_000, 100_000, 9_000_000, 0) - 1.92).abs() < 0.000001);
    }

    #[tokio::test]
    async fn insights_fetches_fx_rates_from_configured_backend_url() {
        async fn fx(headers: HeaderMap) -> Json<serde_json::Value> {
            assert!(
                headers
                    .get(header::USER_AGENT)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .starts_with("yahu/")
            );
            Json(serde_json::json!({"base":"USD","date":"2026-06-09","rates":{"CNY":6.77,"JPY":160.16}}))
        }

        let app = Router::new().route("/latest", get(fx));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let temp = tempfile::tempdir().unwrap();
        let mut state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());
        state.fx_url = format!("http://{addr}/latest");

        let body = fetch_fx_rates(&state).await.unwrap();

        assert_eq!(body["object"], "yahu.insights.fx");
        assert_eq!(body["base"], "USD");
        assert_eq!(body["rates"]["USD"], 1.0);
        assert_eq!(body["rates"]["CNY"], 6.77);
        assert_eq!(body["rates"]["JPY"], 160.16);
    }
}
