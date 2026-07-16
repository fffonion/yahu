    #[test]
    fn internal_model_switch_messages_are_hidden_from_watch_and_history() {
        let items = session_message_items(&serde_json::json!({"data":[
            {"id":1,"role":"user","content":"/model gpt-5.5 --provider openai-codex --session"},
            {"id":2,"role":"user","content":"real prompt"},
            {"id":3,"role":"assistant","content":"answer"}
        ]}));

        assert_eq!(items.iter().map(|item| item["content"].as_str().unwrap_or("")).collect::<Vec<_>>(), vec!["real prompt", "answer"]);
        assert_eq!(watch_message_window(items).len(), 2);
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
            Query(ChatMessagesQuery { before: None, after: None, around: None, limit: Some(20), view: Some("full".to_string()) }),
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
    async fn chat_history_includes_compacted_messages_without_context_window_counting_them() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, parent_session_id TEXT, started_at REAL, ended_at REAL, end_reason TEXT, source TEXT);
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
                active INTEGER NOT NULL DEFAULT 1,
                compacted INTEGER NOT NULL DEFAULT 0
             );",
        ).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,ended_at,end_reason,source) VALUES ('s1',NULL,1,NULL,NULL,'telegram')", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,timestamp,active,compacted) VALUES ('s1','user','older compacted prompt',1,0,1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,timestamp,active,compacted) VALUES ('s1','assistant','older compacted answer',2,0,1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,timestamp,active,compacted) VALUES ('s1','user','current active prompt',3,1,0)", []).unwrap();
        drop(conn);
        let state = Arc::new(test_app_state("http://127.0.0.1:1".to_string(), temp.path()));

        let resp = chat_messages_page(
            State(state.clone()),
            AxumPath("s1".to_string()),
            Query(ChatMessagesQuery { before: None, after: None, around: None, limit: Some(20), view: Some("full".to_string()) }),
        )
        .await;
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let page: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let texts: Vec<_> = page["data"].as_array().unwrap().iter().map(|message| message["content"].as_str().unwrap_or("")).collect();
        assert_eq!(texts, vec!["older compacted prompt", "older compacted answer", "current active prompt"]);

        let context = fetch_local_lineage_context_messages(&state, "s1").unwrap().unwrap();
        let context_texts: Vec<_> = context.messages.iter().map(|message| message["content"].as_str().unwrap_or("")).collect();
        assert_eq!(context_texts, vec!["current active prompt"]);
    }

    #[tokio::test]
    async fn chat_history_prefers_more_complete_local_visible_history_over_active_only_api() {
        async fn api_session(AxumPath(session_id): AxumPath<String>) -> Json<serde_json::Value> {
            Json(serde_json::json!({"session":{"id":session_id,"parent_session_id":null}}))
        }
        async fn api_messages(AxumPath(session_id): AxumPath<String>) -> Json<serde_json::Value> {
            Json(serde_json::json!({"data":[{
                "id": 3,
                "session_id": session_id,
                "role": "user",
                "content": "current active prompt",
                "timestamp": 3
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
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, parent_session_id TEXT, started_at REAL, ended_at REAL, end_reason TEXT, source TEXT);
             CREATE TABLE messages (
                id INTEGER PRIMARY KEY,
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
                active INTEGER NOT NULL DEFAULT 1,
                compacted INTEGER NOT NULL DEFAULT 0
             );",
        ).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,ended_at,end_reason,source) VALUES ('s1',NULL,1,NULL,NULL,'telegram')", []).unwrap();
        conn.execute("INSERT INTO messages (id,session_id,role,content,timestamp,active,compacted) VALUES (1,'s1','user','older compacted prompt',1,0,1)", []).unwrap();
        conn.execute("INSERT INTO messages (id,session_id,role,content,timestamp,active,compacted) VALUES (2,'s1','assistant','older compacted answer',2,0,1)", []).unwrap();
        conn.execute("INSERT INTO messages (id,session_id,role,content,timestamp,active,compacted) VALUES (3,'s1','user','current active prompt',3,1,0)", []).unwrap();
        drop(conn);
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));

        let messages = fetch_session_history_messages(&state, "s1").await.unwrap();
        let texts: Vec<_> = messages.iter().map(|message| message["content"].as_str().unwrap_or("")).collect();
        assert_eq!(texts, vec!["older compacted prompt", "older compacted answer", "current active prompt"]);
    }

    #[tokio::test]
    async fn chat_history_recovers_missing_pre_compaction_prefix_from_request_dump() {
        async fn api_session(AxumPath(session_id): AxumPath<String>) -> Json<serde_json::Value> {
            Json(serde_json::json!({"session":{"id":session_id,"parent_session_id":null}}))
        }
        async fn api_messages(AxumPath(session_id): AxumPath<String>) -> Json<serde_json::Value> {
            Json(serde_json::json!({"data":[{
                "id": 10,
                "session_id": session_id,
                "role": "user",
                "content": "current prompt",
                "timestamp": 10
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
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, parent_session_id TEXT, started_at REAL, ended_at REAL, end_reason TEXT, source TEXT);
             CREATE TABLE messages (
                id INTEGER PRIMARY KEY,
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
                active INTEGER NOT NULL DEFAULT 1,
                compacted INTEGER NOT NULL DEFAULT 0
             );",
        ).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,ended_at,end_reason,source) VALUES ('s1',NULL,1,NULL,NULL,'telegram')", []).unwrap();
        conn.execute("INSERT INTO messages (id,session_id,role,content,timestamp,active,compacted) VALUES (10,'s1','user','current prompt',10,1,0)", []).unwrap();
        drop(conn);

        let sessions_dir = temp.path().join("sessions");
        std::fs::create_dir(&sessions_dir).unwrap();
        std::fs::write(
            sessions_dir.join("request_dump_s1_19700101_000005_000000.json"),
            serde_json::to_vec(&serde_json::json!({
                "timestamp": "1970-01-01T00:00:05.000000",
                "session_id": "s1",
                "request": {
                    "headers": {"Authorization": "outer-secret-must-not-leak"},
                    "body": {"input": [
                    {"role": "user", "content": [{"type": "input_text", "text": "older prompt"}]},
                    {"type": "message", "role": "assistant", "content": [{"type": "output_text", "text": "older answer"}]},
                    {"type": "function_call", "call_id": "call-1", "name": "terminal", "arguments": "{\"command\":\"pwd\",\"password\":\"argument-secret-must-not-leak\"}"},
                    {"type": "function_call_output", "call_id": "call-1", "output": "older tool output"}
                ]}}
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(
            sessions_dir.join("request_dump_s1_19700101_000006_000000.json"),
            serde_json::to_vec(&serde_json::json!({
                "timestamp": "1970-01-01T00:00:06.000000",
                "session_id": "s1",
                "request": {"body": {"input": [
                    {"role": "user", "content": [{"type": "input_text", "text": "shorter newer snapshot"}]}
                ]}}
            }))
            .unwrap(),
        )
        .unwrap();

        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));
        let messages = fetch_session_history_messages(&state, "s1").await.unwrap();
        let roles: Vec<_> = messages.iter().map(|message| message["role"].as_str().unwrap_or("")).collect();
        let texts: Vec<_> = messages.iter().map(|message| message["content"].as_str().unwrap_or("")).collect();

        assert_eq!(roles, vec!["user", "assistant", "assistant", "tool", "user"]);
        assert_eq!(texts, vec!["older prompt", "older answer", "", "older tool output", "current prompt"]);
        assert_eq!(messages[2]["tool_calls"][0]["function"]["name"], "terminal");
        assert_eq!(messages[3]["tool_name"], "terminal");
        let serialized = serde_json::to_string(&messages).unwrap();
        assert!(!serialized.contains("outer-secret-must-not-leak"));
        assert!(!serialized.contains("argument-secret-must-not-leak"));
        assert!(serialized.contains("[REDACTED]"));

        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute("INSERT INTO messages (id,session_id,role,content,timestamp,active,compacted) VALUES (4,'s1','user','already preserved older prompt',4,0,1)", []).unwrap();
        drop(conn);
        let messages = fetch_session_history_messages(&state, "s1").await.unwrap();
        let texts: Vec<_> = messages
            .iter()
            .map(|message| message["content"].as_str().unwrap_or(""))
            .collect();
        assert_eq!(texts, vec!["already preserved older prompt", "current prompt"]);
    }

    #[tokio::test]
    async fn chat_history_trims_compression_child_carryover_prefix_without_content_dedupe() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("state.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (id TEXT PRIMARY KEY, parent_session_id TEXT, started_at REAL, ended_at REAL, end_reason TEXT, source TEXT);
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
        )
        .unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,ended_at,end_reason,source) VALUES ('root',NULL,10,20,'compression','telegram')", []).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,ended_at,end_reason,source) VALUES ('child','root',20,NULL,NULL,'telegram')", []).unwrap();
        for (sid, role, content, ts) in [
            ("root", "user", "who is lei", 11.0),
            ("root", "assistant", "lei answer", 12.0),
            ("root", "user", "fix yahu", 19.0),
            ("child", "user", "who is lei", 21.000),
            ("child", "assistant", "lei answer", 21.001),
            ("child", "user", "fix yahu", 21.002),
            ("child", "assistant", "", 21.003),
            ("child", "tool", "read source", 21.004),
            ("child", "assistant", "fixed yahu", 21.005),
            ("child", "user", "next real prompt", 90.0),
        ] {
            conn.execute(
                "INSERT INTO messages (session_id,role,content,timestamp,active) VALUES (?1,?2,?3,?4,1)",
                rusqlite::params![sid, role, content, ts],
            )
            .unwrap();
        }
        drop(conn);
        let state = Arc::new(test_app_state("http://127.0.0.1:1".to_string(), temp.path()));

        let resp = chat_messages_page(
            State(state),
            AxumPath("child".to_string()),
            Query(ChatMessagesQuery { before: None, after: None, around: None, limit: Some(20), view: Some("full".to_string()) }),
        )
        .await;
        let body = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let page: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let texts: Vec<_> = page["data"]
            .as_array()
            .unwrap()
            .iter()
            .map(|message| message["content"].as_str().unwrap_or(""))
            .collect();

        assert_eq!(page["total"], 7);
        assert_eq!(texts, vec![
            "who is lei",
            "lei answer",
            "fix yahu",
            "",
            "read source",
            "fixed yahu",
            "next real prompt",
        ]);
    }
