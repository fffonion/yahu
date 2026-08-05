    #[test]
    fn chat_stream_run_body_preserves_selected_model_fields() {
        let body = serde_json::json!({"input":"hello","model":"gpt-5.5","provider":"openai-codex"});
        let forwarded = chat_stream_actual_body(&body).unwrap();

        assert_eq!(forwarded["input"], "hello");
        assert_eq!(forwarded["model"], "gpt-5.5");
        assert_eq!(forwarded["provider"], "openai-codex");
    }

    #[tokio::test]
    async fn yahu_chat_stream_forwards_selected_model_to_run_without_slash_command() {
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

        async fn api_run(
            State(state): State<OrderApiState>,
            headers: HeaderMap,
            body: Body,
        ) -> Json<serde_json::Value> {
            let bytes = to_bytes(body, usize::MAX).await.unwrap();
            let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            state.calls.lock().unwrap().push(serde_json::json!({
                "kind": "stream",
                "session_id": payload.get("session_id").cloned().unwrap_or_default(),
                "content_type_count": headers.get_all(header::CONTENT_TYPE).iter().count(),
                "body": payload,
            }));
            Json(serde_json::json!({"run_id":"run-test","status":"started"}))
        }

        async fn api_run_events() -> Response<Body> {
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, "text/event-stream")
                .body(Body::from("data: {\"event\":\"run.completed\",\"run_id\":\"run-test\",\"output\":\"done\"}\n\n"))
                .unwrap()
        }

        let calls = Arc::new(Mutex::new(Vec::new()));
        let api_state = OrderApiState { calls: calls.clone() };
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let api_app = Router::new()
            .route("/api/sessions/{session_id}/chat", post(api_chat))
            .route("/v1/runs", post(api_run))
            .route("/v1/runs/{run_id}/events", get(api_run_events))
            .with_state(api_state);
        tokio::spawn(async move { axum::serve(listener, api_app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));
        let state_for_assert = state.clone();

        let body = serde_json::json!({
            "input": "hello",
            "model": "gpt-5.5",
            "provider": "openai-codex",
            "reasoning_effort": "medium"
        });
        let mut request_headers = HeaderMap::new();
        request_headers.insert(header::CONTENT_TYPE, "application/json".parse().unwrap());
        let resp = chat_stream(
            State(state),
            AxumPath("session-1".to_string()),
            request_headers,
            Body::from(body.to_string()),
        ).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let _ = axum::body::to_bytes(resp.into_body(), usize::MAX).await.unwrap();

        // The final assistant snapshot must carry duration_ms >= 1 (not missing, not 0)
        let snapshot = state_for_assert
            .active_chat_streams
            .read()
            .await
            .get("session-1")
            .map(|snapshot| snapshot.messages.clone())
            .unwrap_or_default();
        let final_assistant = snapshot.iter().find(|m| {
            m.get("role").and_then(|r| r.as_str()) == Some("assistant")
                && !m.get("pending").and_then(|p| p.as_bool()).unwrap_or(true)
        });
        assert!(final_assistant.is_some(), "no completed assistant in snapshot");
        let duration_ms = final_assistant.unwrap().get("duration_ms").and_then(|v| v.as_f64());
        assert!(duration_ms.is_some(), "final assistant must have duration_ms, snapshot: {snapshot:?}");
        assert!(duration_ms.unwrap() > 0.0, "duration_ms must be > 0, got {:?}", duration_ms);

        let calls = calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 1, "calls: {calls:?}");
        assert_eq!(calls[0]["kind"], "stream");
        assert_eq!(calls[0]["content_type_count"], 1);
        assert_eq!(calls[0]["body"]["input"], "hello");
        assert_eq!(calls[0]["body"]["model"], "gpt-5.5");
        assert_eq!(calls[0]["body"]["provider"], "openai-codex");
        assert_eq!(calls[0]["body"]["reasoning_effort"], "medium");
    }

    #[tokio::test]
    async fn yahu_chat_stream_stop_calls_active_run_stop() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        #[derive(Clone)]
        struct StopApiState {
            calls: Arc<AtomicUsize>,
        }

        async fn api_stop_run(
            State(state): State<StopApiState>,
            AxumPath(run_id): AxumPath<String>,
        ) -> Json<serde_json::Value> {
            assert_eq!(run_id, "run-123");
            state.calls.fetch_add(1, Ordering::SeqCst);
            Json(serde_json::json!({"run_id":"run-123","status":"stopping"}))
        }

        let calls = Arc::new(AtomicUsize::new(0));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let api_app = Router::new()
            .route("/v1/runs/{run_id}/stop", post(api_stop_run))
            .with_state(StopApiState { calls: calls.clone() });
        tokio::spawn(async move { axum::serve(listener, api_app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));
        state.active_chat_run_ids.write().await.insert("session-1".to_string(), "run-123".to_string());

        let resp = stop_chat_stream(State(state), AxumPath("session-1".to_string())).await;

        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn yahu_chat_stream_stop_marks_an_admitted_start_for_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        let state = Arc::new(test_app_state("http://127.0.0.1:1".to_string(), temp.path()));
        state
            .active_chat_run_ids
            .write()
            .await
            .insert("session-1".to_string(), STARTING_CHAT_RUN_ID.to_string());

        let resp = stop_chat_stream(State(state.clone()), AxumPath("session-1".to_string())).await;
        let body = json_body(resp).await;

        assert_eq!(body["status"], "stop_pending");
        assert_eq!(
            state.active_chat_run_ids.read().await.get("session-1"),
            Some(&STOP_REQUESTED_CHAT_RUN_ID.to_string())
        );
    }

    #[tokio::test]
    async fn chat_stream_status_reports_active_run_for_attached_sessions() {
        let temp = tempfile::tempdir().unwrap();
        let state = Arc::new(test_app_state("http://127.0.0.1:1".to_string(), temp.path()));

        let idle = chat_stream_status(State(state.clone()), AxumPath("idle".to_string())).await;
        assert_eq!(idle.status(), StatusCode::OK);
        let idle_body: serde_json::Value = json_body(idle).await;
        assert_eq!(idle_body["running"], serde_json::json!(false));
        assert!(idle_body.get("run_id").is_none());

        state.active_chat_run_ids.write().await.insert("busy".to_string(), "run-xyz".to_string());
        let active = chat_stream_status(State(state.clone()), AxumPath("busy".to_string())).await;
        assert_eq!(active.status(), StatusCode::OK);
        let active_body: serde_json::Value = json_body(active).await;
        assert_eq!(active_body["running"], serde_json::json!(true));
        assert_eq!(active_body["run_id"], serde_json::json!("run-xyz"));
    }

    #[test]
    fn external_stream_detection_requires_a_recent_unfinished_message_tail() {
        let now = 10_000.0;
        assert!(session_message_tail_appears_running(
            &[serde_json::json!({"role":"user","timestamp":now - 2.0})],
            now,
        ));
        assert!(session_message_tail_appears_running(
            &[serde_json::json!({"role":"tool","timestamp":now - 2.0})],
            now,
        ));
        assert!(session_message_tail_appears_running(
            &[serde_json::json!({"role":"assistant","finish_reason":"tool_calls","timestamp":now - 2.0})],
            now,
        ));
        assert!(!session_message_tail_appears_running(
            &[serde_json::json!({"role":"assistant","content":"done","timestamp":now - 2.0})],
            now,
        ));
        assert!(!session_message_tail_appears_running(
            &[serde_json::json!({"role":"tool","timestamp":now - EXTERNAL_STREAM_ACTIVITY_TTL_SECONDS - 1.0})],
            now,
        ));
    }

    #[tokio::test]
    async fn chat_stream_status_detects_a_gateway_run_started_on_another_platform() {
        async fn api_health() -> Json<serde_json::Value> {
            Json(serde_json::json!({"active_agents":1}))
        }
        async fn api_messages() -> Json<serde_json::Value> {
            Json(serde_json::json!({
                "object":"list",
                "data":[{"id":1,"role":"tool","content":"working","timestamp":unix_now_seconds()}]
            }))
        }

        let app = Router::new()
            .route("/health/detailed", get(api_health))
            .route("/api/sessions/{session_id}/messages", get(api_messages));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let temp = tempfile::tempdir().unwrap();
        let state = Arc::new(test_app_state(format!("http://{addr}"), temp.path()));

        let response = chat_stream_status(State(state), AxumPath("telegram-session".to_string())).await;
        let body: serde_json::Value = json_body(response).await;

        assert_eq!(body["running"], serde_json::json!(true));
        assert_eq!(body["external"], serde_json::json!(true));
    }

    async fn json_body(resp: Response<Body>) -> serde_json::Value {
        let bytes = to_bytes(resp.into_body(), 64 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    }

    #[test]
    fn idle_chat_stream_snapshots_are_evicted_and_capacity_is_released() {
        let now = Instant::now();
        let mut snapshots = HashMap::with_capacity(64);
        snapshots.insert(
            "stale".to_string(),
            ActiveChatStreamSnapshot {
                updated_at: now - CHAT_STREAM_SNAPSHOT_IDLE_TTL - Duration::from_secs(1),
                messages: vec![serde_json::json!({"content": "x".repeat(1024)})],
            },
        );
        snapshots.insert(
            "recent".to_string(),
            ActiveChatStreamSnapshot {
                updated_at: now - CHAT_STREAM_SNAPSHOT_IDLE_TTL + Duration::from_secs(1),
                messages: vec![serde_json::json!({"content": "current"})],
            },
        );
        let capacity_before = snapshots.capacity();

        let removed = evict_idle_chat_stream_snapshots(
            &mut snapshots,
            now,
            CHAT_STREAM_SNAPSHOT_IDLE_TTL,
        );

        assert_eq!(removed, 1);
        assert!(!snapshots.contains_key("stale"));
        assert!(snapshots.contains_key("recent"));
        assert!(snapshots.capacity() < capacity_before);
    }

    #[test]
    fn chat_stream_broadcast_ring_is_small_because_payloads_are_full_snapshots() {
        const { assert!(CHAT_STREAM_BROADCAST_CAPACITY <= 32) };
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

        let rows = fetch_sessions_from_api_server(&state, "cache", 10, false)
            .await
            .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["id"], "s1");
        assert_eq!(rows[1]["id"], "s2");
        assert_eq!(message_requests.load(Ordering::SeqCst), 0);
    }
