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
    fn turn_duration_injection_skips_synthetic_subsecond_history_deltas() {
        let mut messages = vec![
            serde_json::json!({"id": 1, "role": "user", "content": "prompt", "timestamp": 1000.0}),
            serde_json::json!({"id": 2, "role": "assistant", "content": "synthetic replay row", "reasoning": "thinking", "timestamp": 1000.0001}),
            serde_json::json!({"id": 3, "role": "assistant", "content": "real response", "reasoning": "thinking", "timestamp": 1012.0}),
        ];

        inject_turn_durations(&mut messages);

        assert!(messages[1].get("duration_ms").is_none(), "subsecond synthetic history deltas must not render as 0ms");
        assert_eq!(messages[2]["duration_ms"].as_f64().unwrap(), 12_000.0);
    }

    #[test]
    fn turn_duration_injection_resets_at_history_gap() {
        let mut messages = vec![
            serde_json::json!({"id": -2, "role": "user", "content": "recovered prompt", "timestamp": 1000.0}),
            serde_json::json!({
                "id": -1,
                "role": "system",
                "content": "history unavailable",
                "timestamp": 2000.0,
                "history_gap": {"after": 1000.0, "before": 2000.0}
            }),
            serde_json::json!({"id": 1, "role": "assistant", "content": "retained suffix reply", "timestamp": 2001.0}),
            serde_json::json!({"id": 2, "role": "user", "content": "suffix prompt", "timestamp": 2010.0}),
            serde_json::json!({"id": 3, "role": "assistant", "content": "tool setup", "timestamp": 2011.0, "tool_calls": [{"id": "call-1"}]}),
            serde_json::json!({"id": 4, "role": "tool", "content": "tool result", "timestamp": 2012.0}),
            serde_json::json!({"id": 5, "role": "assistant", "content": "suffix final", "timestamp": 2020.0}),
        ];

        inject_turn_durations(&mut messages);

        assert!(
            messages[2].get("duration_ms").is_none(),
            "the retained suffix must not inherit a user timestamp across a history gap"
        );
        assert_eq!(messages[4]["duration_ms"].as_f64().unwrap(), 1_000.0);
        assert_eq!(messages[6]["duration_ms"].as_f64().unwrap(), 10_000.0);
    }

    #[test]
    fn request_dump_messages_preserve_valid_explicit_time_and_gap() {
        let first_timestamp = 1_783_486_063.0;
        let gap_before = 1_783_662_592.0;
        let dump = serde_json::json!({
            "timestamp": "2026-07-11T05:19:53.762060",
            "session_id": "s1",
            "request": {"body": {"input": [
                {
                    "role": "user",
                    "content": [{"type": "input_text", "text": "verified first prompt"}],
                    "timestamp": first_timestamp
                },
                {
                    "role": "system",
                    "content": "History coverage gap",
                    "timestamp": first_timestamp + 1.0,
                    "history_gap": {"after": first_timestamp, "before": gap_before}
                },
                {"role": "assistant", "content": "later recovered reply"}
            ]}}
        });

        let (_, messages) = request_dump_messages("s1", &dump).unwrap();

        assert_eq!(messages[0]["timestamp"], first_timestamp);
        assert_eq!(messages[1]["history_gap"]["after"], first_timestamp);
        assert_eq!(messages[1]["history_gap"]["before"], gap_before);
        assert_eq!(messages[1]["content"], "History coverage gap");
    }

    #[test]
    fn request_dump_messages_convert_anthropic_messages_in_order() {
        let dump = serde_json::json!({
            "timestamp": "2026-07-20T16:19:44.784375",
            "session_id": "s1",
            "request": {"body": {"messages": [
                {"role": "user", "content": "older prompt"},
                {"role": "assistant", "content": [
                    {"type": "thinking", "thinking": "   ", "signature": "blank-opaque-signature"},
                    {"type": "thinking", "thinking": "provider thought", "signature": "opaque-signature"},
                    {"type": "tool_use", "id": "tool-1", "name": "terminal", "input": {"command": "pwd", "password": "secret-value"}}
                ]},
                {"role": "user", "content": [
                    {"type": "tool_result", "tool_use_id": "tool-1", "content": [{"type": "text", "text": "tool output"}]}
                ]},
                {"role": "assistant", "content": [{"type": "text", "text": "older answer"}]}
            ]}}
        });

        let (_, messages) = request_dump_messages("s1", &dump).expect("Anthropic messages should be recoverable");
        let roles: Vec<_> = messages.iter().map(|message| message["role"].as_str().unwrap_or("")).collect();
        assert_eq!(roles, vec!["user", "assistant", "assistant", "tool", "assistant"]);
        assert_eq!(messages[0]["content"], "older prompt");
        assert_eq!(messages[1]["reasoning"], "provider thought");
        assert_eq!(messages[2]["tool_calls"][0]["function"]["name"], "terminal");
        assert_eq!(messages[3]["tool_name"], "terminal");
        assert_eq!(messages[3]["content"], "tool output");
        assert_eq!(messages[4]["content"], "older answer");
        let serialized = serde_json::to_string(&messages).unwrap();
        assert!(!serialized.contains("opaque-signature"));
        assert!(!serialized.contains("secret-value"));
        assert!(serialized.contains("[REDACTED]"));
    }

    #[test]
    fn session_preview_removes_gateway_sender_prefix() {
        assert_eq!(
            session_preview_from_raw_content(
                "[Alliumcepa Triplef|1698432746]\n会话列表只显示消息本身",
            ),
            "会话列表只显示消息本身",
        );
    }

    #[tokio::test]
    async fn static_app_shell_assets_are_not_http_cached() {
        let root = static_assets("/".parse::<Uri>().unwrap()).await;
        assert_eq!(
            root.headers().get(header::CACHE_CONTROL).and_then(|value| value.to_str().ok()),
            Some("no-store")
        );

        let sw = static_assets("/sw.js".parse::<Uri>().unwrap()).await;
        assert_eq!(
            sw.headers().get(header::CACHE_CONTROL).and_then(|value| value.to_str().ok()),
            Some("no-store")
        );
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

    #[tokio::test]
    async fn workspace_binary_preview_reads_only_the_first_mebibyte() {
        let temp = tempfile::tempdir().unwrap();
        let bytes = vec![0x5a; WORKSPACE_BINARY_PREVIEW_LIMIT + 257];
        std::fs::write(temp.path().join("sample.bin"), &bytes).unwrap();
        let state = Arc::new(test_app_state("http://127.0.0.1:1".to_string(), temp.path()));

        let response = workspace_file(
            State(state),
            Query(WorkspaceQuery {
                path: Some("sample.bin".to_string()),
                download: None,
                preview: Some("1".to_string()),
            }),
        )
        .await;
        let file_size = response.headers()["x-yahu-file-size"].to_str().unwrap().to_string();
        let truncated = response.headers()["x-yahu-preview-truncated"].to_str().unwrap().to_string();
        let body = to_bytes(response.into_body(), WORKSPACE_BINARY_PREVIEW_LIMIT + 1).await.unwrap();

        assert_eq!(body.len(), WORKSPACE_BINARY_PREVIEW_LIMIT);
        assert!(body.iter().all(|byte| *byte == 0x5a));
        assert_eq!(file_size, bytes.len().to_string());
        assert_eq!(truncated, "1");
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
    fn chat_run_header_filter_drops_source_content_type_before_reqwest_json() {
        assert!(!should_forward_chat_run_header("content-type"));
        assert!(should_forward_chat_run_header("accept"));
    }

    #[test]
    fn chat_run_content_type_normalization_replaces_all_values() {
        let mut headers = HeaderMap::new();
        headers.append(header::CONTENT_TYPE, "application/json".parse().unwrap());
        headers.append(
            header::CONTENT_TYPE,
            "application/json; charset=utf-8".parse().unwrap(),
        );

        normalize_chat_run_content_type(&mut headers);

        let values = headers
            .get_all(header::CONTENT_TYPE)
            .iter()
            .map(|value| value.to_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["application/json"]);
    }

    #[test]
    fn cron_run_proxy_waits_for_upstream_scheduler_tick() {
        let proxy_source = include_str!("../proxy.rs");

        assert!(!proxy_source.contains("spawn_cron_tick_after_manual_run"));
        assert!(!proxy_source.contains("should_kick_cron_tick_after_proxy"));
        assert!(!proxy_source.contains("cron.scheduler"));
        assert!(!proxy_source.contains("tick(verbose=True"));
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
    fn chat_media_path_allows_cache_files_and_blocks_credential_paths() {
        let root = std::env::temp_dir().join(format!(
            "hermes-webui-media-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos()
        ));
        let cache_dir = root.join("cache/images");
        std::fs::create_dir_all(&cache_dir).unwrap();
        let image = cache_dir.join("preview.png");
        std::fs::write(&image, b"image").unwrap();
        std::fs::write(root.join(".env"), b"SECRET=1").unwrap();

        let resolved = resolve_chat_media_path(image.to_str().unwrap(), &root).unwrap();
        assert_eq!(resolved, image.canonicalize().unwrap());
        assert!(resolve_chat_media_path(root.join(".env").to_str().unwrap(), &root).is_err());
        assert!(resolve_chat_media_path("relative.png", &root).is_err());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn chat_media_path_rejects_system_denylist() {
        assert!(resolve_chat_media_path("/etc/passwd", &std::env::temp_dir()).is_err());
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

    #[tokio::test]
    async fn context_window_usage_starts_after_latest_compression_split() {
        async fn api_session(AxumPath(session_id): AxumPath<String>) -> Json<serde_json::Value> {
            let (parent, end_reason) = match session_id.as_str() {
                "child" => (Some("root"), None),
                "root" => (None, Some("compression")),
                _ => (None, None),
            };
            Json(serde_json::json!({
                "object": "hermes.session",
                "session": {"id": session_id, "parent_session_id": parent, "end_reason": end_reason}
            }))
        }

        async fn api_messages(AxumPath(session_id): AxumPath<String>) -> Json<serde_json::Value> {
            let (id, tokens) = match session_id.as_str() {
                "root" => (1, 1000),
                _ => (2, 100),
            };
            Json(serde_json::json!({"object":"list","data":[{
                "id": id,
                "session_id": session_id,
                "role":"user",
                "content": format!("{session_id} message"),
                "token_count": tokens
            }]}))
        }

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let api_app = Router::new()
            .route("/api/sessions/{session_id}", get(api_session))
            .route("/api/sessions/{session_id}/messages", get(api_messages));
        tokio::spawn(async move { axum::serve(listener, api_app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));

        let resp = chat_context_window(State(state), AxumPath("child".to_string())).await;
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let usage: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(usage["used"], 100);
        assert_eq!(usage["counted_messages"], 1);
        assert_eq!(usage["total_messages"], 2);
        assert_eq!(usage["compressed"], true);
        assert_eq!(usage["compression_boundary_id"], serde_json::json!(1));
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

    #[test]
    fn skills_collector_skips_archive_directories() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("skills");
        let active = root.join("productivity/live-skill");
        let archived = root.join(".archive/old-skill");
        let nested_archived = root.join("productivity/.archive/nested-old-skill");
        std::fs::create_dir_all(&active).unwrap();
        std::fs::create_dir_all(&archived).unwrap();
        std::fs::create_dir_all(&nested_archived).unwrap();
        std::fs::write(active.join("SKILL.md"), "---\nname: live-skill\ndescription: Live\n---\n").unwrap();
        std::fs::write(archived.join("SKILL.md"), "---\nname: old-skill\ndescription: Old\n---\n").unwrap();
        std::fs::write(nested_archived.join("SKILL.md"), "---\nname: nested-old-skill\ndescription: Nested old\n---\n").unwrap();

        let mut found = HashMap::<String, (SkillInfo, PathBuf)>::new();
        collect_skill_dirs(&root, &root, &HashSet::new(), &mut found);

        assert!(found.contains_key("live-skill"));
        assert!(!found.contains_key("old-skill"));
        assert!(!found.contains_key("nested-old-skill"));
    }

    #[tokio::test]
    async fn skill_download_returns_zip_with_references() {
        use std::sync::Arc;
        use axum::Router;
        use tokio::net::TcpListener;

        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("skills");
        let skill_dir = root.join("productivity/test-skill");
        std::fs::create_dir_all(skill_dir.join("references")).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: test-skill\ndescription: Test\n---\n\n# Test\n",
        ).unwrap();
        std::fs::write(
            skill_dir.join("references").join("guide.md"),
            "# Reference Guide\n",
        ).unwrap();

        let hero = skill_dir.join("references").join("hero.png");
        std::fs::write(&hero, b"PNG_DUMMY").unwrap();

        let state = Arc::new(test_app_state("http://127.0.0.1:1".to_string(), temp.path()));
        let app = Router::new()
            .route("/skills/download/{name}", get(skill_download))
            .with_state(state);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let resp = reqwest::get(format!("http://{addr}/skills/download/test-skill"))
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);

        let content_type = resp.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("");
        let content_dispo = resp.headers().get("content-disposition").and_then(|v| v.to_str().ok()).unwrap_or("");
        assert!(content_type.contains("zip") || content_type.contains("octet-stream"), "content-type: {content_type}");
        assert!(content_dispo.contains("attachment"), "no attachment: {content_dispo}");
        assert!(content_dispo.contains("test-skill"), "no skill name: {content_dispo}");

        let bytes = resp.bytes().await.unwrap();
        assert!(bytes.len() > 20, "zip too small: {} bytes", bytes.len());

        let archive = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).unwrap();
        let names: Vec<String> = archive.file_names().map(|s| s.to_string()).collect();
        assert!(names.contains(&"SKILL.md".to_string()), "missing SKILL.md: {names:?}");
        assert!(names.contains(&"references/guide.md".to_string()), "missing references/guide.md: {names:?}");
    }
