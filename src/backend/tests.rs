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
    fn session_token_is_jwt() {
        let token = make_session_token("webui-secret");
        let parts: Vec<_> = token.split('.').collect();

        assert_eq!(parts.len(), 3);
        let header: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
        let claims: serde_json::Value =
            serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();

        assert_eq!(header["alg"], "HS256");
        assert_eq!(header["typ"], "JWT");
        assert_eq!(claims["iss"], "yahu");
        let iat = claims["iat"].as_u64().unwrap();
        let exp = claims["exp"].as_u64().unwrap();
        assert_eq!(exp - iat, SESSION_TTL);
    }

    #[test]
    fn old_session_token_gets_refresh_cookie() {
        let old_iat = now_secs().saturating_sub(SESSION_REFRESH_AFTER + 1);
        let old_token = make_session_token_at("webui-secret", old_iat);

        let cookie = session_token_refresh_cookie(&old_token, "webui-secret").unwrap();
        let refreshed_token = cookie
            .strip_prefix(&format!("{}=", SESSION_COOKIE))
            .unwrap()
            .split(';')
            .next()
            .unwrap();

        assert!(cookie.contains(&format!("Max-Age={}", SESSION_TTL)));
        assert!(verify_session_token(refreshed_token, "webui-secret"));
        assert!(session_token_refresh_cookie(refreshed_token, "webui-secret").is_none());
    }

    #[test]
    fn fresh_session_token_does_not_refresh() {
        let token = make_session_token("webui-secret");

        assert!(session_token_refresh_cookie(&token, "webui-secret").is_none());
    }

    #[test]
    fn login_html_uses_larger_mobile_input_text() {
        let html = login_html("");

        assert!(html.contains("<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">"));
        assert!(html.contains("@media(max-width:760px)"));
        assert!(html.contains("*,*::before,*::after{box-sizing:border-box}"));
        assert!(html.contains("input{height:48px;font-size:18px}"));
        assert!(html.contains("button{height:48px;font-size:16px}"));
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

    #[test]
    fn context_window_usage_counts_from_latest_compression_summary() {
        let messages = vec![
            serde_json::json!({"id": 1, "role": "user", "content": "old history", "token_count": 1000}),
            serde_json::json!({"id": 2, "role": "assistant", "content": "Context compressed summary: old history", "token_count": 75, "compression_summary": true}),
            serde_json::json!({"id": 3, "role": "user", "content": "new turn", "tokenCount": 8}),
            serde_json::json!({"id": 4, "role": "assistant", "content": "reply without token"}),
        ];

        let usage = estimate_context_window_usage(&messages);

        assert_eq!(usage.used, 75 + 8 + rough_context_token_count("reply without token"));
        assert!(usage.approximate);
        assert!(usage.compressed);
        assert_eq!(usage.compression_boundary_id, Some(serde_json::json!(2)));
        assert_eq!(usage.counted_messages, 3);
        assert_eq!(usage.total_messages, 4);
    }

    #[test]
    fn context_window_usage_is_exact_when_every_counted_message_has_tokens() {
        let messages = vec![
            serde_json::json!({"id": 1, "role": "user", "content": "hello", "token_count": 6}),
            serde_json::json!({"id": 2, "role": "assistant", "content": "world", "tokenCount": 9}),
        ];

        let usage = estimate_context_window_usage(&messages);

        assert_eq!(usage.used, 15);
        assert!(!usage.approximate);
        assert!(!usage.compressed);
        assert_eq!(usage.compression_boundary_id, None);
        assert_eq!(usage.counted_messages, 2);
        assert_eq!(usage.total_messages, 2);
    }

    #[test]
    fn memory_payload_reads_profile_scoped_memory_files() {
        let temp = tempfile::tempdir().unwrap();
        let memories = temp.path().join("memories");
        std::fs::create_dir_all(&memories).unwrap();
        std::fs::write(memories.join("MEMORY.md"), "agent note\n§\nproject convention").unwrap();
        std::fs::write(memories.join("USER.md"), "user preference").unwrap();

        let payload = read_memory_payload_from_files(temp.path()).unwrap();

        assert_eq!(payload.memory, "agent note\n§\nproject convention");
        assert_eq!(payload.user, "user preference");
    }

    #[test]
    fn memory_payload_writes_files_with_hermes_style_locks() {
        let temp = tempfile::tempdir().unwrap();
        let payload = MemoryPayload {
            memory: "agent note".to_string(),
            user: "user preference".to_string(),
        };

        write_memory_payload_to_files(temp.path(), &payload).unwrap();

        let memories = temp.path().join("memories");
        assert_eq!(
            std::fs::read_to_string(memories.join("MEMORY.md")).unwrap(),
            "agent note"
        );
        assert_eq!(
            std::fs::read_to_string(memories.join("USER.md")).unwrap(),
            "user preference"
        );
        assert!(memories.join("MEMORY.md.lock").exists());
        assert!(memories.join("USER.md.lock").exists());
    }

    fn test_app_state(api_url: String, root: &Path) -> AppState {
        let (updates, _) = broadcast::channel::<String>(1);
        let (deletes, _) = broadcast::channel::<String>(1);
        let (chat_streams, _) = broadcast::channel::<String>(1);
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
            chat_streams,
            active_chat_streams: Arc::new(RwLock::new(HashMap::new())),
            model_cache: Arc::new(RwLock::new(ModelCache::default())),
            model_price_cache: Arc::new(RwLock::new(ModelCache::default())),
            models_dev_url: "https://models.dev/api.json".to_string(),
            github_repo: String::new(),
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

    #[test]
    fn session_watch_keeps_only_recent_message_fingerprints() {
        let items: Vec<_> = (0..(API_MESSAGE_WATCH_WINDOW + 25))
            .map(|id| serde_json::json!({"id": id, "role": "assistant", "content": format!("message {id}")}))
            .collect();

        let watched = watch_message_window(items.clone());
        let state = session_message_watch_state(&watched);

        assert_eq!(watched.len(), API_MESSAGE_WATCH_WINDOW);
        assert_eq!(watched[0]["id"], 25);
        assert_eq!(state.fingerprints.len(), API_MESSAGE_WATCH_WINDOW);
        assert!(!state.fingerprints.contains_key(&0));
        assert!(state.fingerprints.contains_key(&((API_MESSAGE_WATCH_WINDOW + 24) as i64)));
    }

    #[test]
    fn chat_stream_broadcast_ring_is_small_because_payloads_are_full_snapshots() {
        assert!(CHAT_STREAM_BROADCAST_CAPACITY <= 32);
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

        let body = aggregate_usage_insights_with_prices(&rows, ts, &catalog, 1);

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

}
