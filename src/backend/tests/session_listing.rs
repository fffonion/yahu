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
            session_watch_feeds: Arc::new(RwLock::new(HashMap::new())),
            active_chat_streams: Arc::new(RwLock::new(HashMap::new())),
            active_chat_run_ids: Arc::new(RwLock::new(HashMap::new())),
            subagent_feeds: Arc::new(RwLock::new(HashMap::new())),
            model_cache: Arc::new(RwLock::new(ModelCache::default())),
            model_price_cache: Arc::new(RwLock::new(ModelPriceCache::default())),
            insights_snapshot_refresh: Arc::new(Mutex::new(())),
            provider_usage_cache: Arc::new(ProviderUsageCache::default()),
        }
    }

    #[test]
    fn pinned_display_metadata_keeps_an_explicit_canonical_title() {
        let temp = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open(temp.path().join("state.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                parent_session_id TEXT,
                end_reason TEXT,
                started_at REAL NOT NULL,
                source TEXT,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT,
                title TEXT
            );
            INSERT INTO sessions VALUES
                ('root', NULL, 'session_switch', 1.0, 'telegram', 'key', 'chat', 'thread', 'kfc'),
                ('child-2', 'root', 'session_switch', 2.0, 'telegram', 'key', 'chat', 'thread', 'kfc #2'),
                ('child-8', 'child-2', 'session_switch', 8.0, 'telegram', 'key', 'chat', 'thread', 'kfc #8');",
        )
        .unwrap();
        drop(conn);

        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());
        let display = local_session_display_metadata(&state, "root").unwrap();

        assert_eq!(display, Some(("root".to_string(), "kfc".to_string())));
    }

    #[test]
    fn keyed_session_entries_stop_at_an_independent_root_with_the_same_chat_key() {
        let temp = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open(temp.path().join("state.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                parent_session_id TEXT,
                end_reason TEXT,
                started_at REAL NOT NULL,
                source TEXT,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT
            );
            INSERT INTO sessions VALUES
                ('root-a', NULL, 'session_switch', 1.0, 'telegram', 'key', 'chat', 'thread'),
                ('child-a', 'root-a', 'session_switch', 2.0, 'telegram', 'key', 'chat', 'thread'),
                ('root-b', NULL, 'session_switch', 3.0, 'telegram', 'key', 'chat', 'thread'),
                ('child-b', 'root-b', 'session_switch', 4.0, 'telegram', 'key', 'chat', 'thread');",
        )
        .unwrap();
        drop(conn);

        let conn = rusqlite::Connection::open(temp.path().join("state.db")).unwrap();
        let entries = local_session_key_group_entries(&conn, "root-a")
            .unwrap()
            .unwrap();

        let ids = entries.into_iter().map(|entry| entry.id).collect::<Vec<_>>();
        assert_eq!(ids, vec!["root-a", "child-a"]);
    }

    #[test]
    fn chat_view_entries_include_a_later_same_key_root_for_latest_history() {
        let temp = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open(temp.path().join("state.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                parent_session_id TEXT,
                end_reason TEXT,
                started_at REAL NOT NULL,
                source TEXT,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT
            );
            INSERT INTO sessions VALUES
                ('root-a', NULL, 'session_switch', 1.0, 'telegram', 'key', 'chat', 'thread'),
                ('child-a', 'root-a', 'session_switch', 2.0, 'telegram', 'key', 'chat', 'thread'),
                ('root-mid', NULL, 'agent_close', 3.0, 'telegram', 'key', 'chat', 'thread'),
                ('child-mid', 'root-mid', 'agent_close', 4.0, 'telegram', 'key', 'chat', 'thread'),
                ('root-b', NULL, 'session_switch', 5.0, 'telegram', 'key', 'chat', 'thread'),
                ('child-b', 'root-b', NULL, 6.0, 'telegram', 'key', 'chat', 'thread');",
        )
        .unwrap();

        let entries = local_session_chat_view_entries(&conn, "root-a").unwrap();
        let ids = entries.into_iter().map(|entry| entry.id).collect::<Vec<_>>();
        assert_eq!(ids, vec!["root-a", "child-a", "root-mid", "child-mid", "root-b", "child-b"]);
    }

    #[test]
    fn session_list_family_merge_keeps_independent_roots_with_the_same_chat_key() {
        let metadata = vec![
            LocalSessionListMetadata {
                id: "root-a".to_string(),
                parent_session_id: None,
                started_at: 1.0,
                ended_at: Some(2.0),
                end_reason: Some("session_switch".to_string()),
                session_key: Some("key".to_string()),
                source: Some("telegram".to_string()),
                chat_id: Some("chat".to_string()),
                thread_id: Some("thread".to_string()),
                title: Some("kfc".to_string()),
                reset_from: None,
            },
            LocalSessionListMetadata {
                id: "root-b".to_string(),
                parent_session_id: None,
                started_at: 3.0,
                ended_at: Some(4.0),
                end_reason: Some("session_switch".to_string()),
                session_key: Some("key".to_string()),
                source: Some("telegram".to_string()),
                chat_id: Some("chat".to_string()),
                thread_id: Some("thread".to_string()),
                title: Some("another".to_string()),
                reset_from: None,
            },
        ];
        let mut rows = vec![
            serde_json::json!({"id":"root-a","started_at":1.0,"last_active":2.0,"title":"kfc"}),
            serde_json::json!({"id":"root-b","started_at":3.0,"last_active":4.0,"title":"another"}),
        ];

        merge_session_rows_by_chat_family(&mut rows, &metadata);

        let ids = rows
            .iter()
            .filter_map(|row| row.get("id").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(ids, vec!["root-a", "root-b"]);
    }

    #[test]
    fn local_session_preview_replaces_marker_with_latest_real_message() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT,
                tool_calls TEXT,
                finish_reason TEXT,
                active INTEGER NOT NULL DEFAULT 1,
                compacted INTEGER NOT NULL DEFAULT 0
            );",
        )
        .unwrap();
        for (role, content) in [
            ("user", "old question"),
            ("assistant", "old answer"),
            ("user", "new question"),
            ("assistant", "new answer"),
            (
                "user",
                "[Alliumcepa Triplef] [CONTEXT COMPACTION — REFERENCE ONLY]\\ncompacted transcript",
            ),
        ] {
            conn.execute(
                "INSERT INTO messages (session_id, role, content) VALUES ('s1', ?1, ?2)",
                rusqlite::params![role, content],
            )
            .unwrap();
        }
        drop(conn);

        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());
        let mut rows = vec![serde_json::json!({
            "id": "s1",
            "preview": "[CONTEXT COMPACTION — REFERENCE ONLY]"
        })];
        enrich_session_previews_from_local_db(&state, &mut rows).unwrap();

        assert_eq!(rows[0]["preview"], "new answer");
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
                    {"id":"s1","source":"telegram","model":"minimax/m3","title":"Cache billing","preview":"[Alliumcepa Triplef|1698432746]\ntoken cache math","started_at":1.0,"message_count":1},
                    {"id":"content-only","source":"telegram","model":"minimax/m3","title":"Billing details","preview":"cache appears only in this message","started_at":1.5,"message_count":1},
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

        let rows = fetch_sessions_from_api_server(&state, "cache", 10, false)
            .await
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "s1");
        assert_eq!(rows[0]["model"], "minimax/m3");
        assert_eq!(rows[0]["preview"], "token cache math");
    }

    #[tokio::test]
    async fn session_search_hides_turtle_sources_when_source_filter_is_on() {
        use std::collections::HashMap;

        async fn api_sessions(
            Query(query): Query<HashMap<String, String>>,
        ) -> Json<serde_json::Value> {
            assert_eq!(query.get("q").map(String::as_str), Some("cache"));
            Json(serde_json::json!({
                "object": "list",
                "data": [
                    {"id":"s1","source":"telegram","title":"Cache keep","preview":"cache","started_at":3.0},
                    {"id":"tb1","source":"turtle-bench","title":"bench","preview":"cache","started_at":2.0},
                    {"id":"ts1","source":"turtle-soup","title":"soup","preview":"cache","started_at":1.5},
                    {"id":"cli1","source":"cli","title":"cli","preview":"cache","started_at":1.0}
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

        let rows = fetch_sessions_from_api_server(&state, "cache", 10, true)
            .await
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "s1");
        assert!(rows.iter().all(|row| {
            row["source"] != "turtle-bench" && row["source"] != "turtle-soup"
        }));
    }

    #[tokio::test]
    async fn session_source_filter_reaches_upstream_before_the_result_limit() {
        use std::collections::HashMap;

        async fn api_sessions(
            Query(query): Query<HashMap<String, String>>,
        ) -> Json<serde_json::Value> {
            assert_eq!(
                query.get("exclude_sources").map(String::as_str),
                Some("tool")
            );
            let offset = query
                .get("offset")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or_default();
            let rows = (0..160)
                .map(|index| {
                    let ordinary = index < 10;
                    let source = if ordinary {
                        "telegram"
                    } else {
                        match index % 3 {
                            0 => "cron",
                            1 => "cli",
                            _ => "alp-worker",
                        }
                    };
                    serde_json::json!({
                        "id": format!("{}-{offset}-{index}", if ordinary { "normal" } else { "cron" }),
                        "source": source,
                        "started_at": 10_000 - offset - index,
                    })
                })
                .collect::<Vec<_>>();
            Json(serde_json::json!({
                "object": "list",
                "data": rows,
                "total": 1_600,
                "has_more": offset < 1_400,
            }))
        }

        let app = Router::new().route("/api/sessions", get(api_sessions));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state(format!("http://{addr}"), temp.path());

        let rows = fetch_sessions_from_api_server(&state, "", 80, true)
            .await
            .unwrap();

        assert_eq!(rows.len(), 80);
        assert!(rows.iter().all(|row| row["source"] == "telegram"));
    }

    #[tokio::test]
    async fn session_source_filter_fetches_followup_pages_concurrently() {
        use std::collections::HashMap;
        use std::sync::atomic::{AtomicUsize, Ordering};

        #[derive(Default)]
        struct ConcurrencyProbe {
            active: AtomicUsize,
            max_active: AtomicUsize,
        }

        async fn api_sessions(
            State(probe): State<Arc<ConcurrencyProbe>>,
            Query(query): Query<HashMap<String, String>>,
        ) -> Json<serde_json::Value> {
            let active = probe.active.fetch_add(1, Ordering::SeqCst) + 1;
            probe.max_active.fetch_max(active, Ordering::SeqCst);
            tokio::time::sleep(Duration::from_millis(40)).await;
            probe.active.fetch_sub(1, Ordering::SeqCst);
            let offset = query
                .get("offset")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or_default();
            let rows = (0..200)
                .map(|index| {
                    let ordinary = index < 10;
                    serde_json::json!({
                        "id": format!("{}-{offset}-{index}", if ordinary { "normal" } else { "cron" }),
                        "source": if ordinary { "telegram" } else { "cron" },
                    })
                })
                .collect::<Vec<_>>();
            Json(serde_json::json!({
                "data": rows,
                "has_more": offset < 1_400,
            }))
        }

        let probe = Arc::new(ConcurrencyProbe::default());
        let app = Router::new()
            .route("/api/sessions", get(api_sessions))
            .with_state(probe.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state(format!("http://{addr}"), temp.path());

        let rows = fetch_sessions_from_api_server(&state, "", 80, true)
            .await
            .unwrap();

        assert_eq!(rows.len(), 80);
        assert!(probe.max_active.load(Ordering::SeqCst) >= 2);
    }

    #[test]
    fn local_filtered_sidebar_preaggregates_session_switch_activity_once() {
        let source = include_str!("../sessions.rs");

        assert!(source.contains("session_switch_activity(root_id, last_active)"));
        assert!(source.contains("GROUP BY chain.root_id"));
        assert!(!source.contains("WHERE chain.root_id = s.id"));
    }

    #[test]
    fn session_search_does_not_reprocess_already_enriched_rows() {
        let source = include_str!("../sessions.rs");

        assert!(!source.contains("let data = session_rows_with_local_previews(&state, data);"));
    }

    #[test]
    fn session_scan_checks_lineage_without_preview_work_between_pages() {
        let source = include_str!("../sessions.rs");

        assert!(source.contains("session_rows_with_local_lineage(state, rows.clone())"));
        assert!(!source.contains("session_rows_with_local_previews(state, rows.clone())"));
    }

    #[test]
    fn local_preview_uses_session_index_ranges_instead_of_global_window_scan() {
        let source = include_str!("../sessions.rs");

        assert!(source.contains("session_id = ?1"));
        assert!(source.contains("ORDER BY id DESC\n         LIMIT 1 OFFSET ?2"));
        assert!(!source.contains("ROW_NUMBER() OVER (PARTITION BY session_id ORDER BY id DESC)"));
    }

    #[test]
    fn local_preview_stops_after_first_real_candidate_per_entry() {
        let source = include_str!("../sessions.rs");

        assert!(source.contains("LIMIT 1 OFFSET ?2"));
        assert!(source.contains("for offset in 0..100_i64"));
        assert!(!source.contains("LIMIT 100\""));
    }

    #[test]
    fn local_filtered_sidebar_query_excludes_sources_and_enriches_previews() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let mut conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT,
                active INTEGER NOT NULL DEFAULT 1
             );
             CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                source TEXT,
                model TEXT,
                model_config TEXT,
                billing_provider TEXT,
                parent_session_id TEXT,
                started_at REAL,
                ended_at REAL,
                end_reason TEXT,
                message_count INTEGER,
                title TEXT,
                archived INTEGER,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT
            );",
        )
        .unwrap();
        let transaction = conn.transaction().unwrap();
        for index in 0..120 {
            transaction
                .execute(
                    "INSERT INTO sessions
                     (id, source, model, started_at, message_count, title, archived)
                     VALUES (?1, 'cron', 'cron-model', ?2, 1, 'scheduled', 0)",
                    rusqlite::params![format!("cron-{index}"), 1_000.0 + index as f64],
                )
                .unwrap();
        }
        for index in 0..90 {
            transaction
                .execute(
                    "INSERT INTO sessions
                     (id, source, model, model_config, billing_provider, started_at, message_count, title, archived)
                     VALUES (?1, 'telegram', 'chat-model', ?2, 'fallback-provider', ?3, 2, ?4, 0)",
                    rusqlite::params![
                        format!("normal-{index}"),
                        r#"{"gateway_runtime":{"provider":"chat-provider"}}"#,
                        10.0 + index as f64,
                        format!("ordinary {index}"),
                    ],
                )
                .unwrap();
        }
        transaction.execute(
            "INSERT INTO sessions
             (id, source, model, started_at, message_count, title, archived)
             VALUES ('alp-worker-newest', 'alp-worker', 'worker-model', 3000.0, 1, 'worker', 0)",
            [],
        ).unwrap();
        transaction.execute(
            "INSERT INTO sessions
             (id, source, model, started_at, message_count, title, archived)
             VALUES ('turtle-bench-newest', 'turtle-bench', 'bench-model', 4000.0, 1, 'bench', 0)",
            [],
        ).unwrap();
        transaction.execute(
            "INSERT INTO sessions
             (id, source, model, started_at, message_count, title, archived)
             VALUES ('turtle-soup-newest', 'turtle-soup', 'soup-model', 3500.0, 1, 'soup', 0)",
            [],
        ).unwrap();
        transaction.execute("INSERT INTO messages (session_id,role,content,active) VALUES ('normal-89','user','latest question',1)", []).unwrap();
        transaction.execute("INSERT INTO messages (session_id,role,content,active) VALUES ('normal-89','assistant','latest final answer',1)", []).unwrap();
        transaction.commit().unwrap();
        drop(conn);
        let state = test_app_state("http://127.0.0.1:9".to_string(), temp.path());

        let rows = fetch_filtered_sidebar_sessions_from_local_db(&state, 80)
            .unwrap()
            .unwrap();

        assert_eq!(rows.len(), 80);
        assert!(rows.iter().all(|row| row["source"] == "telegram"));
        assert_eq!(rows[0]["id"], "normal-89");
        assert_eq!(rows[0]["provider"], "chat-provider");
        assert_eq!(rows[0]["preview"], "latest final answer");
    }

    #[test]
    fn title_filtered_local_sessions_ignore_message_preview_matches() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT,
                active INTEGER NOT NULL DEFAULT 1
             );
             CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                source TEXT,
                model TEXT,
                model_config TEXT,
                billing_provider TEXT,
                parent_session_id TEXT,
                started_at REAL,
                ended_at REAL,
                end_reason TEXT,
                message_count INTEGER,
                title TEXT,
                archived INTEGER,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT
            );
            INSERT INTO sessions (id, source, started_at, title, archived)
                VALUES ('title-hit', 'telegram', 2.0, 'Needle title', 0);
            INSERT INTO sessions (id, source, started_at, title, archived)
                VALUES ('preview-only', 'telegram', 1.0, 'Other title', 0);
            INSERT INTO messages (session_id, role, content, active)
                VALUES ('title-hit', 'assistant', 'unrelated preview', 1);
            INSERT INTO messages (session_id, role, content, active)
                VALUES ('preview-only', 'assistant', 'needle in the preview', 1);",
        )
        .unwrap();
        drop(conn);
        let state = test_app_state("http://127.0.0.1:9".to_string(), temp.path());

        let rows = fetch_title_filtered_sessions_from_local_db(&state, 80, "needle", false)
            .unwrap()
            .unwrap();
        let ids = rows
            .iter()
            .filter_map(|row| row.get("id").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["title-hit"]);
    }

    #[test]
    fn local_filtered_sidebar_keeps_listable_lineage_children() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT,
                active INTEGER NOT NULL DEFAULT 1
             );
             CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                source TEXT,
                model TEXT,
                model_config TEXT,
                billing_provider TEXT,
                parent_session_id TEXT,
                started_at REAL,
                ended_at REAL,
                end_reason TEXT,
                message_count INTEGER,
                title TEXT,
                archived INTEGER,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT
            );
            INSERT INTO sessions (id, source, started_at, end_reason, archived)
                VALUES ('branch-parent', 'telegram', 1.0, 'branched', 0);
            INSERT INTO sessions (id, source, model_config, parent_session_id, started_at, archived)
                VALUES ('branch-child', 'telegram', '{\"_branched_from\":\"branch-parent\"}', 'branch-parent', 2.0, 0);
            INSERT INTO sessions (id, source, started_at, end_reason, session_key, archived)
                VALUES ('reset-parent', 'weixin', 3.0, 'session_reset', 'same-key', 0);
            INSERT INTO sessions (id, source, model_config, parent_session_id, started_at, session_key, archived)
                VALUES ('reset-child', 'weixin', '{\"_reset_from\":\"reset-parent\"}', 'reset-parent', 4.0, 'same-key', 0);
            INSERT INTO sessions (id, source, started_at, end_reason, archived)
                VALUES ('switch-parent', 'telegram', 8.0, 'session_switch', 0);
            INSERT INTO sessions (id, source, parent_session_id, started_at, archived)
                VALUES ('switch-child', 'telegram', 'switch-parent', 9.0, 0);
            INSERT INTO sessions (id, source, started_at, end_reason, archived)
                VALUES ('compression-parent', 'telegram', 5.0, 'compression', 0);
            INSERT INTO sessions (id, source, parent_session_id, started_at, archived)
                VALUES ('compression-child', 'telegram', 'compression-parent', 6.0, 0);
            INSERT INTO sessions (id, source, parent_session_id, model_config, started_at, archived)
                VALUES ('subagent-child', 'subagent', 'branch-parent', '{\"_delegate_from\":\"branch-parent\"}', 7.0, 0);",
        )
        .unwrap();
        let state = test_app_state("http://127.0.0.1:9".to_string(), temp.path());

        let rows = fetch_filtered_sidebar_sessions_from_local_db(&state, 80)
            .unwrap()
            .unwrap();
        let ids = rows
            .iter()
            .filter_map(|row| row.get("id").and_then(|value| value.as_str()))
            .collect::<Vec<_>>();

        assert!(ids.contains(&"branch-child"));
        assert!(ids.contains(&"reset-child"));
        assert!(ids.contains(&"switch-parent"));
        assert!(!ids.contains(&"switch-child"));
        assert!(!ids.contains(&"compression-child"));
        assert!(!ids.contains(&"subagent-child"));
    }

    #[tokio::test]
    async fn pinned_session_outside_recent_window_is_appended_and_deduplicated() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let mut conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT,
                active INTEGER NOT NULL DEFAULT 1
             );
             CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                source TEXT,
                model TEXT,
                model_config TEXT,
                billing_provider TEXT,
                parent_session_id TEXT,
                started_at REAL,
                ended_at REAL,
                end_reason TEXT,
                message_count INTEGER,
                title TEXT,
                archived INTEGER,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT
             );",
        )
        .unwrap();
        let transaction = conn.transaction().unwrap();
        transaction.execute(
            "INSERT INTO sessions
             (id, source, model, started_at, message_count, title, archived)
             VALUES ('pinned-old', 'telegram', 'old-model', 1, 2, 'Pinned old', 0)",
            [],
        ).unwrap();
        transaction.execute(
            "INSERT INTO messages (session_id, role, content, active)
             VALUES ('pinned-old', 'assistant', 'old final', 1)",
            [],
        ).unwrap();
        let mut recent = Vec::new();
        for index in 0..80 {
            let id = format!("recent-{index}");
            transaction.execute(
                "INSERT INTO sessions
                 (id, source, model, started_at, message_count, title, archived)
                 VALUES (?1, 'telegram', 'new-model', ?2, 1, ?3, 0)",
                rusqlite::params![id, 100.0 + index as f64, format!("Recent {index}")],
            ).unwrap();
            recent.push(serde_json::json!({
                "id": format!("recent-{index}"),
                "source": "telegram",
                "started_at": 100.0 + index as f64,
            }));
        }
        transaction.commit().unwrap();
        drop(conn);
        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());

        let rows = append_pinned_session_rows(
            &state,
            recent,
            &["pinned-old".to_string(), "recent-79".to_string()],
        ).await;

        assert_eq!(rows.len(), 81);
        assert_eq!(rows.iter().filter(|row| row["id"] == "recent-79").count(), 1);
        let pinned = rows.iter().find(|row| row["id"] == "pinned-old").unwrap();
        assert_eq!(pinned["title"], "Pinned old");
        assert_eq!(pinned["preview"], "old final");
    }

    #[test]
    fn session_list_hides_reset_predecessor_when_successor_is_present() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                parent_session_id TEXT,
                title TEXT,
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
                active INTEGER NOT NULL DEFAULT 1
             );",
        ).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,title,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('old',NULL,'GFS路径过短原因 #1',1,10,'session_reset','telegram','agent:main:telegram:dm:1698432746','1698432746',NULL)", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,title,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('current',NULL,'GFS路径过短原因',10.007,NULL,NULL,'telegram','agent:main:telegram:dm:1698432746','1698432746',NULL)", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,title,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('switch-root',NULL,'原会话标题',20,30,'session_switch','telegram','same-key','chat',NULL)", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,title,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('switch-child','switch-root','错误的新会话标题',31,NULL,NULL,'telegram','same-key','chat',NULL)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,active) VALUES ('switch-root','assistant','旧消息',1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,active) VALUES ('switch-child','assistant','continuation 最新消息',1)", []).unwrap();
        drop(conn);
        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());
        let rows = vec![
            serde_json::json!({"id":"current","source":"telegram","title":"GFS路径过短原因","started_at":10.007,"ended_at":null}),
            serde_json::json!({"id":"old","source":"telegram","title":"GFS路径过短原因 #1","started_at":1.0,"ended_at":10.0,"end_reason":"session_reset"}),
            serde_json::json!({"id":"switch-child","source":"telegram","title":"错误的新会话标题","started_at":31.0,"ended_at":null}),
            serde_json::json!({"id":"switch-root","source":"telegram","title":"原会话标题","started_at":20.0,"ended_at":30.0,"end_reason":"session_switch"}),
        ];

        let rows = session_rows_with_local_lineage(&state, rows);
        let rows = enrich_session_rows_with_local_previews(&state, rows);

        assert_eq!(rows.iter().map(|row| row["id"].as_str().unwrap()).collect::<Vec<_>>(), vec!["current", "switch-root"]);
        assert_eq!(rows[1]["title"], "原会话标题");
        assert_eq!(rows[1]["preview"], "continuation 最新消息");
    }

    #[test]
    fn session_list_hides_dead_switch_child_under_resumed_reset_parent() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                parent_session_id TEXT,
                title TEXT,
                started_at REAL,
                ended_at REAL,
                end_reason TEXT,
                source TEXT,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT,
                model_config TEXT
             );
             CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT,
                active INTEGER NOT NULL DEFAULT 1
             );",
        ).unwrap();
        // The parent was reset away from, then resumed back (both end fields cleared).
        conn.execute("INSERT INTO sessions (id,parent_session_id,title,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id,model_config) VALUES ('resumed-parent',NULL,'阅读连裤袜',1,NULL,NULL,'telegram','same-key','chat',NULL,'{}')", []).unwrap();
        // The reset child was chatted with briefly, then switched away from for good.
        conn.execute("INSERT INTO sessions (id,parent_session_id,title,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id,model_config) VALUES ('dead-switch-child','resumed-parent','友好问候 #3',2,3,'session_switch','telegram','same-key','chat',NULL,'{\"_reset_from\":\"resumed-parent\"}')", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,active) VALUES ('resumed-parent','assistant','main transcript',1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,active) VALUES ('dead-switch-child','assistant','greeting',1)", []).unwrap();
        drop(conn);
        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());
        let rows = vec![
            serde_json::json!({"id":"dead-switch-child","source":"telegram","title":"友好问候 #3","started_at":2.0,"ended_at":3.0,"end_reason":"session_switch"}),
            serde_json::json!({"id":"resumed-parent","source":"telegram","title":"阅读连裤袜","started_at":1.0,"ended_at":null}),
        ];

        let rows = session_rows_with_local_lineage(&state, rows);
        let rows = enrich_session_rows_with_local_previews(&state, rows);

        assert_eq!(rows.iter().map(|row| row["id"].as_str().unwrap()).collect::<Vec<_>>(), vec!["resumed-parent"]);
        assert_eq!(rows[0]["preview"], "main transcript");
    }

    #[test]
    fn canonical_session_id_follows_same_source_session_switch_parent() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                parent_session_id TEXT,
                end_reason TEXT,
                source TEXT,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT
             );
             INSERT INTO sessions VALUES ('root',NULL,'session_switch','telegram','same-key','chat','topic');
             INSERT INTO sessions VALUES ('continuation','root',NULL,'telegram','same-key','chat','topic');
             INSERT INTO sessions VALUES ('branch','root',NULL,'telegram','other-key','chat','topic');",
        ).unwrap();
        drop(conn);
        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());

        assert_eq!(local_session_switch_root_id(&state, "continuation").unwrap(), Some("root".to_string()));
        assert_eq!(local_session_switch_root_id(&state, "branch").unwrap(), None);
        assert_eq!(local_session_switch_root_id(&state, "root").unwrap(), None);
    }

    #[test]
    fn canonical_session_id_follows_rootless_same_key_continuation() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                parent_session_id TEXT,
                end_reason TEXT,
                source TEXT,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT,
                started_at REAL
             );
             INSERT INTO sessions VALUES ('old-root',NULL,'session_switch','telegram','same-key','chat','topic',1.0);
             INSERT INTO sessions VALUES ('rootless-later',NULL,NULL,'telegram','same-key','chat','topic',10.0);
             INSERT INTO sessions VALUES ('unrelated',NULL,NULL,'telegram','other-key','chat','topic',11.0);",
        ).unwrap();
        drop(conn);
        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());

        assert_eq!(
            local_session_switch_root_id(&state, "rootless-later").unwrap(),
            Some("old-root".to_string())
        );
        assert_eq!(local_session_switch_root_id(&state, "unrelated").unwrap(), None);
    }

    #[tokio::test]
    async fn session_search_enriches_missing_preview_from_local_latest_message() {
        use std::collections::HashMap;

        async fn api_sessions(
            Query(query): Query<HashMap<String, String>>,
        ) -> Json<serde_json::Value> {
            assert_eq!(query.get("include_children").map(String::as_str), Some("false"));
            Json(serde_json::json!({
                "object": "list",
                "data": [
                    {"id":"s1","source":"telegram","title":"Needs preview","started_at":1.0,"message_count":3},
                    {"id":"s2","source":"telegram","title":"Incomplete turn","started_at":2.0,"message_count":2}
                ],
                "has_more": false
            }))
        }

        let app = Router::new().route("/api/sessions", get(api_sessions));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT,
                tool_calls TEXT,
                finish_reason TEXT,
                active INTEGER NOT NULL DEFAULT 1
             );",
        ).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,active) VALUES ('s1','user','first prompt',1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,active) VALUES ('s1','tool','tool output should not be preview',1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,finish_reason,active) VALUES ('s1','assistant','latest answer from rust','stop',1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,active) VALUES ('s2','user','unfinished question',1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,tool_calls,finish_reason,active) VALUES ('s2','assistant','intermediate commentary','[{\"id\":\"call_1\"}]','tool_calls',1)", []).unwrap();
        drop(conn);
        let state = test_app_state(format!("http://{addr}"), temp.path());

        let rows = fetch_sessions_from_api_server(&state, "", 10, false)
            .await
            .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["preview"], "latest answer from rust");
        assert_eq!(rows[1]["preview"], "unfinished question");
    }

    #[test]
    fn session_title_base_strips_lineage_suffix() {
        assert_eq!(session_title_base("Project rename #3"), "Project rename");
        assert_eq!(session_title_base("Project rename #1"), "Project rename #1");
        assert_eq!(session_title_for_lineage_index("Project rename", 2, false), "Project rename #3");
        assert_eq!(session_title_for_lineage_index("Project rename", 2, true), "Project rename");
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

        assert_eq!(payload["title"], "Unified title");
        assert_eq!(payload["base_title"], "Unified title");
        assert_eq!(payload["updated_ids"], serde_json::json!(["root", "parent", "child"]));
        assert_eq!(payload["titles"], serde_json::json!({
            "root": "Unified title #1",
            "parent": "Unified title #2",
            "child": "Unified title",
        }));
        assert_eq!(*patched.lock().unwrap(), vec![
            ("root".to_string(), "Unified title #1".to_string()),
            ("parent".to_string(), "Unified title #2".to_string()),
            ("child".to_string(), "Unified title".to_string()),
        ]);
    }

    #[tokio::test]
    async fn session_lineage_rename_skips_globally_occupied_titles() {
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
        ) -> Response<Body> {
            if method == Method::PATCH {
                let bytes = to_bytes(body, usize::MAX).await.unwrap();
                let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
                let title = payload["title"].as_str().unwrap_or_default().to_string();
                if title == "yahu" || title == "yahu #2" {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(serde_json::json!({
                            "error": {"message": format!("Title '{title}' is already in use by session other"), "code": "invalid_title"}
                        })),
                    ).into_response();
                }
                state.patched.lock().unwrap().push((session_id.clone(), title));
            }
            let parent = match session_id.as_str() {
                "child" => Some("parent"),
                "parent" => Some("root"),
                _ => None,
            };
            Json(serde_json::json!({
                "object": "hermes.session",
                "session": {"id": session_id, "parent_session_id": parent, "title": "old"}
            })).into_response()
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
            Json(SessionRenamePayload { title: "yahu".to_string() }),
        ).await;
        let body = axum::body::to_bytes(resp.into_response().into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["updated_ids"], serde_json::json!(["root", "parent", "child"]));
        assert_eq!(payload["titles"], serde_json::json!({
            "root": "yahu #1",
            "parent": "yahu #3",
            "child": "yahu #4",
        }));
        assert_eq!(payload["title"], "yahu #4");
        assert_eq!(*patched.lock().unwrap(), vec![
            ("root".to_string(), "yahu #1".to_string()),
            ("parent".to_string(), "yahu #3".to_string()),
            ("child".to_string(), "yahu #4".to_string()),
        ]);
    }

    #[tokio::test]
    async fn session_lineage_rename_from_reset_predecessor_includes_successor_session() {
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
            Json(serde_json::json!({
                "object": "hermes.session",
                "session": {"id": session_id, "parent_session_id": null, "title": "old"}
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
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                parent_session_id TEXT,
                title TEXT,
                started_at REAL,
                ended_at REAL,
                end_reason TEXT,
                source TEXT,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT
             );",
        ).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,title,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('old_root',NULL,'Old #1',1,2,'compression','telegram','agent:telegram:group:chat:topic','chat','topic')", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,title,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('old_tip','old_root','Old #2',2,10,'session_reset','telegram','agent:telegram:group:chat:topic','chat','topic')", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,title,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('current',NULL,'Old',10.5,NULL,NULL,'telegram','agent:telegram:group:chat:topic','chat','topic')", []).unwrap();
        let local_entries = local_session_rename_entries(&conn, "old_tip").unwrap();
        assert_eq!(local_entries.iter().map(|entry| entry.id.as_str()).collect::<Vec<_>>(), vec!["old_root", "old_tip", "current"]);
        drop(conn);
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));

        let resp = rename_session_lineage(
            State(state),
            AxumPath("old_tip".to_string()),
            Json(SessionRenamePayload { title: "Unified title".to_string() }),
        ).await;
        let body = axum::body::to_bytes(resp.into_response().into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(payload["updated_ids"], serde_json::json!(["old_root", "old_tip", "current"]));
        assert_eq!(payload["titles"], serde_json::json!({
            "old_root": "Unified title #1",
            "old_tip": "Unified title",
            "current": "Unified title #3",
        }));
        assert_eq!(*patched.lock().unwrap(), vec![
            ("old_root".to_string(), "Unified title #1".to_string()),
            ("old_tip".to_string(), "Unified title".to_string()),
            ("current".to_string(), "Unified title #3".to_string()),
        ]);
    }
    #[test]
    fn local_session_rename_entries_follows_unique_successor_without_thread_metadata() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                parent_session_id TEXT,
                title TEXT,
                started_at REAL,
                ended_at REAL,
                end_reason TEXT,
                source TEXT,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT
             );",
        ).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,title,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('legacy_reset',NULL,'Old #0',1,10,'session_reset','telegram',NULL,NULL,NULL)", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,title,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('legacy_next',NULL,'Old #1',10.004,20,'session_reset','telegram',NULL,NULL,NULL)", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,title,started_at,ended_at,end_reason,source,session_key,chat_id,thread_id) VALUES ('current',NULL,'Old',20.004,NULL,NULL,'telegram','agent:telegram:group:chat:topic','chat','topic')", []).unwrap();

        let entries = local_session_rename_entries(&conn, "legacy_reset").unwrap();

        assert_eq!(entries.iter().map(|entry| entry.id.as_str()).collect::<Vec<_>>(), vec!["legacy_reset", "legacy_next", "current"]);
    }

    #[test]
    fn session_list_merges_detached_chat_key_epoch_into_one_row() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                parent_session_id TEXT,
                title TEXT,
                started_at REAL,
                ended_at REAL,
                end_reason TEXT,
                source TEXT,
                session_key TEXT,
                chat_id TEXT,
                thread_id TEXT,
                model_config TEXT
             );
             CREATE TABLE messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT,
                active INTEGER NOT NULL DEFAULT 1
             );",
        ).unwrap();
        conn.execute("INSERT INTO sessions VALUES ('fresh',NULL,'yahu!',10,NULL,NULL,'telegram','same-key','chat','topic','{}')", []).unwrap();
        conn.execute("INSERT INTO sessions VALUES ('detour',NULL,'yahu! #11',2,9,'session_switch','telegram','same-key','chat','topic','{}')", []).unwrap();
        conn.execute("INSERT INTO sessions VALUES ('side','fresh','side note',11,NULL,NULL,'telegram','same-key','chat','topic','{}')", []).unwrap();
        drop(conn);
        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());
        let rows = vec![
            serde_json::json!({"id":"fresh","source":"telegram","title":"yahu!","started_at":10.0,"ended_at":null,"last_active":100.0}),
            serde_json::json!({"id":"detour","source":"telegram","title":"yahu! #11","started_at":2.0,"ended_at":9.0,"end_reason":"session_switch","last_active":50.0}),
            serde_json::json!({"id":"side","source":"telegram","title":"side note","started_at":11.0,"ended_at":null,"last_active":120.0}),
        ];

        let rows = session_rows_with_local_lineage(&state, rows);
        let ids = rows.iter().map(|row| row["id"].as_str().unwrap()).collect::<Vec<_>>();

        assert_eq!(ids, vec!["fresh", "side"]);
        assert_eq!(rows[0]["last_active"], 100.0);
    }
