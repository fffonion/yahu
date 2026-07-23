    #[tokio::test]
    async fn chat_messages_skeleton_defers_turn_details_until_requested() {
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
                reasoning_details TEXT,
                codex_reasoning_items TEXT,
                active INTEGER NOT NULL DEFAULT 1
             );",
        ).unwrap();
        conn.execute("INSERT INTO sessions (id,parent_session_id,started_at,ended_at,end_reason,source) VALUES ('s1',NULL,1,NULL,NULL,'telegram')", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,timestamp,active) VALUES ('s1','user','do it',1,1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,tool_calls,reasoning_content,reasoning_details,codex_reasoning_items,timestamp,active) VALUES ('s1','assistant','I will inspect','[{\"id\":\"call_1\"}]','plan','[{\"type\":\"thinking\",\"thinking\":\"provider thought\",\"signature\":\"opaque-signature\"}]','[{\"type\":\"reasoning\",\"summary\":[{\"type\":\"summary_text\",\"text\":\"provider summary\"}],\"encrypted_content\":\"opaque-encrypted-payload\"}]',2,1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,tool_name,timestamp,active) VALUES ('s1','tool','{\"ok\":true}','terminal',3,1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,tool_calls,timestamp,active) VALUES ('s1','assistant','I will verify','[{\"id\":\"call_2\"}]',4,1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,tool_name,timestamp,active) VALUES ('s1','tool','{\"verified\":true}','browser',5,1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,timestamp,active) VALUES ('s1','assistant','final answer',6,1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,timestamp,active) VALUES ('s1','user','next prompt',7,1)", []).unwrap();
        conn.execute("INSERT INTO messages (session_id,role,content,tool_name,timestamp,active) VALUES ('s1','tool','unfinished detail','terminal',8,1)", []).unwrap();
        drop(conn);
        let state = Arc::new(test_app_state("http://127.0.0.1:1".to_string(), temp.path()));

        let skeleton = chat_messages_page(
            State(state.clone()),
            AxumPath("s1".to_string()),
            Query(ChatMessagesQuery { before: None, after: None, around: None, limit: Some(20), view: Some("skeleton".to_string()) }),
        ).await;
        let skeleton_body = axum::body::to_bytes(skeleton.into_body(), usize::MAX).await.unwrap();
        let skeleton_page: serde_json::Value = serde_json::from_slice(&skeleton_body).unwrap();
        let skeleton_data = skeleton_page["data"].as_array().unwrap();
        let skeleton_roles: Vec<_> = skeleton_data.iter().map(|message| message["role"].as_str().unwrap_or("")).collect();
        let skeleton_texts: Vec<_> = skeleton_data.iter().map(|message| message["content"].as_str().unwrap_or("")).collect();

        assert_eq!(skeleton_roles, vec!["user", "assistant", "user", "tool"]);
        assert_eq!(skeleton_texts, vec!["do it", "final answer", "next prompt", "unfinished detail"]);
        assert_eq!(skeleton_data[1]["turn_details"]["count"], 2);
        assert_eq!(skeleton_data[1]["turn_details"]["tool_count"], 2);
        assert_eq!(skeleton_data[1]["turn_details"]["thinking_count"], 0);
        assert_eq!(skeleton_data[1]["turn_details"]["after_id"], "1");
        assert_eq!(skeleton_data[1]["turn_details"]["before_id"], "6");
        assert_eq!(skeleton_data[1]["turn_details"]["commentary"].as_array().unwrap().len(), 2);
        assert_eq!(skeleton_data[1]["turn_details"]["commentary"][0]["id"], 2);
        assert_eq!(skeleton_data[1]["turn_details"]["commentary"][0]["role"], "assistant");
        assert_eq!(skeleton_data[1]["turn_details"]["commentary"][0]["content"], "I will inspect");
        assert!(skeleton_data[1]["turn_details"]["commentary"][0].get("tool_calls").is_none());
        assert_eq!(skeleton_data[1]["turn_details"]["commentary"][1]["id"], 4);
        assert_eq!(skeleton_data[1]["turn_details"]["commentary"][1]["content"], "I will verify");
        let timeline = skeleton_data[1]["turn_details"]["timeline"].as_array().unwrap();
        assert_eq!(timeline.iter().map(|item| item["kind"].as_str().unwrap_or("")).collect::<Vec<_>>(), vec!["commentary", "detail", "commentary", "detail"]);
        assert_eq!(timeline[0]["message"]["id"], 2);
        assert_eq!(timeline[1]["count"], 1);
        assert_eq!(timeline[1]["after_id"], "2");
        assert_eq!(timeline[1]["before_id"], "4");
        assert_eq!(timeline[2]["message"]["id"], 4);
        assert_eq!(timeline[3]["count"], 1);
        assert_eq!(timeline[3]["after_id"], "4");
        assert_eq!(timeline[3]["before_id"], "6");
        assert_eq!(skeleton_data[1]["reasoning"], "plan\nprovider thought\nprovider summary");
        assert!(!skeleton_data[1].to_string().contains("opaque-signature"));
        assert!(!skeleton_data[1].to_string().contains("opaque-encrypted-payload"));

        let default_resp = chat_messages_page(
            State(state.clone()),
            AxumPath("s1".to_string()),
            Query(ChatMessagesQuery { before: None, after: None, around: None, limit: Some(20), view: None }),
        ).await;
        let default_body = axum::body::to_bytes(default_resp.into_body(), usize::MAX).await.unwrap();
        let default_page: serde_json::Value = serde_json::from_slice(&default_body).unwrap();
        let default_roles: Vec<_> = default_page["data"].as_array().unwrap().iter().map(|message| message["role"].as_str().unwrap_or("")).collect();
        assert_eq!(default_roles, skeleton_roles);
        assert_eq!(default_page["data"].as_array().unwrap()[1]["turn_details"]["count"], 2);
        assert_eq!(default_page["data"].as_array().unwrap()[1]["turn_details"]["commentary"][0]["content"], "I will inspect");

        let details = chat_messages_page(
            State(state),
            AxumPath("s1".to_string()),
            Query(ChatMessagesQuery { before: Some(6), after: Some(1), around: None, limit: Some(20), view: Some("details".to_string()) }),
        ).await;
        let details_body = axum::body::to_bytes(details.into_body(), usize::MAX).await.unwrap();
        let details_page: serde_json::Value = serde_json::from_slice(&details_body).unwrap();
        let details_texts: Vec<_> = details_page["data"].as_array().unwrap().iter().map(|message| message["content"].as_str().unwrap_or("")).collect();
        assert_eq!(details_texts, vec!["I will inspect", "{\"ok\":true}", "I will verify", "{\"verified\":true}"]);
        assert_eq!(details_page["data"].as_array().unwrap()[0]["reasoning"], "plan\nprovider thought\nprovider summary");
    }

    #[test]
    fn history_skeleton_preserves_long_trailing_detail_segment_for_pagination() {
        let mut messages = vec![
            serde_json::json!({"id": 1, "role": "user", "content": "start"}),
            serde_json::json!({"id": 2, "role": "assistant", "content": "first answer"}),
        ];
        for id in 3..=32 {
            messages.push(serde_json::json!({
                "id": id,
                "role": "tool",
                "content": format!("tool result {id}"),
            }));
        }

        let skeleton = history_skeleton_messages(&messages);
        assert_eq!(skeleton.len(), 32);
        let query = ChatMessagesQuery {
            before: None,
            after: None,
            around: None,
            limit: Some(5),
            view: Some("skeleton".to_string()),
        };
        let (start, end) = page_bounds(&skeleton, &query, 5);
        let ids: Vec<_> = skeleton[start..end]
            .iter()
            .filter_map(message_i64_id)
            .collect();
        assert_eq!(ids, vec![28, 29, 30, 31, 32]);
        assert!(start > 0, "long trailing detail segment must expose older pagination");
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
            Query(ChatMessagesQuery { before: None, after: None, around: None, limit: Some(1), view: Some("full".to_string()) }),
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
            Query(ChatMessagesQuery { before: None, after: None, around: None, limit: Some(20), view: Some("full".to_string()) }),
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
    fn user_message_nav_does_not_pair_across_a_history_gap() {
        let messages = vec![
            serde_json::json!({"id": 1, "role": "user", "content": "last recoverable prompt"}),
            serde_json::json!({"id": -8_000_000_000_000i64, "role": "system", "content": "History coverage gap", "history_gap": {"after": 5, "before": 90_000}}),
            serde_json::json!({"id": 2, "role": "assistant", "content": "unrelated retained answer"}),
        ];

        let nav = build_user_message_nav(&messages);

        assert_eq!(nav.len(), 1);
        assert!(nav[0].assistant_preview.is_none());
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
