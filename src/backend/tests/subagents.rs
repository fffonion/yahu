    #[test]
    fn subagent_projection_reports_task_current_tool_todos_and_summary() {
        let session = serde_json::json!({
            "id": "child-1",
            "parent_session_id": "parent-1",
            "model": "gpt-5.6-sol",
            "started_at": 100.0,
            "ended_at": null,
            "message_count": 6,
            "tool_call_count": 2,
            "api_call_count": 3
        });
        let messages = vec![
            serde_json::json!({"id": 1, "role": "user", "content": "Review the backend", "timestamp": 100.0}),
            serde_json::json!({"id": 2, "role": "assistant", "content": "", "timestamp": 101.0, "tool_calls": [{"id": "todo-1", "function": {"name": "todo", "arguments": "{}"}}]}),
            serde_json::json!({"id": 3, "role": "tool", "tool_name": "todo", "tool_call_id": "todo-1", "timestamp": 101.1, "content": "{\"todos\":[{\"id\":\"inspect\",\"content\":\"Inspect files\",\"status\":\"completed\"},{\"id\":\"test\",\"content\":\"Run tests\",\"status\":\"in_progress\"}]}"}),
            serde_json::json!({"id": 4, "role": "assistant", "content": "", "timestamp": 102.0, "tool_calls": [{"id": "term-1", "function": {"name": "terminal", "arguments": "{\"command\":\"cargo test\"}"}}]}),
            serde_json::json!({"id": 5, "role": "assistant", "content": "Partial review note", "timestamp": 102.5}),
        ];

        let projected = project_subagent_session(&session, &messages).unwrap();

        assert_eq!(projected.session_id, "child-1");
        assert_eq!(projected.task, "Review the backend");
        assert_eq!(projected.status, "running");
        assert_eq!(projected.current_tool.as_deref(), Some("terminal"));
        assert_eq!(projected.todos.len(), 2);
        assert_eq!(projected.todos[1].status, "in_progress");
        assert_eq!(projected.summary.as_deref(), Some("Partial review note"));
        assert_eq!(projected.activity.last().unwrap().tool, "todo");
    }

    #[test]
    fn persistent_goal_is_loaded_separately_from_subagent_tasks() {
        let temp = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open(temp.path().join("state.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE state_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO state_meta (key, value) VALUES (?1, ?2)",
            rusqlite::params![
                "goal:parent-1",
                serde_json::json!({
                    "goal": "Optimize the interpreter",
                    "status": "active",
                    "turns_used": 4,
                    "max_turns": 20,
                    "last_reason": "More profiling is required",
                    "subgoals": ["Keep the script implementation"]
                })
                .to_string()
            ],
        )
        .unwrap();
        drop(conn);

        let goal = load_persistent_goal(temp.path(), "parent-1")
            .unwrap()
            .unwrap();

        assert_eq!(goal.text, "Optimize the interpreter");
        assert_eq!(goal.status, "active");
        assert_eq!(goal.turns_used, 4);
        assert_eq!(goal.max_turns, 20);
        assert_eq!(goal.last_reason.as_deref(), Some("More profiling is required"));
        assert_eq!(goal.subgoals, vec!["Keep the script implementation"]);
        assert!(goal.todos.is_empty());
        assert!(load_persistent_goal(temp.path(), "child-1").unwrap().is_none());
    }

    #[tokio::test]
    async fn persistent_goal_todos_are_loaded_from_parent_api_messages() {
        async fn api_session() -> Json<Value> {
            Json(serde_json::json!({
                "session": { "id": "parent-1", "message_count": 3 }
            }))
        }
        async fn api_messages() -> Json<Value> {
            Json(serde_json::json!({
                "data": [
                    { "role": "tool", "tool_name": "todo", "content": "{\"todos\":[{\"id\":\"old\",\"content\":\"Old task\",\"status\":\"completed\"}]}" },
                    { "role": "assistant", "content": "", "tool_calls": [{ "id": "todo-current", "function": { "name": "todo", "arguments": "{\"todos\":[{\"id\":\"current\",\"content\":\"Current main task\",\"status\":\"in_progress\"}]}" } }] },
                    { "role": "tool", "tool_name": "todo", "tool_call_id": "todo-current", "content": "[todo] updated task list" }
                ]
            }))
        }

        let app = Router::new()
            .route("/api/sessions/{session_id}", get(api_session))
            .route("/api/sessions/{session_id}/messages", get(api_messages));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state(format!("http://{addr}"), temp.path());
        let mut cache = CachedParentTodos::default();

        let todos = fetch_parent_session_todos(&state, "parent-1", &mut cache)
            .await
            .unwrap();

        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].id, "current");
        assert_eq!(todos[0].content, "Current main task");
        assert_eq!(todos[0].status, "in_progress");
    }

    #[test]
    fn subagent_projection_does_not_expose_tool_arguments_or_results() {
        let session = serde_json::json!({
            "id": "child-secret",
            "parent_session_id": "parent-1",
            "started_at": 100.0,
            "ended_at": null
        });
        let messages = vec![
            serde_json::json!({"role": "user", "content": "Check auth"}),
            serde_json::json!({"role": "assistant", "tool_calls": [{"id": "term-1", "function": {"name": "terminal", "arguments": "{\"command\":\"curl -H Authorization:secret\"}"}}]}),
            serde_json::json!({"role": "tool", "tool_name": "terminal", "tool_call_id": "term-1", "content": "API_KEY=secret-value", "timestamp": 101.0}),
        ];

        let projected = project_subagent_session(&session, &messages).unwrap();
        let encoded = serde_json::to_string(&projected).unwrap();

        assert!(!encoded.contains("Authorization:secret"));
        assert!(!encoded.contains("secret-value"));
        assert_eq!(projected.activity[0].tool, "terminal");
    }

    #[test]
    fn visible_subagent_sessions_keep_active_batch_and_five_recent_roots_with_descendants() {
        let sessions = vec![
            serde_json::json!({"id": "old-a", "parent_session_id": "parent", "started_at": 10.0, "ended_at": 20.0}),
            serde_json::json!({"id": "root-a", "parent_session_id": "parent", "started_at": 100.0, "ended_at": null}),
            serde_json::json!({"id": "old-b", "parent_session_id": "parent", "started_at": 20.0, "ended_at": 25.0}),
            serde_json::json!({"id": "root-b", "parent_session_id": "parent", "started_at": 100.2, "ended_at": 110.0}),
            serde_json::json!({"id": "old-c", "parent_session_id": "parent", "started_at": 30.0, "ended_at": 35.0}),
            serde_json::json!({"id": "recent", "parent_session_id": "parent", "started_at": 90.0, "ended_at": 95.0}),
            serde_json::json!({"id": "nested", "parent_session_id": "root-a", "started_at": 105.0, "ended_at": null}),
            serde_json::json!({"id": "other", "parent_session_id": "another", "started_at": 120.0, "ended_at": null}),
        ];

        let visible = select_visible_subagent_sessions("parent", &sessions);
        let direct_ids = visible
            .iter()
            .filter(|item| item.get("parent_session_id").and_then(Value::as_str) == Some("parent"))
            .filter_map(|item| item.get("id").and_then(Value::as_str))
            .collect::<Vec<_>>();
        let all_ids = visible
            .iter()
            .filter_map(|item| item.get("id").and_then(Value::as_str))
            .collect::<Vec<_>>();

        assert_eq!(direct_ids, vec!["root-b", "root-a", "recent", "old-c", "old-b"]);
        assert!(all_ids.contains(&"nested"));
        assert!(!all_ids.contains(&"old-a"));
        assert!(!all_ids.contains(&"other"));
    }

    #[test]
    fn visible_subagent_sessions_keep_five_recent_roots_when_idle() {
        let sessions = vec![
            serde_json::json!({"id": "old-a", "parent_session_id": "parent", "started_at": 10.0, "ended_at": 15.0}),
            serde_json::json!({"id": "old-b", "parent_session_id": "parent", "started_at": 20.0, "ended_at": 25.0}),
            serde_json::json!({"id": "latest-a", "parent_session_id": "parent", "started_at": 100.0, "ended_at": 120.0}),
            serde_json::json!({"id": "old-c", "parent_session_id": "parent", "started_at": 30.0, "ended_at": 35.0}),
            serde_json::json!({"id": "latest-b", "parent_session_id": "parent", "started_at": 100.3, "ended_at": 122.0}),
            serde_json::json!({"id": "old-d", "parent_session_id": "parent", "started_at": 40.0, "ended_at": 45.0}),
            serde_json::json!({"id": "nested", "parent_session_id": "latest-a", "started_at": 104.0, "ended_at": 119.0}),
        ];

        let visible = select_visible_subagent_sessions("parent", &sessions);
        let direct_ids = visible
            .iter()
            .filter(|item| item.get("parent_session_id").and_then(Value::as_str) == Some("parent"))
            .filter_map(|item| item.get("id").and_then(Value::as_str))
            .collect::<Vec<_>>();
        let all_ids = visible
            .iter()
            .filter_map(|item| item.get("id").and_then(Value::as_str))
            .collect::<Vec<_>>();

        assert_eq!(direct_ids, vec!["latest-b", "latest-a", "old-d", "old-c", "old-b"]);
        assert!(all_ids.contains(&"nested"));
        assert!(!all_ids.contains(&"old-a"));
    }

    #[test]
    fn subagent_websocket_rejects_cross_origin_browser_handshakes() {
        let mut same_origin = HeaderMap::new();
        same_origin.insert(header::HOST, HeaderValue::from_static("yahu.example:443"));
        same_origin.insert(header::ORIGIN, HeaderValue::from_static("https://yahu.example:443"));
        assert!(subagent_websocket_origin_allowed(&same_origin));

        let mut cross_origin = same_origin.clone();
        cross_origin.insert(header::ORIGIN, HeaderValue::from_static("https://attacker.example"));
        assert!(!subagent_websocket_origin_allowed(&cross_origin));

        let mut native_client = HeaderMap::new();
        native_client.insert(header::HOST, HeaderValue::from_static("127.0.0.1:9642"));
        assert!(subagent_websocket_origin_allowed(&native_client));
    }

    #[test]
    fn subagent_feed_registry_reuses_one_broadcast_channel_per_session() {
        let mut feeds = HashMap::new();
        let (first, first_created) = subagent_feed_sender(&mut feeds, "parent");
        let (second, second_created) = subagent_feed_sender(&mut feeds, "parent");

        assert!(first_created);
        assert!(!second_created);
        assert!(first.same_channel(&second));
        assert_eq!(feeds.len(), 1);
    }

    #[test]
    fn completed_subagent_batches_use_the_idle_poll_interval() {
        fn projection(status: &str) -> SubagentProjection {
            SubagentProjection {
                session_id: "child".to_string(),
                parent_session_id: "parent".to_string(),
                task: "Review".to_string(),
                model: None,
                status: status.to_string(),
                started_at: Some(1.0),
                ended_at: (status != "running").then_some(2.0),
                message_count: 1,
                tool_count: 0,
                api_calls: 0,
                current_tool: None,
                todos: Vec::new(),
                activity: Vec::new(),
                summary: None,
            }
        }

        assert_eq!(subagent_poll_delay(&[], None), SUBAGENT_POLL_INTERVAL);
        assert_eq!(
            subagent_poll_delay(&[projection("running")], None),
            SUBAGENT_POLL_INTERVAL
        );
        assert_eq!(
            subagent_poll_delay(&[projection("completed")], None),
            SUBAGENT_IDLE_POLL_INTERVAL
        );
        let active_goal = PersistentGoalProjection {
            text: "Goal".to_string(),
            status: "active".to_string(),
            turns_used: 1,
            max_turns: 20,
            subgoals: Vec::new(),
            todos: Vec::new(),
            last_reason: None,
            paused_reason: None,
        };
        assert_eq!(
            subagent_poll_delay(&[projection("completed")], Some(&active_goal)),
            SUBAGENT_POLL_INTERVAL
        );
    }

    #[test]
    fn subagent_message_details_are_exposed_through_an_authenticated_lazy_route() {
        let backend_source = include_str!("../mod.rs");
        assert!(backend_source.contains("\"/chat/subagents/{session_id}/messages\""));
    }

    #[test]
    fn subagent_websocket_uses_api_unauthorized_response_path() {
        let auth_source = include_str!("../auth.rs");
        assert!(auth_source.contains("path.starts_with(\"/chat/subagents\")"));
    }
