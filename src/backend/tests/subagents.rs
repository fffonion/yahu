    #[test]
    fn subagent_projection_reports_task_current_tool_todos_and_summary() {
        let session = serde_json::json!({
            "id": "child-1",
            "parent_session_id": "parent-1",
            "model": "gpt-5.6-sol",
            "started_at": 100.0,
            "last_active": 100.0,
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

        let projected = project_subagent_session(Path::new("/nonexistent"), &session, &messages).unwrap();

        assert_eq!(projected.session_id, "child-1");
        assert_eq!(projected.task, "Review the backend");
        assert_eq!(projected.model.as_deref(), Some("gpt-5.6-sol"));
        assert_eq!(projected.status, "running");
        assert_eq!(projected.current_tool.as_deref(), Some("terminal"));
        assert_eq!(projected.todos.len(), 2);
        assert_eq!(projected.todos[1].status, "in_progress");
        assert_eq!(projected.summary.as_deref(), Some("Partial review note"));
        assert_eq!(projected.activity.last().unwrap().tool, "todo");
    }

    #[test]
    fn subagent_projection_keeps_original_task_after_compaction_marker() {
        let session = serde_json::json!({
            "id": "child-compacted",
            "parent_session_id": "parent-1",
            "started_at": 100.0,
            "message_count": 3
        });
        let messages = vec![
            serde_json::json!({
                "role": "user",
                "content": "[CONTEXT COMPACTION — REFERENCE ONLY]\nsummary of the earlier task",
                "timestamp": 200.0
            }),
            serde_json::json!({
                "role": "assistant",
                "content": "Continuing the task",
                "timestamp": 201.0
            }),
            serde_json::json!({
                "role": "user",
                "content": "Recover and finish the original subagent task.",
                "timestamp": 202.0
            }),
        ];

        let projected = project_subagent_session(Path::new("/nonexistent"), &session, &messages).unwrap();

        assert_eq!(projected.task, "Recover and finish the original subagent task.");
    }

    #[test]
    fn stale_running_subagent_is_marked_interrupted_without_hermes_changes() {
        let session = serde_json::json!({
            "id": "stale-child",
            "parent_session_id": "parent-1",
            "started_at": 100.0,
            "ended_at": null,
            "message_count": 1
        });
        let messages = vec![serde_json::json!({
            "role": "user",
            "content": "Long task",
            "timestamp": 100.0
        })];
        let projection = project_subagent_session(Path::new("/nonexistent"), &session, &messages).unwrap();
        let projection = mark_stale_running_subagent(projection, Some(100.0), 100.0 + SUBAGENT_STALE_RUNNING_SECONDS);

        assert_eq!(projection.status, "interrupted");
        assert_eq!(projection.ended_at, Some(100.0));
        assert_eq!(projection.current_tool, None);
    }

    #[test]
    fn subagent_context_is_loaded_from_the_matching_parent_delegate_call() {
        let temp = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open(temp.path().join("state.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
                id INTEGER PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                timestamp REAL NOT NULL,
                tool_calls TEXT
            );",
        )
        .unwrap();
        let calls = serde_json::json!([{
            "function": {
                "name": "delegate_task",
                "arguments": serde_json::json!({
                    "goal": "Review the backend",
                    "context": "Use the read-only architecture checklist."
                }).to_string()
            }
        }]).to_string();
        conn.execute(
            "INSERT INTO messages (id, session_id, role, timestamp, tool_calls) VALUES (1, ?1, 'assistant', 99.0, ?2)",
            rusqlite::params!["parent-1", calls],
        )
        .unwrap();
        drop(conn);

        assert_eq!(
            load_subagent_context(temp.path(), "parent-1", 100.0, "Review the backend").as_deref(),
            Some("Use the read-only architecture checklist.")
        );
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
                    "milestones": [
                        { "turn": 2, "timestamp": 200.0, "verdict": "continue", "reason": "Second result" },
                        { "turn": 4, "timestamp": 400.0, "verdict": "continue", "reason": "Latest result" },
                        { "turn": 1, "timestamp": 100.0, "verdict": "continue", "reason": "First result" }
                    ],
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
        assert_eq!(goal.created_at, 0.0);
        assert_eq!(goal.last_turn_at, 0.0);
        assert_eq!(goal.turns_used, 4);
        assert_eq!(goal.max_turns, 20);
        assert_eq!(goal.last_reason.as_deref(), Some("More profiling is required"));
        assert_eq!(goal.subgoals, vec!["Keep the script implementation"]);
        assert!(goal.todos.is_empty());
        assert_eq!(goal.milestones.iter().map(|item| item.turn).collect::<Vec<_>>(), vec![4, 2, 1]);
        let conn = rusqlite::Connection::open(temp.path().join("state.db")).unwrap();
        conn.execute(
            "UPDATE state_meta SET value = ?2 WHERE key = ?1",
            rusqlite::params![
                "goal:parent-1",
                serde_json::json!({
                    "goal": "Optimize the interpreter",
                    "status": "active",
                    "turns_used": 5,
                    "max_turns": 20,
                    "last_turn_at": 500.0,
                    "last_reason": "Fifth result",
                    "subgoals": []
                })
                .to_string()
            ],
        )
        .unwrap();
        drop(conn);
        let updated = load_persistent_goal(temp.path(), "parent-1")
            .unwrap()
            .unwrap();
        assert_eq!(updated.last_turn_at, 500.0);
        assert_eq!(
            updated
                .milestones
                .iter()
                .map(|item| item.turn)
                .collect::<Vec<_>>(),
            vec![5, 4, 2, 1]
        );
        assert!(load_persistent_goal(temp.path(), "child-1").unwrap().is_none());
    }

    #[test]
    fn completed_goal_is_not_loaded_as_a_standing_goal() {
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
                    "goal": "Archived completed goal",
                    "status": "done",
                    "turns_used": 3,
                    "max_turns": 20,
                    "last_verdict": "done",
                    "last_reason": "goal satisfied"
                })
                .to_string()
            ],
        )
        .unwrap();
        drop(conn);

        assert!(load_persistent_goal(temp.path(), "parent-1")
            .unwrap()
            .is_none());
    }

    #[test]
    fn replacing_a_goal_drops_milestones_from_the_previous_goal_generation() {
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
                    "goal": "Previous goal",
                    "status": "active",
                    "created_at": 100.0,
                    "turns_used": 1,
                    "last_turn_at": 150.0,
                    "last_reason": "Previous result",
                    "milestones": [{
                        "turn": 1,
                        "timestamp": 150.0,
                        "verdict": "continue",
                        "reason": "Previous result"
                    }]
                })
                .to_string()
            ],
        )
        .unwrap();
        drop(conn);

        let previous = load_persistent_goal(temp.path(), "parent-1")
            .unwrap()
            .unwrap();
        assert_eq!(previous.milestones.len(), 1);

        let conn = rusqlite::Connection::open(temp.path().join("state.db")).unwrap();
        conn.execute(
            "UPDATE state_meta SET value = ?2 WHERE key = ?1",
            rusqlite::params![
                "goal:parent-1",
                serde_json::json!({
                    "goal": "Replacement goal",
                    "status": "active",
                    "created_at": 300.0,
                    "turns_used": 0,
                    "last_turn_at": 0.0
                })
                .to_string()
            ],
        )
        .unwrap();
        drop(conn);

        let replacement = load_persistent_goal(temp.path(), "parent-1")
            .unwrap()
            .unwrap();
        assert_eq!(replacement.text, "Replacement goal");
        assert!(replacement.milestones.is_empty());
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
                "messages": [
                    { "role": "assistant", "content": "", "tool_calls": [{ "id": "todo-full", "function": { "name": "todo", "arguments": "{\"todos\":[{\"id\":\"keep\",\"content\":\"Keep this task\",\"status\":\"pending\"},{\"id\":\"current\",\"content\":\"Current main task\",\"status\":\"in_progress\"}]}" } }] },
                    { "role": "tool", "tool_name": "todo", "tool_call_id": "todo-full", "content": "[todo] updated task list" },
                    { "role": "assistant", "content": "", "tool_calls": [{ "id": "todo-merge", "function": { "name": "todo", "arguments": "{\"merge\":true,\"todos\":[{\"id\":\"current\",\"status\":\"completed\"}]}" } }] },
                    { "role": "tool", "tool_name": "todo", "tool_call_id": "todo-merge", "content": "[todo] updated task list" }
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

        let todos = fetch_parent_session_todos(&state, "parent-1", 0.0, &mut cache)
            .await
            .unwrap();

        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].id, "keep");
        assert_eq!(todos[0].status, "pending");
        assert_eq!(todos[1].id, "current");
        assert_eq!(todos[1].content, "Current main task");
        assert_eq!(todos[1].status, "completed");
    }

    #[tokio::test]
    async fn replacing_a_goal_drops_todos_from_the_previous_goal_generation() {
        async fn api_session() -> Json<Value> {
            Json(serde_json::json!({
                "session": { "id": "parent-1", "message_count": 1 }
            }))
        }
        async fn api_messages() -> Json<Value> {
            Json(serde_json::json!({
                "data": [{
                    "role": "assistant",
                    "timestamp": 100.0,
                    "tool_calls": [{
                        "function": {
                            "name": "todo",
                            "arguments": {
                                "todos": [{
                                    "id": "previous",
                                    "content": "Previous goal task",
                                    "status": "in_progress"
                                }]
                            }
                        }
                    }]
                }]
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

        let previous = fetch_parent_session_todos(&state, "parent-1", 50.0, &mut cache)
            .await
            .unwrap();
        assert_eq!(previous.len(), 1);

        let replacement = fetch_parent_session_todos(&state, "parent-1", 200.0, &mut cache)
            .await
            .unwrap();
        assert!(replacement.is_empty());
        assert!(cache.todos.is_empty());
    }

    #[test]
    fn preserved_todo_state_seeds_replay_after_compression() {
        let messages = vec![
            serde_json::json!({
                "role": "user",
                "content": "[Your active task list was preserved across context compression]\n- [>] keep. Keep this task (in_progress)\n- [ ] current. Current main task (pending)"
            }),
            serde_json::json!({
                "role": "assistant",
                "tool_calls": [{
                    "function": {
                        "name": "todo",
                        "arguments": "{\"merge\":true,\"todos\":[{\"id\":\"current\",\"status\":\"completed\"}]}"
                    }
                }]
            }),
            serde_json::json!({
                "role": "tool",
                "tool_name": "todo",
                "content": "[todo] updated task list"
            }),
        ];

        let todos = latest_todos(&messages);
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].id, "keep");
        assert_eq!(todos[0].status, "in_progress");
        assert_eq!(todos[1].id, "current");
        assert_eq!(todos[1].content, "Current main task");
        assert_eq!(todos[1].status, "completed");
    }

    #[test]
    fn compacted_goal_todos_are_available_when_the_api_window_has_no_todo_state() {
        let api_messages = vec![serde_json::json!({
            "role": "user",
            "timestamp": 300.0,
            "content": "Continue"
        })];
        let local_messages = vec![serde_json::json!({
            "role": "user",
            "timestamp": 200.0,
            "content": "[Your active task list was preserved across context compression]\n- [>] implementation. Complete the implementation (in_progress)\n- [ ] report. Write the report (pending)"
        })];

        assert!(latest_todos_state_since(&api_messages, 100.0).is_none());
        let todos = latest_todos_state_since(&local_messages, 100.0).unwrap();
        assert_eq!(todos.len(), 2);
        assert_eq!(todos[0].id, "implementation");
        assert_eq!(todos[1].id, "report");
    }

    #[test]
    fn local_goal_todo_history_includes_compacted_preserved_state() {
        let temp = tempfile::tempdir().unwrap();
        let conn = rusqlite::Connection::open(temp.path().join("state.db")).unwrap();
        conn.execute_batch(
            "CREATE TABLE messages (
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
        )
        .unwrap();
        conn.execute(
            "INSERT INTO messages (session_id, role, content, timestamp, active, compacted) VALUES (?1, 'user', ?2, 200.0, 0, 1)",
            rusqlite::params![
                "parent-1",
                "[Your active task list was preserved across context compression]\n- [>] implementation. Complete the implementation (in_progress)"
            ],
        )
        .unwrap();
        drop(conn);
        let state = test_app_state("http://127.0.0.1:1".to_string(), temp.path());

        let messages = fetch_local_goal_todo_messages(&state, "parent-1", 100.0)
            .unwrap()
            .unwrap();
        let todos = latest_todos_state_since(&messages, 100.0).unwrap();
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].id, "implementation");
    }

    #[test]
    fn explicit_empty_api_todo_state_does_not_request_a_history_fallback() {
        let api_messages = vec![serde_json::json!({
            "role": "assistant",
            "timestamp": 300.0,
            "tool_calls": [{"function": {"name": "todo", "arguments": {"todos": []}}}]
        })];

        assert_eq!(latest_todos_state_since(&api_messages, 100.0), Some(Vec::new()));
    }

    #[test]
    fn todo_merge_deduplicates_incoming_ids_before_applying_updates() {
        let messages = vec![
            serde_json::json!({
                "role": "assistant",
                "tool_calls": [{"function": {"name": "todo", "arguments": {
                    "todos": [{"id": "task", "content": "Original", "status": "pending"}]
                }}}]
            }),
            serde_json::json!({
                "role": "assistant",
                "tool_calls": [{"function": {"name": "todo", "arguments": {
                    "merge": true,
                    "todos": [
                        {"id": "task", "content": "Discarded duplicate", "status": "in_progress"},
                        {"id": "task", "status": "completed"}
                    ]
                }}}]
            }),
        ];

        let todos = latest_todos(&messages);
        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].content, "Original");
        assert_eq!(todos[0].status, "completed");
    }

    #[test]
    fn todo_merge_never_grows_a_full_projection_past_its_bound() {
        let initial = (0..100)
            .map(|index| serde_json::json!({
                "id": format!("task-{index}"),
                "content": format!("Task {index}"),
                "status": "pending"
            }))
            .collect::<Vec<_>>();
        let messages = vec![
            serde_json::json!({
                "role": "assistant",
                "tool_calls": [{"function": {"name": "todo", "arguments": {"todos": initial}}}]
            }),
            serde_json::json!({
                "role": "assistant",
                "tool_calls": [{"function": {"name": "todo", "arguments": {
                    "merge": true,
                    "todos": [
                        {"id": "overflow-a", "content": "Must be omitted", "status": "pending"},
                        {"id": "task-99", "status": "completed"},
                        {"id": "overflow-b", "content": "Must also be omitted", "status": "pending"}
                    ]
                }}}]
            }),
        ];

        let todos = latest_todos(&messages);
        assert_eq!(todos.len(), 100);
        assert_eq!(todos.last().unwrap().id, "task-99");
        assert_eq!(todos.last().unwrap().status, "completed");
        assert!(!todos.iter().any(|todo| todo.id.starts_with("overflow-")));
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

        let projected = project_subagent_session(Path::new("/nonexistent"), &session, &messages).unwrap();
        let encoded = serde_json::to_string(&projected).unwrap();

        assert!(!encoded.contains("Authorization:secret"));
        assert!(!encoded.contains("secret-value"));
        assert_eq!(projected.activity[0].tool, "terminal");
    }

    #[test]
    fn visible_subagent_sessions_keep_an_old_running_child_visible() {
        let window_end = 200_000.0;
        let sessions = vec![serde_json::json!({
            "id": "old-running",
            "parent_session_id": "parent",
            "started_at": window_end - SUBAGENT_LOOKBACK_SECONDS - 1.0,
            "ended_at": null
        })];

        let visible = select_visible_subagent_sessions("parent", &sessions, window_end);

        assert_eq!(visible.len(), 1);
        assert_eq!(string_field(&visible[0], "id").as_deref(), Some("old-running"));
    }

    #[test]
    fn visible_subagent_sessions_follow_the_backward_twelve_hour_window_and_limit_ten() {
        assert_eq!(SUBAGENT_LOOKBACK_SECONDS, 43_200.0);
        let window_end = 200_000.0;
        let mut sessions = (0..12)
            .map(|index| serde_json::json!({
                "id": format!("root-{index}"),
                "parent_session_id": "parent",
                "started_at": window_end - f64::from(index) * 100.0,
                "ended_at": window_end - f64::from(index) * 50.0
            }))
            .collect::<Vec<_>>();
        sessions.extend([
            serde_json::json!({"id": "nested", "parent_session_id": "root-0", "started_at": window_end - 25.0, "ended_at": window_end - 10.0}),
            serde_json::json!({"id": "future", "parent_session_id": "parent", "started_at": window_end + 0.1, "ended_at": null}),
            serde_json::json!({"id": "too-old", "parent_session_id": "parent", "started_at": window_end - SUBAGENT_LOOKBACK_SECONDS - 0.1, "ended_at": window_end}),
            serde_json::json!({"id": "other", "parent_session_id": "another", "started_at": window_end - 1.0, "ended_at": null}),
        ]);

        let visible = select_visible_subagent_sessions("parent", &sessions, window_end);
        let ids = visible
            .iter()
            .filter_map(|item| item.get("id").and_then(Value::as_str))
            .collect::<Vec<_>>();

        assert_eq!(visible.len(), SUBAGENT_VISIBLE_LIMIT);
        assert_eq!(ids[0], "root-0");
        assert_eq!(ids[1], "nested");
        assert!(!ids.contains(&"future"));
        assert!(!ids.contains(&"too-old"));
        assert!(!ids.contains(&"other"));
        assert!(visible.iter().all(|item| {
            let started_at = number_field(item, "started_at").unwrap();
            started_at >= window_end - SUBAGENT_LOOKBACK_SECONDS && started_at <= window_end
        }));
    }

    #[test]
    fn visible_subagent_sessions_include_both_time_window_boundaries() {
        let window_end = 500_000.0;
        let sessions = vec![
            serde_json::json!({"id": "at-start", "parent_session_id": "parent", "started_at": window_end - SUBAGENT_LOOKBACK_SECONDS}),
            serde_json::json!({"id": "at-end", "parent_session_id": "parent", "started_at": window_end}),
            serde_json::json!({"id": "before-start", "parent_session_id": "parent", "started_at": window_end - SUBAGENT_LOOKBACK_SECONDS - 1.0, "ended_at": window_end - 0.5}),
            serde_json::json!({"id": "after-end", "parent_session_id": "parent", "started_at": window_end + 1.0}),
        ];

        let visible = select_visible_subagent_sessions("parent", &sessions, window_end);
        let ids = visible
            .iter()
            .filter_map(|item| item.get("id").and_then(Value::as_str))
            .collect::<Vec<_>>();

        assert_eq!(ids, vec!["at-end", "at-start"]);
    }

    #[test]
    fn visible_subagent_sessions_use_api_lineage_when_an_intermediate_parent_is_on_an_older_page() {
        let window_end = 500_000.0;
        let sessions = vec![serde_json::json!({
            "id": "nested-visible",
            "parent_session_id": "older-parent-not-in-page",
            "_lineage_root_id": "parent",
            "started_at": window_end - 10.0
        })];

        let visible = select_visible_subagent_sessions("parent", &sessions, window_end);

        assert_eq!(visible.len(), 1);
        assert_eq!(string_field(&visible[0], "id").as_deref(), Some("nested-visible"));
    }

    #[tokio::test]
    async fn subagent_missing_intermediate_ancestor_is_resolved_through_the_api() {
        async fn api_session(AxumPath(id): AxumPath<String>) -> Json<Value> {
            assert_eq!(id, "older-parent");
            Json(serde_json::json!({
                "session": {
                    "id": "older-parent",
                    "parent_session_id": "parent",
                    "started_at": 300_000.0,
                    "last_active": 300_100.0,
                    "ended_at": 300_200.0
                }
            }))
        }

        let app = Router::new().route("/api/sessions/{id}", get(api_session));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state(format!("http://{addr}"), temp.path());
        let mut sessions = vec![serde_json::json!({
            "id": "nested-visible",
            "parent_session_id": "older-parent",
            "started_at": 490_000.0,
            "last_active": 490_100.0
        })];

        resolve_missing_subagent_ancestors(&state, &mut sessions, "parent", 500_000.0).await.unwrap();
        let visible = select_visible_subagent_sessions("parent", &sessions, 500_000.0);

        assert_eq!(visible.len(), 1);
        assert_eq!(string_field(&visible[0], "id").as_deref(), Some("nested-visible"));
    }

    #[tokio::test]
    async fn subagent_api_json_rejects_a_response_over_the_byte_limit() {
        async fn oversized() -> String {
            "x".repeat(256)
        }

        let app = Router::new().route("/oversized", get(oversized));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state(format!("http://{addr}"), temp.path());

        let error = fetch_api_json(&state, format!("http://{addr}/oversized"), 32).await.unwrap_err();

        assert!(error.to_string().contains("byte limit"));
    }

    #[tokio::test]
    async fn subagent_api_pagination_continues_and_deduplicates_session_ids() {
        async fn api_sessions(
            State(requested_offsets): State<Arc<std::sync::Mutex<Vec<usize>>>>,
            Query(query): Query<HashMap<String, String>>,
        ) -> Json<Value> {
            let offset = query.get("offset").and_then(|value| value.parse::<usize>().ok()).unwrap_or(0);
            requested_offsets.lock().unwrap().push(offset);
            if offset == 0 {
                let data = (0..2).map(|index| serde_json::json!({
                    "id": format!("other-{index}"),
                    "parent_session_id": "another",
                    "started_at": 499_000.0 - index as f64,
                    "last_active": 499_500.0 - index as f64
                })).collect::<Vec<_>>();
                return Json(serde_json::json!({"sessions": data, "has_more": true}));
            }
            Json(serde_json::json!({
                "sessions": [
                    {"id": "other-0", "parent_session_id": "another", "started_at": 499_000.0, "last_active": 499_500.0},
                    {"id": "target", "parent_session_id": "parent", "started_at": 498_000.0, "last_active": 498_100.0}
                ],
                "has_more": false
            }))
        }

        let requested_offsets = Arc::new(std::sync::Mutex::new(Vec::new()));
        let app = Router::new().route("/api/sessions", get(api_sessions)).with_state(requested_offsets.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state(format!("http://{addr}"), temp.path());

        let sessions = fetch_subagent_sessions(&state, 500_000.0).await.unwrap();
        let ids = sessions.iter().filter_map(|session| string_field(session, "id")).collect::<HashSet<_>>();

        assert_eq!(sessions.len(), 3);
        assert!(ids.contains("target"));
        assert_eq!(ids.len(), sessions.len());
        assert_eq!(*requested_offsets.lock().unwrap(), vec![0, 2]);
    }

    #[tokio::test]
    async fn subagent_api_pagination_rejects_an_empty_nonfinal_page() {
        async fn api_sessions() -> Json<Value> {
            Json(serde_json::json!({"data": [], "has_more": true}))
        }

        let app = Router::new().route("/api/sessions", get(api_sessions));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state(format!("http://{addr}"), temp.path());

        let error = fetch_subagent_sessions(&state, 500_000.0).await.unwrap_err();

        assert!(error.to_string().contains("pagination made no progress"));
    }

    #[tokio::test]
    async fn subagent_api_pagination_fails_closed_at_the_memory_limit() {
        async fn api_sessions(Query(query): Query<HashMap<String, String>>) -> Json<Value> {
            let offset = query.get("offset").and_then(|value| value.parse::<usize>().ok()).unwrap_or(0);
            let data = (offset..offset + SUBAGENT_PAGE_SIZE).map(|index| serde_json::json!({
                "id": format!("child-{index}"),
                "parent_session_id": "another",
                "started_at": 499_000.0,
                "last_active": 499_500.0
            })).collect::<Vec<_>>();
            Json(serde_json::json!({"data": data, "has_more": true}))
        }

        let app = Router::new().route("/api/sessions", get(api_sessions));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state(format!("http://{addr}"), temp.path());

        let error = fetch_subagent_sessions(&state, 500_000.0).await.unwrap_err();

        assert!(error.to_string().contains("in-memory safety limit"));
    }

    #[test]
    fn omitted_parent_is_preserved_and_marked_explicitly() {
        let session = serde_json::json!({
            "id": "nested",
            "parent_session_id": "omitted-parent",
            "started_at": 100.0,
            "ended_at": 101.0
        });
        let messages = vec![serde_json::json!({"role": "user", "content": "Review"})];
        let projection = project_subagent_session(Path::new("/nonexistent"), &session, &messages).unwrap();

        let marked = mark_subagent_omitted_ancestry(projection, &HashSet::new(), "parent");

        assert_eq!(marked.parent_session_id, "omitted-parent");
        assert!(marked.ancestry_omitted);
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
                ancestry_omitted: false,
                task: "Review".to_string(),
                context: None,
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
            created_at: 100.0,
            last_turn_at: 150.0,
            turns_used: 1,
            max_turns: 20,
            subgoals: Vec::new(),
            todos: Vec::new(),
            milestones: Vec::new(),
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
        assert!(backend_source.contains("\"/chat/subagents/{session_id}/snapshot\""));
    }

    #[test]
    fn subagent_websocket_uses_api_unauthorized_response_path() {
        let auth_source = include_str!("../auth.rs");
        assert!(auth_source.contains("path.starts_with(\"/chat/subagents\")"));
    }
