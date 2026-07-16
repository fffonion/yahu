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
            active_chat_run_ids: Arc::new(RwLock::new(HashMap::new())),
            subagent_feeds: Arc::new(RwLock::new(HashMap::new())),
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
                    {"id":"s1","source":"telegram","model":"minimax/m3","title":"MiniMax billing","preview":"[Alliumcepa Triplef|1698432746]\ntoken cache math","started_at":1.0,"message_count":1},
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
        drop(conn);
        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());
        let rows = vec![
            serde_json::json!({"id":"current","source":"telegram","title":"GFS路径过短原因","started_at":10.007,"ended_at":null}),
            serde_json::json!({"id":"old","source":"telegram","title":"GFS路径过短原因 #1","started_at":1.0,"ended_at":10.0,"end_reason":"session_reset"}),
        ];

        let rows = session_rows_with_local_previews(&state, rows);

        assert_eq!(rows.iter().map(|row| row["id"].as_str().unwrap()).collect::<Vec<_>>(), vec!["current"]);
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
                    {"id":"s1","source":"telegram","title":"Needs preview","started_at":1.0,"message_count":3}
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
                active INTEGER NOT NULL DEFAULT 1
             );",
        ).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,active) VALUES ('s1','user','first prompt',1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,active) VALUES ('s1','tool','tool output should not be preview',1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,active) VALUES ('s1','assistant','latest answer from rust',1)", []).unwrap();
        drop(conn);
        let state = test_app_state(format!("http://{addr}"), temp.path());

        let rows = fetch_sessions_from_api_server(&state, "", 10)
            .await
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["preview"], "latest answer from rust");
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
