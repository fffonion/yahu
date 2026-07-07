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
    fn cron_run_proxy_waits_for_upstream_scheduler_tick() {
        let proxy_source = include_str!("proxy.rs");

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
        std::fs::create_dir_all(&skill_dir.join("references")).unwrap();
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
                    {"id":"tool1","source":"tool","model":"minimax/m3","title":"Tool internal","preview":"cache","started_at":2.0,"message_count":1}
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
    fn session_title_base_strips_lineage_suffix() {
        assert_eq!(session_title_base("Project rename #3"), "Project rename");
        assert_eq!(session_title_base("Project rename #1"), "Project rename #1");
        assert_eq!(session_title_for_lineage_index("Project rename", 2), "Project rename #3");
    }

    #[tokio::test]
    async fn session_lineage_rename_patches_child_and_parent_titles() {
        use std::sync::Mutex;

        #[derive(Clone)]
        struct RenameApiState {
            patched: Arc<Mutex<Vec<(String, String)>>>,
        }

        async fn api_session(
            State(state): State<RenameApiState>,
            AxumPath(session_id): AxumPath<String>,
            method: Method,
            body: Body,
        ) -> Json<serde_json::Value> {
            if method == Method::PATCH {
                let bytes = to_bytes(body, usize::MAX).await.unwrap();
                let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                state.patched.lock().unwrap().push((
                    session_id.clone(),
                    payload["title"].as_str().unwrap_or_default().to_string(),
                ));
            }
            let parent = match session_id.as_str() {
                "child" => Some("parent"),
                "parent" => Some("root"),
                _ => None,
            };
            Json(serde_json::json!({
                "object": "hermes.session",
                "session": {"id": session_id, "parent_session_id": parent, "title": "old"}
            }))
        }

        let patched = Arc::new(Mutex::new(Vec::new()));
        let api_state = RenameApiState { patched: patched.clone() };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let api_app = Router::new()
            .route("/api/sessions/{session_id}", any(api_session))
            .with_state(api_state);
        tokio::spawn(async move { axum::serve(listener, api_app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));

        let resp = rename_session_lineage(
            State(state),
            AxumPath("child".to_string()),
            Json(SessionRenamePayload { title: "Unified title".to_string() }),
        ).await;
        let body = axum::body::to_bytes(resp.into_response().into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["title"], "Unified title #3");
        assert_eq!(payload["base_title"], "Unified title");
        assert_eq!(payload["updated_ids"], serde_json::json!(["root", "parent", "child"]));
        assert_eq!(payload["titles"], serde_json::json!({
            "root": "Unified title",
            "parent": "Unified title #2",
            "child": "Unified title #3",
        }));
        assert_eq!(*patched.lock().unwrap(), vec![
            ("root".to_string(), "Unified title".to_string()),
            ("parent".to_string(), "Unified title #2".to_string()),
            ("child".to_string(), "Unified title #3".to_string()),
        ]);
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

    #[tokio::test]
    async fn session_history_context_stitches_parent_chain_before_child_messages() {
        async fn api_session(AxumPath(session_id): AxumPath<String>) -> Json<serde_json::Value> {
            let parent = match session_id.as_str() {
                "child" => Some("parent"),
                "parent" => Some("root"),
                _ => None,
            };
            Json(serde_json::json!({
                "object": "hermes.session",
                "session": {"id": session_id, "parent_session_id": parent}
            }))
        }

        async fn api_messages(AxumPath(session_id): AxumPath<String>) -> Json<serde_json::Value> {
            Json(serde_json::json!({"object":"list","data":[{
                "id": match session_id.as_str() { "root" => 1, "parent" => 2, _ => 3 },
                "session_id": session_id,
                "role":"user",
                "content": format!("{session_id} message")
            }]}))
        }

        let app = Router::new()
            .route("/api/sessions/{session_id}", get(api_session))
            .route("/api/sessions/{session_id}/messages", get(api_messages));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state(format!("http://{addr}"), temp.path());

        let messages = fetch_all_session_messages_for_context(&state, "child").await.unwrap();

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0]["session_id"], "root");
        assert_eq!(messages[1]["session_id"], "parent");
        assert_eq!(messages[2]["session_id"], "child");
    }

    #[tokio::test]
    async fn session_history_context_uses_local_lineage_when_api_detail_has_no_parent() {
        async fn api_session(AxumPath(session_id): AxumPath<String>) -> Json<serde_json::Value> {
            Json(serde_json::json!({
                "object": "hermes.session",
                "session": {"id": session_id, "parent_session_id": null}
            }))
        }

        async fn api_messages(AxumPath(_session_id): AxumPath<String>) -> Json<serde_json::Value> {
            Json(serde_json::json!({"object":"list","data":[{
                "id": 3,
                "session_id": "child",
                "role":"user",
                "content": "api child segment only"
            }]}))
        }

        let app = Router::new()
            .route("/api/sessions/{session_id}", get(api_session))
            .route("/api/sessions/{session_id}/messages", get(api_messages));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

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
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,end_reason,source) VALUES ('root',NULL,1,'compression','telegram')", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,end_reason,source) VALUES ('parent','root',2,'compression','telegram')", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,end_reason,source) VALUES ('child','parent',3,NULL,'telegram')", []).unwrap();
        for (sid, content) in [("root", "local root message"), ("parent", "local parent message"), ("child", "local child message")] {
            conn.execute(
                "INSERT INTO messages (session_id,role,content,timestamp,active) VALUES (?1,'user',?2,1,1)",
                rusqlite::params![sid, content],
            ).unwrap();
        }
        drop(conn);
        let state = test_app_state(format!("http://{addr}"), temp.path());

        let messages = fetch_all_session_messages_for_context(&state, "child").await.unwrap();

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0]["session_id"], "root");
        assert_eq!(messages[1]["session_id"], "parent");
        assert_eq!(messages[2]["session_id"], "child");
        assert_eq!(messages[0]["content"], "local root message");
    }

    #[tokio::test]
    async fn session_history_context_prefers_local_db_lineage_messages() {
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
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,end_reason,source) VALUES ('root',NULL,1,'compression','telegram')", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,end_reason,source) VALUES ('parent','root',2,'compression','telegram')", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,end_reason,source) VALUES ('child','parent',3,NULL,'telegram')", []).unwrap();
        for (sid, content) in [("root", "root message"), ("parent", "parent message"), ("child", "child message")] {
            conn.execute(
                "INSERT INTO messages (session_id,role,content,timestamp,active) VALUES (?1,'user',?2,1,1)",
                rusqlite::params![sid, content],
            ).unwrap();
        }
        drop(conn);
        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());
        let local_messages = fetch_local_lineage_messages(&state, "child").unwrap().unwrap();
        assert_eq!(local_messages.len(), 3);

        let messages = fetch_all_session_messages_for_context(&state, "child").await.unwrap();

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0]["session_id"], "root");
        assert_eq!(messages[1]["session_id"], "parent");
        assert_eq!(messages[2]["session_id"], "child");
    }

    #[tokio::test]
    async fn chat_history_includes_reset_predecessor_for_same_thread() {
        async fn api_session(AxumPath(session_id): AxumPath<String>) -> Json<serde_json::Value> {
            Json(serde_json::json!({
                "object": "hermes.session",
                "session": {"id": session_id, "parent_session_id": null}
            }))
        }

        async fn api_messages(AxumPath(_session_id): AxumPath<String>) -> Json<serde_json::Value> {
            Json(serde_json::json!({"object":"list","data":[{
                "id": 3,
                "session_id": "current",
                "role":"assistant",
                "content": "current summary"
            }]}))
        }

        let app = Router::new()
            .route("/api/sessions/{session_id}", get(api_session))
            .route("/api/sessions/{session_id}/messages", get(api_messages));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                parent_session_id TEXT,
                started_at REAL,
                ended_at REAL,
                end_reason TEXT,
                source TEXT,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT
             );
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
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('old_root',NULL,1,2,'compression','telegram','agent:telegram:thread','chat','topic')", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('old_tip','old_root',2,10,'session_reset','telegram','agent:telegram:thread','chat','topic')", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('current',NULL,10.5,NULL,NULL,'telegram','agent:telegram:thread','chat','topic')", []).unwrap();
        for (sid, role, content) in [
            ("old_root", "user", "original question"),
            ("old_tip", "assistant", "previous answer"),
            ("current", "assistant", "current summary"),
        ] {
            conn.execute(
                "INSERT INTO messages (session_id,role,content,timestamp,active) VALUES (?1,?2,?3,1,1)",
                rusqlite::params![sid, role, content],
            ).unwrap();
        }
        drop(conn);
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));

        let resp = chat_messages_page(
            State(state),
            AxumPath("current".to_string()),
            Query(ChatMessagesQuery { before: None, after: None, around: None, limit: Some(20) }),
        )
        .await;
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let page: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let data = page["data"].as_array().unwrap();

        assert_eq!(page["total"], 3);
        assert_eq!(data[0]["session_id"], "old_root");
        assert_eq!(data[0]["role"], "user");
        assert_eq!(data[2]["session_id"], "current");
    }

    #[tokio::test]
    async fn chat_messages_page_reports_stitched_boundary_timestamps() {
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
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,end_reason,source) VALUES ('root',NULL,10,'compression','telegram')", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,end_reason,source) VALUES ('child','root',20,NULL,'telegram')", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,timestamp,active) VALUES ('root','user','first stitched message',111,1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,timestamp,active) VALUES ('child','assistant','latest stitched message',333,1)", []).unwrap();
        drop(conn);
        let state = Arc::new(test_app_state("http://127.0.0.1:1".to_string(), temp.path()));

        let resp = chat_messages_page(
            State(state),
            AxumPath("child".to_string()),
            Query(ChatMessagesQuery { before: None, after: None, around: None, limit: Some(1) }),
        )
        .await;
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let page: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(page["total"], 2);
        assert_eq!(page["data"].as_array().unwrap().len(), 1);
        assert_eq!(page["started_at"], serde_json::json!(111.0));
        assert_eq!(page["last_active"], serde_json::json!(333.0));
    }

    #[tokio::test]
    async fn chat_history_includes_immediate_agent_close_predecessor_for_same_thread() {
        async fn api_session(AxumPath(session_id): AxumPath<String>) -> Json<serde_json::Value> {
            Json(serde_json::json!({
                "object": "hermes.session",
                "session": {"id": session_id, "parent_session_id": null}
            }))
        }

        async fn api_messages(AxumPath(_session_id): AxumPath<String>) -> Json<serde_json::Value> {
            Json(serde_json::json!({"object":"list","data":[{
                "id": 3,
                "session_id": "current",
                "role":"assistant",
                "content": "current summary"
            }]}))
        }

        let app = Router::new()
            .route("/api/sessions/{session_id}", get(api_session))
            .route("/api/sessions/{session_id}/messages", get(api_messages));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                parent_session_id TEXT,
                started_at REAL,
                ended_at REAL,
                end_reason TEXT,
                source TEXT,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT
             );
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
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('old_root',NULL,1,2,'compression','telegram','agent:telegram:dm','chat',NULL)", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('old_tip','old_root',2,10,'agent_close','telegram','agent:telegram:dm','chat',NULL)", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('current',NULL,10.5,NULL,NULL,'telegram','agent:telegram:dm','chat',NULL)", []).unwrap();
        for (sid, role, content) in [
            ("old_root", "user", "original dm question"),
            ("old_tip", "assistant", "previous dm answer"),
            ("current", "assistant", "current summary"),
        ] {
            conn.execute(
                "INSERT INTO messages (session_id,role,content,timestamp,active) VALUES (?1,?2,?3,1,1)",
                rusqlite::params![sid, role, content],
            ).unwrap();
        }
        drop(conn);
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));

        let resp = chat_messages_page(
            State(state),
            AxumPath("current".to_string()),
            Query(ChatMessagesQuery { before: None, after: None, around: None, limit: Some(20) }),
        )
        .await;
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let page: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let data = page["data"].as_array().unwrap();

        assert_eq!(page["total"], 3);
        assert_eq!(data[0]["session_id"], "old_root");
        assert_eq!(data[0]["role"], "user");
        assert_eq!(data[2]["session_id"], "current");
    }

    #[tokio::test]
    async fn chat_history_does_not_merge_same_session_key_from_other_thread() {
        async fn api_session(AxumPath(session_id): AxumPath<String>) -> Json<serde_json::Value> {
            Json(serde_json::json!({
                "object": "hermes.session",
                "session": {"id": session_id, "parent_session_id": null}
            }))
        }

        async fn api_messages(AxumPath(_session_id): AxumPath<String>) -> Json<serde_json::Value> {
            Json(serde_json::json!({"object":"list","data":[{
                "id": 3,
                "session_id": "current",
                "role":"assistant",
                "content": "current summary"
            }]}))
        }

        let app = Router::new()
            .route("/api/sessions/{session_id}", get(api_session))
            .route("/api/sessions/{session_id}/messages", get(api_messages));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                parent_session_id TEXT,
                started_at REAL,
                ended_at REAL,
                end_reason TEXT,
                source TEXT,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT
             );
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
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('other_root',NULL,1,2,'compression','telegram','shared-key','other-chat','topic')", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('other_tip','other_root',2,10,'session_reset','telegram','shared-key','other-chat','topic')", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('current',NULL,10.5,NULL,NULL,'telegram','shared-key','chat','topic')", []).unwrap();
        for (sid, role, content) in [
            ("other_root", "user", "other thread question"),
            ("other_tip", "assistant", "other thread answer"),
            ("current", "assistant", "current summary"),
        ] {
            conn.execute(
                "INSERT INTO messages (session_id,role,content,timestamp,active) VALUES (?1,?2,?3,1,1)",
                rusqlite::params![sid, role, content],
            ).unwrap();
        }
        drop(conn);
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));

        let resp = chat_messages_page(
            State(state),
            AxumPath("current".to_string()),
            Query(ChatMessagesQuery { before: None, after: None, around: None, limit: Some(20) }),
        )
        .await;
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let page: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let data = page["data"].as_array().unwrap();

        assert_eq!(page["total"], 1);
        assert_eq!(data[0]["session_id"], "current");
    }

    #[test]
    fn user_message_nav_items_include_position_excerpt_and_timestamp() {
        let messages = vec![
            serde_json::json!({"id": 1, "role": "system", "content": "setup"}),
            serde_json::json!({"id": 2, "role": "user", "content": "[Alliumcepa Triplef|1698432746] first question", "timestamp": 1710000000}),
            serde_json::json!({"id": 3, "role": "assistant", "content": "draft answer"}),
            serde_json::json!({"id": 4, "role": "assistant", "content": "final answer shown in navigator"}),
            serde_json::json!({"id": 5, "role": "user", "content": {"text": "second question with a lot of detail"}, "created_at": 1710000060}),
        ];

        let nav = build_user_message_nav(&messages);

        assert_eq!(nav.len(), 2);
        assert_eq!(nav[0].id, "2");
        assert_eq!(nav[0].role, "user");
        assert_eq!(nav[0].content, "first question");
        assert_eq!(nav[0].assistant_preview.as_deref(), Some("final answer shown in navigator"));
        assert_eq!(nav[0].timestamp, Some(serde_json::json!(1710000000)));
        assert_eq!(nav[0].index, 1);
        assert_eq!(nav[0].total, 5);
        assert!(nav[0].position > 0.24 && nav[0].position < 0.26);
        assert_eq!(nav[1].id, "5");
        assert!(nav[1].assistant_preview.is_none());
        assert!(nav[1].position > 0.99);
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
    fn chat_stream_model_switch_request_builds_provider_command() {
        let body = serde_json::json!({
            "input": "hello",
            "model": "anthropic/claude-haiku-4.5",
            "provider": "anthropic",
            "reasoning_effort": "none"
        });
        let request = chat_stream_model_switch_request(
            "api/sessions/existing-session/chat/stream",
            &Method::POST,
            body.to_string().as_bytes(),
        )
        .unwrap();

        assert_eq!(request.session_id, "existing-session");
        assert_eq!(request.command, "/model anthropic/claude-haiku-4.5 --provider anthropic --session");
        assert_eq!(request.body["input"], "/model anthropic/claude-haiku-4.5 --provider anthropic --session");
        assert_eq!(request.body["reasoning_effort"], "none");
    }

    #[test]
    fn chat_stream_model_switch_request_ignores_placeholder_model() {
        let body = serde_json::json!({"input":"hello","model":"hermes-agent"});
        assert!(chat_stream_model_switch_request(
            "api/sessions/new-session/chat/stream",
            &Method::POST,
            body.to_string().as_bytes(),
        ).is_none());
    }

    #[tokio::test]
    async fn yahu_chat_stream_sends_model_switch_before_actual_stream() {
        use std::sync::Mutex;

        #[derive(Clone)]
        struct OrderApiState {
            calls: Arc<Mutex<Vec<serde_json::Value>>>,
        }

        async fn api_chat(
            State(state): State<OrderApiState>,
            AxumPath(session_id): AxumPath<String>,
            body: Body,
        ) -> Json<serde_json::Value> {
            let bytes = to_bytes(body, usize::MAX).await.unwrap();
            let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            state.calls.lock().unwrap().push(serde_json::json!({
                "kind": "model",
                "session_id": session_id,
                "body": payload,
            }));
            Json(serde_json::json!({"ok": true}))
        }

        async fn api_stream(
            State(state): State<OrderApiState>,
            AxumPath(session_id): AxumPath<String>,
            body: Body,
        ) -> Response<Body> {
            let bytes = to_bytes(body, usize::MAX).await.unwrap();
            let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            state.calls.lock().unwrap().push(serde_json::json!({
                "kind": "stream",
                "session_id": session_id,
                "body": payload,
            }));
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/event-stream")
                .body(Body::from("event: done\ndata: {}\n\n"))
                .unwrap()
        }

        let calls = Arc::new(Mutex::new(Vec::new()));
        let api_state = OrderApiState { calls: calls.clone() };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let api_app = Router::new()
            .route("/api/sessions/{session_id}/chat", post(api_chat))
            .route("/api/sessions/{session_id}/chat/stream", post(api_stream))
            .with_state(api_state);
        tokio::spawn(async move { axum::serve(listener, api_app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));

        let body = serde_json::json!({
            "input": "hello",
            "model": "gpt-5.5",
            "provider": "openai-codex",
            "reasoning_effort": "medium"
        });
        let resp = chat_stream(
            State(state),
            AxumPath("session-1".to_string()),
            HeaderMap::new(),
            Body::from(body.to_string()),
        ).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let _ = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();

        let calls = calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 2, "calls: {calls:?}");
        assert_eq!(calls[0]["kind"], "model");
        assert_eq!(calls[0]["body"]["input"], "/model gpt-5.5 --provider openai-codex --session");
        assert_eq!(calls[1]["kind"], "stream");
        assert_eq!(calls[1]["body"]["input"], "hello");
        assert_eq!(calls[1]["body"]["reasoning_effort"], "medium");
        assert!(calls[1]["body"].get("model").is_none(), "stream body must not carry model: {calls:?}");
        assert!(calls[1]["body"].get("provider").is_none(), "stream body must not carry provider: {calls:?}");
    }

    #[tokio::test]
    async fn yahu_chat_stream_does_not_publish_user_snapshot_before_model_switch_completes() {
        use std::sync::Mutex;
        use tokio::sync::{mpsc, oneshot};

        #[derive(Clone)]
        struct SlowModelApiState {
            calls: Arc<Mutex<Vec<String>>>,
            model_started: mpsc::UnboundedSender<()>,
            release_model: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
        }

        async fn slow_api_chat(
            State(state): State<SlowModelApiState>,
            AxumPath(_session_id): AxumPath<String>,
            body: Body,
        ) -> Json<serde_json::Value> {
            let bytes = to_bytes(body, usize::MAX).await.unwrap();
            let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            state.calls.lock().unwrap().push(format!("model:{}", payload["input"].as_str().unwrap_or_default()));
            let _ = state.model_started.send(());
            let release = state.release_model.lock().unwrap().take().unwrap();
            let _ = release.await;
            Json(serde_json::json!({"ok": true}))
        }

        async fn slow_api_stream(
            State(state): State<SlowModelApiState>,
            AxumPath(_session_id): AxumPath<String>,
            body: Body,
        ) -> Response<Body> {
            let bytes = to_bytes(body, usize::MAX).await.unwrap();
            let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            state.calls.lock().unwrap().push(format!("stream:{}", payload["input"].as_str().unwrap_or_default()));
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/event-stream")
                .body(Body::from("event: done\ndata: {}\n\n"))
                .unwrap()
        }

        let calls = Arc::new(Mutex::new(Vec::new()));
        let (model_started_tx, mut model_started_rx) = mpsc::unbounded_channel();
        let (release_tx, release_rx) = oneshot::channel();
        let api_state = SlowModelApiState {
            calls: calls.clone(),
            model_started: model_started_tx,
            release_model: Arc::new(Mutex::new(Some(release_rx))),
        };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let api_app = Router::new()
            .route("/api/sessions/{session_id}/chat", post(slow_api_chat))
            .route("/api/sessions/{session_id}/chat/stream", post(slow_api_stream))
            .with_state(api_state);
        tokio::spawn(async move { axum::serve(listener, api_app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));

        let body = serde_json::json!({
            "input": "hello after model",
            "model": "gpt-5.5",
            "provider": "openai-codex",
            "reasoning_effort": "medium"
        });
        let state_for_call = state.clone();
        let stream_task = tokio::spawn(async move {
            chat_stream(
                State(state_for_call),
                AxumPath("session-1".to_string()),
                HeaderMap::new(),
                Body::from(body.to_string()),
            ).await
        });

        model_started_rx.recv().await.unwrap();
        let active = state.active_chat_streams.read().await;
        assert!(
            active.get("session-1").is_none_or(|messages| messages.is_empty()),
            "user snapshot must wait until model switch succeeds: {active:?}"
        );
        drop(active);

        release_tx.send(()).unwrap();
        let resp = stream_task.await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let _ = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        assert_eq!(
            calls.lock().unwrap().as_slice(),
            [
                "model:/model gpt-5.5 --provider openai-codex --session".to_string(),
                "stream:hello after model".to_string()
            ]
        );
    }

    #[test]
    fn chat_stream_broadcast_ring_is_small_because_payloads_are_full_snapshots() {
        assert!(CHAT_STREAM_BROADCAST_CAPACITY <= 32);
    }

    #[tokio::test]
    async fn session_search_trusts_api_server_query_results_without_history_scan() {
        use std::collections::HashMap;
        use std::sync::atomic::{AtomicUsize, Ordering};

        #[derive(Clone)]
        struct SearchApiState {
            message_requests: Arc<AtomicUsize>,
        }

        async fn api_sessions(
            Query(query): Query<HashMap<String, String>>,
        ) -> Json<serde_json::Value> {
            assert_eq!(
                query.get("include_children").map(String::as_str),
                Some("false")
            );
            assert_eq!(query.get("q").map(String::as_str), Some("cache"));
            Json(serde_json::json!({
                "object": "list",
                "data": [
                    {"id":"s1","source":"telegram","model":"minimax/m3","title":"MiniMax billing","preview":"first prompt","started_at":1.0,"message_count":2},
                    {"id":"s2","source":"api_server","model":"gpt-5.5","title":"Other","preview":"unrelated","started_at":3.0,"message_count":1}
                ],
                "has_more": false
            }))
        }

        async fn api_messages(
            State(state): State<SearchApiState>,
            AxumPath(_session_id): AxumPath<String>,
        ) -> Json<serde_json::Value> {
            state.message_requests.fetch_add(1, Ordering::SeqCst);
            Json(serde_json::json!({"object":"list","data":[{"id":1,"role":"user","content":"cache inside long history"}]}))
        }

        let message_requests = Arc::new(AtomicUsize::new(0));
        let app = Router::new()
            .route("/api/sessions", get(api_sessions))
            .route("/api/sessions/{session_id}/messages", get(api_messages))
            .with_state(SearchApiState { message_requests: message_requests.clone() });
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

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["id"], "s1");
        assert_eq!(rows[1]["id"], "s2");
        assert_eq!(message_requests.load(Ordering::SeqCst), 0);
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
            Query(ChatMessagesQuery { before: None, after: None, around: None, limit: Some(3) }),
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

}
