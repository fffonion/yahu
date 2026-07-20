const SUBAGENT_POLL_INTERVAL: Duration = Duration::from_millis(1_000);
const SUBAGENT_IDLE_POLL_INTERVAL: Duration = Duration::from_secs(15);
const SUBAGENT_POLL_TIMEOUT: Duration = Duration::from_secs(10);
const SUBAGENT_PAGE_SIZE: usize = 200;
const SUBAGENT_SESSION_SCAN_LIMIT: usize = 10_000;
const SUBAGENT_API_PAGE_BYTE_LIMIT: usize = 2 * 1024 * 1024;
const SUBAGENT_API_DETAIL_BYTE_LIMIT: usize = 4 * 1024 * 1024;
const SUBAGENT_ANCESTOR_RESOLUTION_LIMIT: usize = 200;
const SUBAGENT_VISIBLE_LIMIT: usize = 10;
const SUBAGENT_LOOKBACK_SECONDS: f64 = 43_200.0;
const SUBAGENT_ACTIVITY_LIMIT: usize = 8;
const SUBAGENT_SUMMARY_LIMIT: usize = 600;
const SUBAGENT_SNAPSHOT_CONCURRENCY: usize = 4;

static SUBAGENT_SNAPSHOT_PERMITS: std::sync::LazyLock<tokio::sync::Semaphore> =
    std::sync::LazyLock::new(|| tokio::sync::Semaphore::new(SUBAGENT_SNAPSHOT_CONCURRENCY));

#[derive(Clone, Debug, PartialEq, Serialize)]
struct SubagentTodo {
    id: String,
    content: String,
    status: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct SubagentActivity {
    tool: String,
    timestamp: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct SubagentProjection {
    session_id: String,
    parent_session_id: String,
    ancestry_omitted: bool,
    task: String,
    model: Option<String>,
    status: String,
    started_at: Option<f64>,
    ended_at: Option<f64>,
    message_count: u64,
    tool_count: u64,
    api_calls: u64,
    current_tool: Option<String>,
    todos: Vec<SubagentTodo>,
    activity: Vec<SubagentActivity>,
    summary: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct GoalMilestoneProjection {
    turn: u64,
    timestamp: f64,
    verdict: String,
    reason: String,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
struct PersistentGoalProjection {
    text: String,
    status: String,
    created_at: f64,
    last_turn_at: f64,
    turns_used: u64,
    max_turns: u64,
    subgoals: Vec<String>,
    todos: Vec<SubagentTodo>,
    milestones: Vec<GoalMilestoneProjection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    last_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    paused_reason: Option<String>,
}

#[derive(Serialize)]
struct SubagentSnapshot<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    session_id: &'a str,
    generated_at: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    goal: Option<&'a PersistentGoalProjection>,
    subagents: &'a [SubagentProjection],
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'a str>,
}

#[derive(Clone)]
struct CachedSubagentProjection {
    message_count: u64,
    ended_at: Option<f64>,
    projection: SubagentProjection,
}

#[derive(Default)]
struct CachedParentTodos {
    goal_created_at: Option<f64>,
    message_count: Option<u64>,
    todos: Vec<SubagentTodo>,
}

#[derive(Default, Deserialize)]
struct SubagentWindowQuery {
    before: Option<f64>,
}

async fn subagent_websocket(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Response<Body> {
    if !subagent_websocket_origin_allowed(&headers) {
        return StatusCode::FORBIDDEN.into_response();
    }
    ws.on_upgrade(move |socket| stream_subagent_snapshots(socket, state, session_id))
        .into_response()
}

async fn subagent_snapshot(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
    Query(query): Query<SubagentWindowQuery>,
) -> Response<Body> {
    let Ok(_permit) = SUBAGENT_SNAPSHOT_PERMITS.try_acquire() else {
        return (StatusCode::TOO_MANY_REQUESTS, "Too many subagent snapshot requests").into_response();
    };
    let Some(before) = query.before.filter(|value| value.is_finite() && *value > 0.0) else {
        return (StatusCode::BAD_REQUEST, "A finite positive before timestamp is required").into_response();
    };
    let mut cache = HashMap::<String, CachedSubagentProjection>::new();
    let mut parent_todo_cache = CachedParentTodos::default();
    let mut error = None::<String>;
    let subagents = match timeout(
        SUBAGENT_POLL_TIMEOUT,
        fetch_subagent_projection_snapshot(&state, &session_id, before, &mut cache),
    )
    .await
    {
        Ok(Ok(items)) => items,
        Ok(Err(err)) => {
            error = Some(err.to_string());
            Vec::new()
        }
        Err(_) => {
            error = Some("subagent snapshot timed out".to_string());
            Vec::new()
        }
    };
    let mut goal = match load_persistent_goal(&state.hermes_home, &session_id) {
        Ok(goal) => goal,
        Err(err) => {
            error = append_subagent_error(error, format!("failed to load persistent goal: {err}"));
            None
        }
    };
    if let Some(goal) = goal.as_mut() {
        match timeout(
            SUBAGENT_POLL_TIMEOUT,
            fetch_parent_session_todos(
                &state,
                &session_id,
                goal.created_at,
                &mut parent_todo_cache,
            ),
        )
        .await
        {
            Ok(Ok(todos)) => goal.todos = todos,
            Ok(Err(err)) => {
                error = append_subagent_error(error, format!("failed to load main-session todos: {err}"));
            }
            Err(_) => {
                error = append_subagent_error(error, "main-session todo snapshot timed out".to_string());
            }
        }
    }
    Json(serde_json::json!({
        "type": "subagents.snapshot",
        "session_id": session_id,
        "generated_at": unix_now_seconds(),
        "goal": goal,
        "subagents": subagents,
        "error": error,
    }))
    .into_response()
}

fn append_subagent_error(current: Option<String>, message: String) -> Option<String> {
    Some(match current {
        Some(existing) => format!("{existing}; {message}"),
        None => message,
    })
}

async fn subagent_messages(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
) -> Response<Body> {
    match fetch_session_messages(&state, &session_id).await {
        Ok(messages) => Json(serde_json::json!({ "data": messages })).into_response(),
        Err(err) => json_error(
            StatusCode::BAD_GATEWAY,
            &format!("failed to load subagent messages: {err}"),
        ),
    }
}

fn subagent_websocket_origin_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|value| value.to_str().ok()) else {
        return true;
    };
    let Some(host) = headers.get(header::HOST).and_then(|value| value.to_str().ok()) else {
        return false;
    };
    let Ok(origin_uri) = origin.parse::<Uri>() else {
        return false;
    };
    let Some(origin_authority) = origin_uri.authority() else {
        return false;
    };
    let Ok(host_authority) = host.parse::<axum::http::uri::Authority>() else {
        return false;
    };
    let default_port = match origin_uri.scheme_str() {
        Some("http") => 80,
        Some("https") => 443,
        _ => return false,
    };
    origin_authority.host().eq_ignore_ascii_case(host_authority.host())
        && origin_authority.port_u16().unwrap_or(default_port)
            == host_authority.port_u16().unwrap_or(default_port)
}

fn subagent_feed_sender(
    feeds: &mut HashMap<String, watch::Sender<String>>,
    session_id: &str,
) -> (watch::Sender<String>, bool) {
    if let Some(sender) = feeds.get(session_id) {
        return (sender.clone(), false);
    }
    let (sender, _) = watch::channel(String::new());
    feeds.insert(session_id.to_string(), sender.clone());
    (sender, true)
}

fn subagent_poll_delay(
    subagents: &[SubagentProjection],
    goal: Option<&PersistentGoalProjection>,
) -> Duration {
    if subagents.is_empty()
        || subagents.iter().any(|item| item.status == "running")
        || goal.is_some_and(|item| item.status == "active")
    {
        SUBAGENT_POLL_INTERVAL
    } else {
        SUBAGENT_IDLE_POLL_INTERVAL
    }
}

async fn subscribe_subagent_snapshots(
    state: Arc<AppState>,
    session_id: &str,
) -> watch::Receiver<String> {
    let (sender, receiver, created) = {
        let mut feeds = state.subagent_feeds.write().await;
        let (sender, created) = subagent_feed_sender(&mut feeds, session_id);
        let receiver = sender.subscribe();
        (sender, receiver, created)
    };
    if created {
        let state = state.clone();
        let session_id = session_id.to_string();
        tokio::spawn(async move { run_subagent_feed(state, session_id, sender).await });
    }
    receiver
}

async fn stream_subagent_snapshots(socket: WebSocket, state: Arc<AppState>, session_id: String) {
    let (mut sender, mut receiver) = socket.split();
    let mut snapshots = subscribe_subagent_snapshots(state, &session_id).await;
    let initial = snapshots.borrow_and_update().clone();
    if !initial.is_empty() && sender.send(Message::Text(initial.into())).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            changed = snapshots.changed() => {
                if changed.is_err() {
                    break;
                }
                let text = snapshots.borrow_and_update().clone();
                if sender.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            incoming = receiver.next() => {
                match incoming {
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
}

async fn run_subagent_feed(
    state: Arc<AppState>,
    session_id: String,
    sender: watch::Sender<String>,
) {
    let mut ticker = interval(SUBAGENT_POLL_INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut cache = HashMap::<String, CachedSubagentProjection>::new();
    let mut parent_todo_cache = CachedParentTodos::default();
    let mut current_subagents = Vec::<SubagentProjection>::new();
    let mut current_goal = None::<PersistentGoalProjection>;
    let mut last_fingerprint = String::new();
    let mut next_poll = Instant::now();

    loop {
        ticker.tick().await;
        if sender.receiver_count() == 0 {
            let mut feeds = state.subagent_feeds.write().await;
            let current_is_same = feeds
                .get(&session_id)
                .is_some_and(|current| current.same_channel(&sender));
            if !current_is_same {
                return;
            }
            if sender.receiver_count() == 0 {
                feeds.remove(&session_id);
                return;
            }
        }
        if Instant::now() < next_poll {
            continue;
        }
        let Ok(_permit) = SUBAGENT_SNAPSHOT_PERMITS.try_acquire() else {
            next_poll = Instant::now() + SUBAGENT_POLL_INTERVAL;
            continue;
        };

        let window_end = unix_now_seconds();
        let mut error = match timeout(
            SUBAGENT_POLL_TIMEOUT,
            fetch_subagent_projection_snapshot(&state, &session_id, window_end, &mut cache),
        )
        .await
        {
            Ok(Ok(items)) => {
                current_subagents = items;
                None
            }
            Ok(Err(err)) => Some(err.to_string()),
            Err(_) => Some("subagent progress poll timed out".to_string()),
        };
        match load_persistent_goal(&state.hermes_home, &session_id) {
            Ok(goal) => current_goal = goal,
            Err(err) => {
                let message = format!("failed to load persistent goal: {err}");
                error = Some(match error {
                    Some(existing) => format!("{existing}; {message}"),
                    None => message,
                });
            }
        }
        if let Some(goal) = current_goal.as_mut() {
            match timeout(
                SUBAGENT_POLL_TIMEOUT,
                fetch_parent_session_todos(
                    &state,
                    &session_id,
                    goal.created_at,
                    &mut parent_todo_cache,
                ),
            )
            .await
            {
                Ok(Ok(todos)) => goal.todos = todos,
                Ok(Err(err)) => {
                    goal.todos = parent_todo_cache.todos.clone();
                    let message = format!("failed to load main-session todos: {err}");
                    error = Some(match error {
                        Some(existing) => format!("{existing}; {message}"),
                        None => message,
                    });
                }
                Err(_) => {
                    goal.todos = parent_todo_cache.todos.clone();
                    let message = "main-session todo poll timed out".to_string();
                    error = Some(match error {
                        Some(existing) => format!("{existing}; {message}"),
                        None => message,
                    });
                }
            }
        }
        let fingerprint = serde_json::to_string(&(
            current_subagents.as_slice(),
            current_goal.as_ref(),
            error.as_deref(),
        ))
        .unwrap_or_default();
        if fingerprint != last_fingerprint {
            last_fingerprint = fingerprint;
            let payload = SubagentSnapshot {
                kind: "subagents.snapshot",
                session_id: &session_id,
                generated_at: unix_now_seconds(),
                goal: current_goal.as_ref(),
                subagents: &current_subagents,
                error: error.as_deref(),
            };
            if let Ok(text) = serde_json::to_string(&payload) {
                sender.send_replace(text);
            }
        }
        next_poll = Instant::now()
            + subagent_poll_delay(&current_subagents, current_goal.as_ref());
    }
}

fn load_persistent_goal(
    hermes_home: &Path,
    session_id: &str,
) -> anyhow::Result<Option<PersistentGoalProjection>> {
    // API Server currently exposes session metadata/messages but no GoalManager state.
    // Keep this fallback exact, read-only, and scoped to the selected main session.
    let db_path = hermes_home.join("state.db");
    if !db_path.exists() {
        return Ok(None);
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    if !sqlite_table_has_columns(&conn, "state_meta", &["key", "value"])? {
        return Ok(None);
    }
    let raw = conn
        .query_row(
            "SELECT value FROM state_meta WHERE key = ?1",
            [format!("goal:{session_id}")],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    let Some(raw) = raw else {
        return Ok(None);
    };
    let value = serde_json::from_str::<Value>(&raw)?;
    let Some(text) = string_field(&value, "goal") else {
        return Ok(None);
    };
    let raw_status = string_field(&value, "status").unwrap_or_else(|| "active".to_string());
    if matches!(raw_status.as_str(), "done" | "cleared") {
        return Ok(None);
    }
    let status = match raw_status.as_str() {
        "paused" => raw_status,
        _ => "active".to_string(),
    };
    let created_at = number_field(&value, "created_at").unwrap_or_default();
    let subgoals = value
        .get("subgoals")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .take(20)
        .map(|item| truncate_chars(item, 500))
        .collect();
    let mut milestones = value
        .get("milestones")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let reason = string_field(item, "reason")?;
            let reason = reason.trim();
            if reason.is_empty() {
                return None;
            }
            Some(GoalMilestoneProjection {
                turn: u64_field(item, "turn"),
                timestamp: number_field(item, "timestamp").unwrap_or_default(),
                verdict: string_field(item, "verdict")
                    .unwrap_or_else(|| "continue".to_string()),
                reason: truncate_chars(reason, 1_000),
            })
        })
        .collect::<Vec<_>>();
    let milestone_cache_key = format!("yahu:goal_milestones:{session_id}");
    let milestone_cache_raw = conn
        .query_row(
            "SELECT value FROM state_meta WHERE key = ?1",
            [&milestone_cache_key],
            |row| row.get::<_, String>(0),
        )
        .optional()?;
    if let Some(cached) = milestone_cache_raw.as_deref()
        && let Ok(items) = serde_json::from_str::<Value>(cached)
        && let Some(items) = items.as_array()
    {
        milestones.extend(items.iter().filter_map(|item| {
            let reason = string_field(item, "reason")?;
            Some(GoalMilestoneProjection {
                turn: u64_field(item, "turn"),
                timestamp: number_field(item, "timestamp").unwrap_or_default(),
                verdict: string_field(item, "verdict")
                    .unwrap_or_else(|| "continue".to_string()),
                reason: truncate_chars(reason.trim(), 1_000),
            })
        }));
    }
    if let Some(reason) = string_field(&value, "last_reason") {
        let latest = GoalMilestoneProjection {
            turn: u64_field(&value, "turns_used"),
            timestamp: number_field(&value, "last_turn_at").unwrap_or_default(),
            verdict: string_field(&value, "last_verdict")
                .unwrap_or_else(|| "continue".to_string()),
            reason: truncate_chars(reason.trim(), 1_000),
        };
        if !milestones
            .iter()
            .any(|item| item.turn == latest.turn)
        {
            milestones.push(latest);
        }
    }
    if created_at > 0.0 {
        milestones.retain(|item| item.timestamp >= created_at);
    }
    milestones.sort_by(|left, right| {
        right
            .timestamp
            .total_cmp(&left.timestamp)
            .then_with(|| right.turn.cmp(&left.turn))
    });
    milestones.dedup_by(|left, right| left.turn == right.turn);
    let encoded_milestones = serde_json::to_string(&milestones)?;
    if milestone_cache_raw.as_deref() != Some(encoded_milestones.as_str()) {
        let write_conn = rusqlite::Connection::open(hermes_home.join("state.db"))?;
        write_conn.execute(
            "INSERT INTO state_meta (key, value) VALUES (?1, ?2) \
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            rusqlite::params![milestone_cache_key, encoded_milestones],
        )?;
    }
    Ok(Some(PersistentGoalProjection {
        text: truncate_chars(text.trim(), 2_000),
        status,
        created_at,
        last_turn_at: number_field(&value, "last_turn_at").unwrap_or_default(),
        turns_used: u64_field(&value, "turns_used"),
        max_turns: u64_field(&value, "max_turns"),
        subgoals,
        todos: Vec::new(),
        milestones,
        last_reason: string_field(&value, "last_reason")
            .map(|item| truncate_chars(item.trim(), 1_000)),
        paused_reason: string_field(&value, "paused_reason")
            .map(|item| truncate_chars(item.trim(), 500)),
    }))
}

async fn fetch_parent_session_todos(
    state: &AppState,
    session_id: &str,
    goal_created_at: f64,
    cache: &mut CachedParentTodos,
) -> anyhow::Result<Vec<SubagentTodo>> {
    let goal_created_at = if goal_created_at.is_finite() && goal_created_at > 0.0 {
        goal_created_at
    } else {
        0.0
    };
    if cache.goal_created_at != Some(goal_created_at) {
        cache.goal_created_at = Some(goal_created_at);
        cache.message_count = None;
        cache.todos.clear();
    }

    let url = format!(
        "{}/api/sessions/{}",
        state.api_url.trim_end_matches('/'),
        path_segment(session_id),
    );
    let body = fetch_api_json(state, url, SUBAGENT_API_PAGE_BYTE_LIMIT).await?;
    let session = body.get("session").unwrap_or(&body);
    let message_count = u64_field(session, "message_count");
    if cache.message_count == Some(message_count) {
        return Ok(cache.todos.clone());
    }

    let messages = fetch_session_messages(state, session_id).await?;
    let todos = if let Some(todos) = latest_todos_state_since(&messages, goal_created_at) {
        todos
    } else {
        match fetch_local_goal_todo_messages(state, session_id, goal_created_at) {
            Ok(Some(messages)) => latest_todos_state_since(&messages, goal_created_at).unwrap_or_default(),
            Ok(None) => Vec::new(),
            Err(err) => {
                warn!(session_id = %session_id, error = %err, "cannot restore Goal todos from local todo history");
                Vec::new()
            }
        }
    };
    cache.message_count = Some(message_count);
    cache.todos = todos.clone();
    Ok(todos)
}

fn fetch_local_goal_todo_messages(
    state: &AppState,
    session_id: &str,
    goal_created_at: f64,
) -> anyhow::Result<Option<Vec<Value>>> {
    let db_path = state.hermes_home.join("state.db");
    if !db_path.exists() {
        return Ok(None);
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    if !sqlite_table_has_columns(
        &conn,
        "messages",
        &[
            "id",
            "session_id",
            "role",
            "content",
            "tool_call_id",
            "tool_calls",
            "tool_name",
            "timestamp",
            "token_count",
            "finish_reason",
            "reasoning",
            "reasoning_content",
            "active",
            "compacted",
        ],
    )? {
        return Ok(None);
    }
    let reasoning_columns = local_reasoning_select_columns(&conn)?;
    let sql = format!(
        "SELECT id, session_id, role, content, tool_call_id, tool_calls, tool_name, timestamp, \
                token_count, finish_reason, {reasoning_columns} \
         FROM messages \
         WHERE (active = 1 OR compacted = 1) \
           AND session_id = ?1 \
           AND timestamp >= ?2 \
           AND ( \
               (role = 'user' AND content LIKE '[Your active task list was preserved across context compression]%') \
               OR (role = 'assistant' AND tool_calls LIKE '%todo%') \
               OR (role = 'tool' AND tool_name = 'todo') \
           ) \
         ORDER BY timestamp, id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params![session_id, goal_created_at],
        row_to_session_message,
    )?;
    Ok(Some(rows.collect::<rusqlite::Result<Vec<_>>>()?))
}

async fn fetch_subagent_projection_snapshot(
    state: &AppState,
    parent_session_id: &str,
    window_end: f64,
    cache: &mut HashMap<String, CachedSubagentProjection>,
) -> anyhow::Result<Vec<SubagentProjection>> {
    let mut sessions = fetch_subagent_sessions(state, window_end).await?;
    resolve_missing_subagent_ancestors(state, &mut sessions, parent_session_id, window_end).await?;
    let visible = select_visible_subagent_sessions(parent_session_id, &sessions, window_end);
    let visible_ids = visible
        .iter()
        .filter_map(|session| string_field(session, "id"))
        .collect::<HashSet<_>>();
    cache.retain(|session_id, _| visible_ids.contains(session_id));

    let mut out = Vec::with_capacity(visible.len());
    for session in visible {
        let Some(session_id) = string_field(&session, "id") else {
            continue;
        };
        let message_count = u64_field(&session, "message_count");
        let ended_at = number_field(&session, "ended_at");
        if let Some(cached) = cache.get(&session_id)
            && cached.message_count == message_count
            && cached.ended_at == ended_at
        {
            out.push(mark_subagent_omitted_ancestry(
                cached.projection.clone(),
                &visible_ids,
                parent_session_id,
            ));
            continue;
        }

        let messages = fetch_session_messages(state, &session_id).await?;
        let Some(projection) = project_subagent_session(&session, &messages) else {
            continue;
        };
        cache.insert(
            session_id,
            CachedSubagentProjection {
                message_count,
                ended_at,
                projection: projection.clone(),
            },
        );
        out.push(mark_subagent_omitted_ancestry(
            projection,
            &visible_ids,
            parent_session_id,
        ));
    }
    Ok(out)
}

fn mark_subagent_omitted_ancestry(
    mut projection: SubagentProjection,
    visible_ids: &HashSet<String>,
    parent_session_id: &str,
) -> SubagentProjection {
    projection.ancestry_omitted = projection.parent_session_id != parent_session_id
        && !visible_ids.contains(&projection.parent_session_id);
    projection
}

async fn fetch_subagent_sessions(state: &AppState, window_end: f64) -> anyhow::Result<Vec<Value>> {
    let mut sessions = HashMap::<String, Value>::new();
    let window_start = window_end - SUBAGENT_LOOKBACK_SECONDS;
    let mut offset = 0usize;
    loop {
        let url = format!(
            "{}/api/sessions?source=subagent&include_children=true&limit={}&offset={}",
            state.api_url.trim_end_matches('/'),
            SUBAGENT_PAGE_SIZE,
            offset,
        );
        let body = fetch_api_json(state, url, SUBAGENT_API_PAGE_BYTE_LIMIT).await?;
        let data = body
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let has_more = body.get("has_more").and_then(Value::as_bool).unwrap_or(false);
        let count = data.len();
        let reached_window_start = data
            .last()
            .and_then(session_activity_time)
            .is_some_and(|time| time < window_start);
        let mut new_ids = 0usize;
        for session in data {
            let Some(id) = string_field(&session, "id") else {
                continue;
            };
            if let std::collections::hash_map::Entry::Vacant(entry) = sessions.entry(id) {
                entry.insert(session);
                new_ids += 1;
            }
        }
        if !has_more || reached_window_start {
            break;
        }
        if count == 0 || new_ids == 0 {
            anyhow::bail!("subagent session pagination made no progress");
        }
        let next_offset = offset.saturating_add(count);
        if sessions.len() >= SUBAGENT_SESSION_SCAN_LIMIT
            || next_offset >= SUBAGENT_SESSION_SCAN_LIMIT
        {
            anyhow::bail!("subagent session scan exceeded the in-memory safety limit");
        }
        offset = next_offset;
    }
    Ok(sessions.into_values().collect())
}

async fn resolve_missing_subagent_ancestors(
    state: &AppState,
    sessions: &mut Vec<Value>,
    parent_session_id: &str,
    window_end: f64,
) -> anyhow::Result<()> {
    let window_start = window_end - SUBAGENT_LOOKBACK_SECONDS;
    let mut by_id = sessions
        .iter()
        .filter_map(|session| Some((string_field(session, "id")?, session.clone())))
        .collect::<HashMap<_, _>>();
    let mut candidate_ids = sessions
        .iter()
        .filter(|session| {
            number_field(session, "started_at")
                .is_some_and(|started| started >= window_start && started <= window_end)
        })
        .filter_map(|session| string_field(session, "id"))
        .collect::<Vec<_>>();
    candidate_ids.sort_by(|left, right| {
        let left_started = by_id.get(left).and_then(|session| number_field(session, "started_at")).unwrap_or(f64::NEG_INFINITY);
        let right_started = by_id.get(right).and_then(|session| number_field(session, "started_at")).unwrap_or(f64::NEG_INFINITY);
        right_started.total_cmp(&left_started)
    });

    let mut matched = 0usize;
    let mut resolved = 0usize;
    for candidate_id in candidate_ids {
        loop {
            match subagent_membership_or_missing(&candidate_id, &by_id, parent_session_id) {
                Ok(true) => {
                    matched += 1;
                    break;
                }
                Ok(false) => break,
                Err(missing_id) => {
                    if resolved >= SUBAGENT_ANCESTOR_RESOLUTION_LIMIT {
                        anyhow::bail!("subagent ancestor resolution exceeded the safety limit");
                    }
                    let url = format!(
                        "{}/api/sessions/{}",
                        state.api_url.trim_end_matches('/'),
                        path_segment(&missing_id),
                    );
                    let body = fetch_api_json(state, url, SUBAGENT_API_PAGE_BYTE_LIMIT).await?;
                    let ancestor = body.get("session").unwrap_or(&body).clone();
                    let Some(ancestor_id) = string_field(&ancestor, "id") else {
                        anyhow::bail!("subagent ancestor response omitted its id");
                    };
                    by_id.insert(ancestor_id, ancestor.clone());
                    sessions.push(ancestor);
                    resolved += 1;
                }
            }
        }
        if matched >= SUBAGENT_VISIBLE_LIMIT {
            break;
        }
    }
    Ok(())
}

fn subagent_membership_or_missing(
    session_id: &str,
    sessions_by_id: &HashMap<String, Value>,
    parent_session_id: &str,
) -> Result<bool, String> {
    let mut current = session_id.to_string();
    let mut seen = HashSet::new();
    while seen.insert(current.clone()) {
        let Some(session) = sessions_by_id.get(&current) else {
            return Err(current);
        };
        if let Some(lineage_root) = string_field(session, "_lineage_root_id") {
            return Ok(lineage_root == parent_session_id);
        }
        let Some(parent) = string_field(session, "parent_session_id") else {
            return Ok(false);
        };
        if parent == parent_session_id {
            return Ok(true);
        }
        current = parent;
    }
    Ok(false)
}

fn session_activity_time(session: &Value) -> Option<f64> {
    number_field(session, "last_active")
        .or_else(|| number_field(session, "ended_at"))
        .or_else(|| number_field(session, "started_at"))
}

async fn fetch_session_messages(state: &AppState, session_id: &str) -> anyhow::Result<Vec<Value>> {
    let url = format!(
        "{}/api/sessions/{}/messages",
        state.api_url.trim_end_matches('/'),
        path_segment(session_id),
    );
    let body = fetch_api_json(state, url, SUBAGENT_API_DETAIL_BYTE_LIMIT).await?;
    Ok(body
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

async fn fetch_api_json(state: &AppState, url: String, max_bytes: usize) -> anyhow::Result<Value> {
    let mut request = state.client.get(url);
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        request = request.bearer_auth(key);
    }
    let mut response = request.send().await?.error_for_status()?;
    if response.content_length().is_some_and(|length| length > max_bytes as u64) {
        anyhow::bail!("subagent API response exceeds the byte limit");
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        if body.len().saturating_add(chunk.len()) > max_bytes {
            anyhow::bail!("subagent API response exceeds the byte limit");
        }
        body.extend_from_slice(&chunk);
    }
    Ok(serde_json::from_slice::<Value>(&body)?)
}

fn select_visible_subagent_sessions(
    parent_session_id: &str,
    sessions: &[Value],
    window_end: f64,
) -> Vec<Value> {
    let window_start = window_end - SUBAGENT_LOOKBACK_SECONDS;
    let parent_by_id = sessions
        .iter()
        .filter_map(|session| {
            Some((
                string_field(session, "id")?,
                string_field(session, "parent_session_id")?,
            ))
        })
        .collect::<HashMap<_, _>>();

    let mut visible = sessions
        .iter()
        .filter(|session| {
            let Some(started_at) = number_field(session, "started_at") else {
                return false;
            };
            if started_at < window_start || started_at > window_end {
                return false;
            }
            if string_field(session, "_lineage_root_id").as_deref() == Some(parent_session_id) {
                return true;
            }
            let Some(mut current) = string_field(session, "id") else {
                return false;
            };
            let mut seen = HashSet::new();
            while seen.insert(current.clone()) {
                let Some(parent) = parent_by_id.get(&current) else {
                    return false;
                };
                if parent == parent_session_id {
                    return true;
                }
                current = parent.clone();
            }
            false
        })
        .cloned()
        .collect::<Vec<_>>();
    visible.sort_by(|left, right| {
        number_field(right, "started_at")
            .unwrap_or(f64::NEG_INFINITY)
            .total_cmp(&number_field(left, "started_at").unwrap_or(f64::NEG_INFINITY))
    });
    visible.truncate(SUBAGENT_VISIBLE_LIMIT);
    visible
}

fn project_subagent_session(session: &Value, messages: &[Value]) -> Option<SubagentProjection> {
    let session_id = string_field(session, "id")?;
    let parent_session_id = string_field(session, "parent_session_id").unwrap_or_default();
    let task = messages
        .iter()
        .find(|message| message.get("role").and_then(Value::as_str) == Some("user"))
        .and_then(|message| message.get("content"))
        .map(content_text)
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| "Subagent".to_string());

    let mut pending_tools = Vec::<(String, String)>::new();
    let mut completed_tool_ids = HashSet::<String>::new();
    let mut activity = Vec::<SubagentActivity>::new();
    let todos = latest_todos(messages);
    let mut summary = None;

    for message in messages {
        match message.get("role").and_then(Value::as_str) {
            Some("assistant") => {
                let text = message.get("content").map(content_text).unwrap_or_default();
                if !text.trim().is_empty() {
                    summary = Some(truncate_chars(text.trim(), SUBAGENT_SUMMARY_LIMIT));
                }
                for call in tool_calls(message.get("tool_calls")) {
                    let Some(call_id) = string_field(&call, "id").or_else(|| string_field(&call, "call_id")) else {
                        continue;
                    };
                    let name = call
                        .get("function")
                        .and_then(Value::as_object)
                        .and_then(|function| function.get("name"))
                        .and_then(Value::as_str)
                        .unwrap_or("tool")
                        .to_string();
                    pending_tools.push((call_id, name));
                }
            }
            Some("tool") => {
                if let Some(call_id) = string_field(message, "tool_call_id") {
                    completed_tool_ids.insert(call_id);
                }
                let tool = string_field(message, "tool_name").unwrap_or_else(|| "tool".to_string());
                activity.push(SubagentActivity {
                    tool: tool.clone(),
                    timestamp: number_field(message, "timestamp"),
                });
            }
            _ => {}
        }
    }

    if activity.len() > SUBAGENT_ACTIVITY_LIMIT {
        activity = activity.split_off(activity.len() - SUBAGENT_ACTIVITY_LIMIT);
    }
    let current_tool = pending_tools
        .iter()
        .rev()
        .find(|(call_id, _)| !completed_tool_ids.contains(call_id))
        .map(|(_, name)| name.clone());
    let ended_at = number_field(session, "ended_at");
    let status = subagent_status(ended_at, session.get("end_reason").and_then(Value::as_str));

    Some(SubagentProjection {
        session_id,
        parent_session_id,
        ancestry_omitted: false,
        task: truncate_chars(task.trim(), 500),
        model: string_field(session, "model"),
        status,
        started_at: number_field(session, "started_at"),
        ended_at,
        message_count: u64_field(session, "message_count"),
        tool_count: u64_field(session, "tool_call_count"),
        api_calls: u64_field(session, "api_call_count"),
        current_tool,
        todos,
        activity,
        summary,
    })
}

fn latest_todos_state(messages: &[Value]) -> Option<Vec<SubagentTodo>> {
    let mut current = Vec::<SubagentTodo>::new();
    let mut observed = false;
    for message in messages {
        if message.get("role").and_then(Value::as_str) == Some("user")
            && let Some(content) = message.get("content").map(content_text)
            && let Some(preserved) = parse_preserved_todos(&content)
        {
            observed = true;
            current = preserved;
        }
        if message.get("role").and_then(Value::as_str) == Some("assistant") {
            for call in tool_calls(message.get("tool_calls")) {
                let function = call.get("function").and_then(Value::as_object);
                if function
                    .and_then(|value| value.get("name"))
                    .and_then(Value::as_str)
                    != Some("todo")
                {
                    continue;
                }
                let Some(arguments) = function.and_then(|value| value.get("arguments")) else {
                    continue;
                };
                let parsed = match arguments {
                    Value::String(text) => serde_json::from_str::<Value>(text).ok(),
                    Value::Object(_) => Some(arguments.clone()),
                    _ => None,
                };
                let Some(parsed) = parsed else {
                    continue;
                };
                let Some(items) = parsed.get("todos").and_then(Value::as_array) else {
                    continue;
                };
                observed = true;
                if parsed.get("merge").and_then(Value::as_bool).unwrap_or(false) {
                    merge_todos(&mut current, items);
                } else {
                    current = normalize_todos(items);
                }
            }
        }
        if message.get("role").and_then(Value::as_str) == Some("tool")
            && string_field(message, "tool_name").as_deref() == Some("todo")
        {
            let content = message.get("content").map(content_text).unwrap_or_default();
            if let Ok(value) = serde_json::from_str::<Value>(&content)
                && let Some(items) = value.get("todos").and_then(Value::as_array)
            {
                observed = true;
                current = normalize_todos(items);
            }
        }
    }
    observed.then_some(current)
}

fn latest_todos(messages: &[Value]) -> Vec<SubagentTodo> {
    latest_todos_state(messages).unwrap_or_default()
}

fn latest_todos_state_since(messages: &[Value], goal_created_at: f64) -> Option<Vec<SubagentTodo>> {
    if goal_created_at <= 0.0 {
        return latest_todos_state(messages);
    }
    let current_generation = messages
        .iter()
        .filter(|message| {
            number_field(message, "timestamp")
                .is_some_and(|timestamp| timestamp >= goal_created_at)
        })
        .cloned()
        .collect::<Vec<_>>();
    latest_todos_state(&current_generation)
}

fn parse_preserved_todos(content: &str) -> Option<Vec<SubagentTodo>> {
    const HEADER: &str = "[Your active task list was preserved across context compression]";
    if !content.trim_start().starts_with(HEADER) {
        return None;
    }
    let mut todos = Vec::<SubagentTodo>::new();
    for line in content.lines().skip(1) {
        let Some((_, rest)) = line.trim().strip_prefix("- [").and_then(|line| line.split_once("] ")) else {
            continue;
        };
        let Some((id, remainder)) = rest.split_once(". ") else {
            continue;
        };
        let Some((todo_content, status)) = remainder.rsplit_once(" (") else {
            continue;
        };
        let Some(status) = status.strip_suffix(')') else {
            continue;
        };
        let Some(status) = valid_todo_status(Some(status)) else {
            continue;
        };
        if id.trim().is_empty() || todo_content.trim().is_empty() {
            continue;
        }
        todos.push(SubagentTodo {
            id: id.trim().to_string(),
            content: truncate_chars(todo_content.trim(), 240),
            status,
        });
        if todos.len() == 100 {
            break;
        }
    }
    Some(todos)
}

fn normalize_todos(items: &[Value]) -> Vec<SubagentTodo> {
    let mut normalized = Vec::<SubagentTodo>::new();
    for item in items {
        let id = string_field(item, "id").unwrap_or_else(|| "?".to_string());
        let id = if id.trim().is_empty() { "?" } else { id.trim() };
        let content = string_field(item, "content").unwrap_or_else(|| "(no description)".to_string());
        let content = if content.trim().is_empty() {
            "(no description)"
        } else {
            content.trim()
        };
        if let Some(position) = normalized.iter().position(|todo| todo.id == id) {
            normalized.remove(position);
        }
        normalized.push(SubagentTodo {
            id: id.to_string(),
            content: truncate_chars(content, 240),
            status: normalize_todo_status(item.get("status").and_then(Value::as_str)),
        });
    }
    normalized.truncate(100);
    normalized
}

fn merge_todos(current: &mut Vec<SubagentTodo>, items: &[Value]) {
    current.truncate(100);

    // TodoStore collapses an incoming batch by id before applying it. Preserve
    // the last occurrence (and its position) so an earlier duplicate cannot
    // leak fields into the update that Hermes itself discarded.
    let mut last_index_by_id = std::collections::HashMap::<String, usize>::new();
    for (index, item) in items.iter().enumerate() {
        if let Some(id) = string_field(item, "id") {
            last_index_by_id.insert(id, index);
        }
    }

    for (index, item) in items.iter().enumerate() {
        let Some(id) = string_field(item, "id") else {
            continue;
        };
        let id = id.trim();
        if id.is_empty() {
            continue;
        }
        if last_index_by_id.get(id) != Some(&index) {
            continue;
        }
        if let Some(existing) = current.iter_mut().find(|todo| todo.id == id) {
            if let Some(content) = string_field(item, "content")
                && !content.trim().is_empty()
            {
                existing.content = truncate_chars(content.trim(), 240);
            }
            if let Some(status) = valid_todo_status(item.get("status").and_then(Value::as_str)) {
                existing.status = status;
            }
            continue;
        }
        if current.len() >= 100 {
            continue;
        }
        let content = string_field(item, "content").unwrap_or_else(|| "(no description)".to_string());
        let content = if content.trim().is_empty() {
            "(no description)"
        } else {
            content.trim()
        };
        current.push(SubagentTodo {
            id: id.to_string(),
            content: truncate_chars(content, 240),
            status: normalize_todo_status(item.get("status").and_then(Value::as_str)),
        });
    }
}

fn subagent_status(ended_at: Option<f64>, end_reason: Option<&str>) -> String {
    if ended_at.is_none() {
        return "running".to_string();
    }
    let reason = end_reason.unwrap_or_default().trim().to_ascii_lowercase();
    if reason.contains("interrupt") || reason.contains("cancel") {
        "interrupted".to_string()
    } else if reason.contains("timeout") {
        "timeout".to_string()
    } else if reason.contains("error") || reason.contains("fail") {
        "failed".to_string()
    } else {
        "completed".to_string()
    }
}

fn normalize_todo_status(status: Option<&str>) -> String {
    valid_todo_status(status).unwrap_or_else(|| "pending".to_string())
}

fn valid_todo_status(status: Option<&str>) -> Option<String> {
    match status?.trim().to_ascii_lowercase().as_str() {
        "pending" => Some("pending".to_string()),
        "in_progress" => Some("in_progress".to_string()),
        "completed" => Some("completed".to_string()),
        "cancelled" => Some("cancelled".to_string()),
        _ => None,
    }
}

fn tool_calls(value: Option<&Value>) -> Vec<Value> {
    match value {
        Some(Value::Array(items)) => items.clone(),
        Some(Value::String(text)) => serde_json::from_str::<Vec<Value>>(text).unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn content_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(|item| {
                item.as_str()
                    .map(str::to_string)
                    .or_else(|| item.get("text").and_then(Value::as_str).map(str::to_string))
                    .or_else(|| item.get("content").and_then(Value::as_str).map(str::to_string))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Value::Object(_) => serde_json::to_string(value).unwrap_or_default(),
        _ => String::new(),
    }
}

fn string_field(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_string)
}

fn number_field(value: &Value, key: &str) -> Option<f64> {
    value.get(key).and_then(|item| {
        item.as_f64()
            .or_else(|| item.as_str().and_then(|text| text.parse::<f64>().ok()))
    })
}

fn u64_field(value: &Value, key: &str) -> u64 {
    value.get(key).and_then(|item| {
        item.as_u64()
            .or_else(|| item.as_i64().and_then(|number| u64::try_from(number).ok()))
            .or_else(|| item.as_str().and_then(|text| text.parse::<u64>().ok()))
    }).unwrap_or(0)
}

fn truncate_chars(text: &str, limit: usize) -> String {
    let mut chars = text.chars();
    let prefix = chars.by_ref().take(limit).collect::<String>();
    if chars.next().is_some() {
        format!("{prefix}…")
    } else {
        prefix
    }
}
