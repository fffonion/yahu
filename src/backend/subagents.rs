const SUBAGENT_POLL_INTERVAL: Duration = Duration::from_millis(1_000);
const SUBAGENT_IDLE_POLL_INTERVAL: Duration = Duration::from_secs(15);
const SUBAGENT_EMPTY_FAST_POLLS: u32 = 45;
const SUBAGENT_POLL_TIMEOUT: Duration = Duration::from_secs(10);
const SUBAGENT_PAGE_SIZE: usize = 200;
const SUBAGENT_SESSION_SCAN_LIMIT: usize = 10_000;
const SUBAGENT_API_PAGE_BYTE_LIMIT: usize = 2 * 1024 * 1024;
const SUBAGENT_API_DETAIL_BYTE_LIMIT: usize = 4 * 1024 * 1024;
const SUBAGENT_PARENT_MESSAGE_PAGE_SIZE: usize = 500;
const SUBAGENT_PARENT_MESSAGE_SCAN_LIMIT: usize = 20_000;
const SUBAGENT_PARENT_DISCOVERY_LIMIT: usize = 4;
const SUBAGENT_CHILD_MESSAGE_LIMIT: usize = 200;
const SUBAGENT_ANCESTOR_RESOLUTION_LIMIT: usize = 200;
const SUBAGENT_VISIBLE_LIMIT: usize = 10;
const API_DISCOVERED_SUBAGENT_FIELD: &str = "_yahu_api_discovered_subagent";
const SUBAGENT_LOOKBACK_SECONDS: f64 = 48.0 * 60.0 * 60.0;
const SUBAGENT_STALE_RUNNING_SECONDS: f64 = 900.0;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    context: Option<String>,
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
    #[serde(skip_serializing)]
    source_session_id: String,
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
    last_active: Option<f64>,
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
        let goal_session_id = goal.source_session_id.clone();
        let goal_created_at = goal.created_at;
        match timeout(
            SUBAGENT_POLL_TIMEOUT,
            fetch_parent_session_todos(
                &state,
                &goal_session_id,
                goal_created_at,
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
    websocket_origin_allowed(headers)
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
    empty_streak: u32,
) -> Duration {
    let active = subagents.iter().any(|item| item.status == "running")
        || goal.is_some_and(|item| item.status == "active");
    // An empty projection keeps a fast cadence only while it may still be
    // catching a fresh delegation; after that it backs off to the idle rate
    // instead of re-scanning the window every second forever.
    if active || (subagents.is_empty() && empty_streak < SUBAGENT_EMPTY_FAST_POLLS) {
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
    let mut empty_streak = 0u32;
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
            let goal_session_id = goal.source_session_id.clone();
            let goal_created_at = goal.created_at;
            match timeout(
                SUBAGENT_POLL_TIMEOUT,
                fetch_parent_session_todos(
                    &state,
                    &goal_session_id,
                    goal_created_at,
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
        if current_subagents.is_empty() && current_goal.is_none() {
            empty_streak = empty_streak.saturating_add(1);
        } else {
            empty_streak = 0;
        }
        next_poll =
            Instant::now() + subagent_poll_delay(&current_subagents, current_goal.as_ref(), empty_streak);
    }
}

type GoalCandidateKeyRow = (
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

/// Session ids whose `goal:` records may belong to the same visible chat:
/// the requested id plus every segment sharing its session key.
fn load_goal_candidate_session_ids(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> anyhow::Result<Vec<String>> {
    let mut candidates = vec![session_id.to_string()];
    if !sqlite_table_has_columns(
        conn,
        "sessions",
        &["id", "started_at", "source", "session_key", "chat_id", "thread_id"],
    )? {
        return Ok(candidates);
    }
    let row: Option<GoalCandidateKeyRow> = conn
        .query_row(
            "SELECT source, session_key, chat_id, thread_id FROM sessions WHERE id = ?1",
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((source, session_key, chat_id, thread_id)) = row else {
        return Ok(candidates);
    };
    let Some(session_key) = session_key.filter(|value| !value.trim().is_empty()) else {
        return Ok(candidates);
    };
    let mut statement = conn.prepare(
        "SELECT id FROM sessions
         WHERE session_key = ?1
           AND source IS ?2
           AND chat_id IS ?3
           AND thread_id IS ?4
           AND COALESCE(source, '') != 'subagent'
         ORDER BY started_at, id
         LIMIT 200",
    )?;
    let ids = statement
        .query_map(
            rusqlite::params![session_key, source, chat_id, thread_id],
            |row| row.get::<_, String>(0),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if !ids.is_empty() {
        candidates = ids;
    }
    Ok(candidates)
}

fn load_subagent_parent_session_ids(
    hermes_home: &Path,
    session_id: &str,
) -> anyhow::Result<HashSet<String>> {
    let mut parent_ids = HashSet::from([session_id.to_string()]);
    let db_path = hermes_home.join("state.db");
    if !db_path.exists() {
        return Ok(parent_ids);
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    parent_ids.extend(load_goal_candidate_session_ids(&conn, session_id)?);
    Ok(parent_ids)
}

fn load_subagent_parent_discovery_ids(
    hermes_home: &Path,
    requested_parent_id: &str,
    parent_session_ids: &HashSet<String>,
    window_start: f64,
    window_end: f64,
) -> anyhow::Result<Vec<String>> {
    let mut ordered = vec![requested_parent_id.to_string()];
    let db_path = hermes_home.join("state.db");
    if !db_path.exists() || parent_session_ids.len() <= 1 {
        return Ok(ordered);
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    if !sqlite_table_has_columns(&conn, "sessions", &["id", "started_at", "ended_at"])? {
        return Ok(ordered);
    }
    let mut candidates = Vec::new();
    for id in parent_session_ids.iter() {
        let Some((started_at, ended_at)) = conn
            .query_row(
                "SELECT started_at, ended_at FROM sessions WHERE id = ?1",
                [id.as_str()],
                |row| Ok((row.get::<_, f64>(0)?, row.get::<_, Option<f64>>(1)?)),
            )
            .optional()?
        else {
            continue;
        };
        let overlaps_window = started_at <= window_end
            && (ended_at.is_none() || ended_at.is_some_and(|ended| ended >= window_start));
        if overlaps_window {
            candidates.push((
                ended_at.unwrap_or(window_end).max(started_at),
                started_at,
                id.clone(),
            ));
        }
    }
    candidates.sort_by(|left, right| {
        right
            .0
            .total_cmp(&left.0)
            .then_with(|| right.1.total_cmp(&left.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    ordered = candidates
        .into_iter()
        .map(|(_, _, id)| id)
        .take(SUBAGENT_PARENT_DISCOVERY_LIMIT)
        .collect();
    if ordered.is_empty() {
        ordered.push(requested_parent_id.to_string());
    }
    Ok(ordered)
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
    // The goal manager persists one record per session incarnation, so a chat
    // that continued through compression/switch epochs keeps its live goal on
    // the newest incarnation, not on the canonical (oldest) session id.
    let candidate_ids = load_goal_candidate_session_ids(&conn, session_id)?;
    let mut best: Option<(f64, f64, String, Value)> = None;
    for candidate in &candidate_ids {
        let raw = conn
            .query_row(
                "SELECT value FROM state_meta WHERE key = ?1",
                [format!("goal:{candidate}")],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(raw) = raw else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        if string_field(&value, "goal").is_none() {
            continue;
        }
        let candidate_created = number_field(&value, "created_at").unwrap_or_default();
        let candidate_turn = number_field(&value, "last_turn_at").unwrap_or_default();
        let replace = match best.as_ref() {
            None => true,
            Some((best_created, best_turn, _, _)) => candidate_created
                .total_cmp(best_created)
                .then_with(|| candidate_turn.total_cmp(best_turn))
                == std::cmp::Ordering::Greater,
        };
        if replace {
            best = Some((candidate_created, candidate_turn, candidate.clone(), value));
        }
    }
    let Some((created_at, _rank_turn, goal_source_id, value)) = best else {
        return Ok(None);
    };
    // The newest record decides: a completed or cleared goal shows nothing —
    // never fall back to an older incarnation's stale goal.
    let raw_status = string_field(&value, "status").unwrap_or_else(|| "active".to_string());
    if matches!(raw_status.as_str(), "done" | "cleared") {
        return Ok(None);
    }
    let Some(text) = string_field(&value, "goal") else {
        return Ok(None);
    };
    let status = match raw_status.as_str() {
        "paused" => raw_status,
        _ => "active".to_string(),
    };
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
    let milestone_cache_key = format!("yahu:goal_milestones:{goal_source_id}");
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
        source_session_id: goal_source_id,
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

async fn fetch_child_messages_bounded<T, F, Fut>(
    items: Vec<T>,
    fetch: F,
) -> anyhow::Result<Vec<(T, Vec<Value>)>>
where
    T: Clone,
    F: Fn(T) -> Fut + Clone,
    Fut: std::future::Future<Output = anyhow::Result<Vec<Value>>>,
{
    futures_util::stream::iter(items.into_iter().map(|item| {
        let fetch = fetch.clone();
        let request_item = item.clone();
        async move {
            let messages = fetch(request_item.clone()).await?;
            Ok::<_, anyhow::Error>((request_item, messages))
        }
    }))
    .buffer_unordered(SUBAGENT_SNAPSHOT_CONCURRENCY)
    .collect::<Vec<_>>()
    .await
    .into_iter()
    .collect()
}

async fn fetch_subagent_projection_snapshot(
    state: &AppState,
    parent_session_id: &str,
    window_end: f64,
    cache: &mut HashMap<String, CachedSubagentProjection>,
) -> anyhow::Result<Vec<SubagentProjection>> {
    let window_start = window_end - SUBAGENT_LOOKBACK_SECONDS;
    let parent_session_ids = load_subagent_parent_session_ids(&state.hermes_home, parent_session_id)?;
    let discovery_parent_ids = load_subagent_parent_discovery_ids(
        &state.hermes_home,
        parent_session_id,
        &parent_session_ids,
        window_start,
        window_end,
    )?;
    let mut sessions = fetch_subagent_sessions_for_parent(state, parent_session_id, window_end).await?;
    let api_child_ids =
        match fetch_api_delegate_child_ids(state, &discovery_parent_ids, &sessions, window_start).await
        {
        Ok(ids) => ids,
        Err(err) => {
            warn!(error = %err, parent_session_id, "cannot discover delegated child ids from API transcript");
            HashSet::new()
        }
    };
    mark_api_discovered_subagents(&mut sessions, &api_child_ids);
    resolve_missing_subagent_ancestors_for_parents(
        state,
        &mut sessions,
        &parent_session_ids,
        window_end,
    )
    .await?;
    let visible = select_visible_subagent_sessions_for_parents(&parent_session_ids, &sessions, window_end);
    let visible_ids = visible
        .iter()
        .filter_map(|session| string_field(session, "id"))
        .collect::<HashSet<_>>();
    cache.retain(|session_id, _| visible_ids.contains(session_id));

    let mut resolved = (0..visible.len())
        .map(|_| None::<CachedSubagentProjection>)
        .collect::<Vec<_>>();
    let mut pending = Vec::new();
    for (index, session) in visible.into_iter().enumerate() {
        let Some(session_id) = string_field(&session, "id") else {
            continue;
        };
        let message_count = u64_field(&session, "message_count");
        let ended_at = number_field(&session, "ended_at");
        let last_active = number_field(&session, "last_active");
        if let Some(cached) = cache.get(&session_id)
            && cached.message_count == message_count
            && cached.ended_at == ended_at
            && cached.last_active == last_active
        {
            resolved[index] = Some(cached.clone());
        } else {
            pending.push((index, session, session_id, message_count, ended_at, last_active));
        }
    }

    let fetched = fetch_child_messages_bounded(pending, |item| async move {
        fetch_session_messages_tail(state, &item.2, SUBAGENT_CHILD_MESSAGE_LIMIT).await
    })
    .await?;
    for ((index, session, _session_id, message_count, ended_at, last_active), messages) in fetched {
        let Some(projection) = project_subagent_session(&state.hermes_home, &session, &messages) else {
            continue;
        };
        resolved[index] = Some(CachedSubagentProjection {
            message_count,
            ended_at,
            last_active,
            projection,
        });
    }

    let mut out = Vec::with_capacity(resolved.len());
    for cached in resolved.into_iter().flatten() {
        let session_id = cached.projection.session_id.clone();
        let projection = mark_stale_running_subagent(
            cached.projection.clone(),
            cached.last_active,
            window_end,
        );
        cache.insert(session_id, cached);
        out.push(mark_subagent_omitted_ancestry_for_parents(
            projection,
            &visible_ids,
            &parent_session_ids,
        ));
    }
    Ok(out)
}

#[cfg(test)]
fn mark_subagent_omitted_ancestry(
    projection: SubagentProjection,
    visible_ids: &HashSet<String>,
    parent_session_id: &str,
) -> SubagentProjection {
    let parent_session_ids = HashSet::from([parent_session_id.to_string()]);
    mark_subagent_omitted_ancestry_for_parents(projection, visible_ids, &parent_session_ids)
}

fn mark_subagent_omitted_ancestry_for_parents(
    mut projection: SubagentProjection,
    visible_ids: &HashSet<String>,
    parent_session_ids: &HashSet<String>,
) -> SubagentProjection {
    projection.ancestry_omitted = !parent_session_ids.contains(&projection.parent_session_id)
        && !visible_ids.contains(&projection.parent_session_id);
    projection
}

fn mark_stale_running_subagent(
    mut projection: SubagentProjection,
    last_active: Option<f64>,
    now: f64,
) -> SubagentProjection {
    if projection.status == "running"
        && last_active.is_some_and(|last| {
            last.is_finite() && now.is_finite() && now >= last + SUBAGENT_STALE_RUNNING_SECONDS
        })
    {
        projection.status = "interrupted".to_string();
        projection.ended_at = last_active;
        projection.current_tool = None;
    }
    projection
}

async fn fetch_subagent_sessions_for_parent(
    state: &AppState,
    parent_session_id: &str,
    window_end: f64,
) -> anyhow::Result<Vec<Value>> {
    let url = format!(
        "{}/api/sessions/{}",
        state.api_url.trim_end_matches('/'),
        path_segment(parent_session_id),
    );
    let body = fetch_api_json(state, url, SUBAGENT_API_PAGE_BYTE_LIMIT).await?;
    let session = body.get("session").unwrap_or(&body);
    let parent_source = string_field(session, "source");
    let sources = subagent_candidate_sources(parent_source.as_deref());
    fetch_subagent_sessions_from_sources(state, window_end, &sources).await
}

fn subagent_candidate_sources(parent_source: Option<&str>) -> Vec<String> {
    let mut sources = vec!["subagent".to_string()];
    if let Some(source) = parent_source
        .map(str::trim)
        .filter(|source| !source.is_empty() && *source != "subagent")
    {
        sources.push(source.to_string());
    }
    sources
}

#[cfg(test)]
async fn fetch_subagent_sessions(state: &AppState, window_end: f64) -> anyhow::Result<Vec<Value>> {
    fetch_subagent_sessions_from_sources(state, window_end, &[]).await
}

async fn fetch_subagent_sessions_from_sources(
    state: &AppState,
    window_end: f64,
    sources: &[String],
) -> anyhow::Result<Vec<Value>> {
    let source_filters = if sources.is_empty() {
        vec![None]
    } else {
        sources.iter().map(Some).collect::<Vec<_>>()
    };
    // Sources scan independently; running them serially doubled the snapshot's
    // wall time for the same candidate rows.
    let scans = source_filters
        .into_iter()
        .map(|source| scan_subagent_sessions_for_source(state, window_end, source.cloned()));
    let parts = futures_util::future::try_join_all(scans).await?;
    let mut sessions = HashMap::<String, Value>::new();
    for part in parts {
        for session in part {
            if let Some(id) = string_field(&session, "id") {
                sessions.entry(id).or_insert(session);
            }
        }
    }
    Ok(sessions.into_values().collect())
}

async fn scan_subagent_sessions_for_source(
    state: &AppState,
    window_end: f64,
    source: Option<String>,
) -> anyhow::Result<Vec<Value>> {
    // `source=subagent` only matches the legacy child source. The parent
    // source is queried alongside it because current Hermes children may
    // inherit that source; transcript and lineage checks still classify
    // the rows locally.
    let source_query = source
        .map(|value| format!("&source={}", utf8_percent_encode(&value, NON_ALPHANUMERIC)))
        .unwrap_or_default();
    let window_start = window_end - SUBAGENT_LOOKBACK_SECONDS;
    let mut sessions = Vec::new();
    let mut seen = HashSet::new();
    let mut offset = 0usize;
    loop {
        let url = format!(
            "{}/api/sessions?include_children=true&limit={}&offset={}{}",
            state.api_url.trim_end_matches('/'),
            SUBAGENT_PAGE_SIZE,
            offset,
            source_query,
        );
        let body = fetch_api_json(state, url, SUBAGENT_API_PAGE_BYTE_LIMIT).await?;
        let data = body
            .get("sessions")
            .or_else(|| body.get("data"))
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
            if seen.insert(id) {
                sessions.push(session);
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
    Ok(sessions)
}

#[cfg(test)]
async fn resolve_missing_subagent_ancestors(
    state: &AppState,
    sessions: &mut Vec<Value>,
    parent_session_id: &str,
    window_end: f64,
) -> anyhow::Result<()> {
    let parent_session_ids = HashSet::from([parent_session_id.to_string()]);
    resolve_missing_subagent_ancestors_for_parents(state, sessions, &parent_session_ids, window_end).await
}

async fn resolve_missing_subagent_ancestors_for_parents(
    state: &AppState,
    sessions: &mut Vec<Value>,
    parent_session_ids: &HashSet<String>,
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
            is_subagent_session(session)
                && number_field(session, "started_at")
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
        let candidate_discovered = by_id
            .get(&candidate_id)
            .and_then(|session| session.get(API_DISCOVERED_SUBAGENT_FIELD))
            .and_then(Value::as_bool)
            .unwrap_or(false);
        loop {
            match subagent_membership_or_missing_for_parents(&candidate_id, &by_id, parent_session_ids) {
                Ok(true) => {
                    matched += 1;
                    break;
                }
                Ok(false) => break,
                Err(missing_id) => {
                    if !candidate_discovered {
                        // Only transcript-discovered candidates justify ancestor network
                        // fetches; foreign candidates resolve in-memory, so one snapshot
                        // no longer chains API calls for unrelated children.
                        break;
                    }
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

fn subagent_membership_or_missing_for_parents(
    session_id: &str,
    sessions_by_id: &HashMap<String, Value>,
    parent_session_ids: &HashSet<String>,
) -> Result<bool, String> {
    let mut current = session_id.to_string();
    let mut seen = HashSet::new();
    while seen.insert(current.clone()) {
        if parent_session_ids.contains(&current) {
            return Ok(true);
        }
        let Some(session) = sessions_by_id.get(&current) else {
            return Err(current);
        };
        // A source-inherited child may use ordinary continuation sessions as
        // parents; resolve the full chain before deciding membership.
        if let Some(lineage_root) = string_field(session, "_lineage_root_id") {
            return Ok(parent_session_ids.contains(&lineage_root));
        }
        if is_session_switch_continuation(
            session,
            string_field(session, "parent_session_id")
                .and_then(|parent_id| sessions_by_id.get(&parent_id)),
        ) {
            // A delegated child that became the conversation continuation is a
            // parent session, never a subagent row.
            return Ok(false);
        }
        let Some(parent) = string_field(session, "parent_session_id") else {
            return Ok(false);
        };
        if parent_session_ids.contains(&parent) {
            return Ok(true);
        }
        current = parent;
    }
    Ok(false)
}

/// A same-source child whose parent ended with `session_switch` is a transcript
/// continuation of that parent, not a subagent. Pure in-memory check over rows
/// already fetched from the session list; adds no I/O and no retained state.
fn is_session_switch_continuation(child: &Value, parent: Option<&Value>) -> bool {
    let Some(parent) = parent else {
        return false;
    };
    string_field(parent, "end_reason").as_deref() == Some("session_switch")
        && string_field(child, "source") == string_field(parent, "source")
}

fn has_delegate_marker(session: &Value) -> bool {
    let Some(model_config) = session.get("model_config") else {
        return false;
    };
    match model_config {
        Value::Object(config) => config
            .get("_delegate_from")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty()),
        Value::String(raw) => serde_json::from_str::<Value>(raw)
            .ok()
            .and_then(|config| config.get("_delegate_from").and_then(Value::as_str).map(str::to_string))
            .is_some_and(|value| !value.trim().is_empty()),
        _ => false,
    }
}

fn is_subagent_session(session: &Value) -> bool {
    // API Server strips model_config, so the caller adds an internal marker after
    // discovering child ids from the parent transcript. Keep raw source/marker
    // support for legacy rows and direct unit/API fixtures.
    session
        .get(API_DISCOVERED_SUBAGENT_FIELD)
        .and_then(Value::as_bool)
        .unwrap_or(false)
        || string_field(session, "source").is_none_or(|source| source == "subagent")
        || has_delegate_marker(session)
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
        .get("messages")
        .or_else(|| body.get("data"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// Bounded tail read for projection snapshots. The projector only reads recent
/// activity (todos, tool trail, latest assistant text), so fetching a long child
/// transcript in full was pure cost.
async fn fetch_session_messages_tail(
    state: &AppState,
    session_id: &str,
    limit: usize,
) -> anyhow::Result<Vec<Value>> {
    let url = format!(
        "{}/api/sessions/{}/messages?order=latest&limit={}",
        state.api_url.trim_end_matches('/'),
        path_segment(session_id),
        limit,
    );
    let body = fetch_api_json(state, url, SUBAGENT_API_DETAIL_BYTE_LIMIT).await?;
    Ok(body
        .get("messages")
        .or_else(|| body.get("data"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// Fetch the parent transcript newest-page-first, stopping at the lookback boundary.
/// Delegation discovery only needs the current window; scanning a huge transcript
/// from its oldest row cost dozens of pages and starved the snapshot timeout.
async fn fetch_recent_parent_messages(
    state: &AppState,
    session_id: &str,
    window_start: f64,
) -> anyhow::Result<Vec<Value>> {
    let mut messages = Vec::new();
    let mut offset = 0usize;
    loop {
        let url = format!(
            "{}/api/sessions/{}/messages?order=latest&limit={}&offset={}",
            state.api_url.trim_end_matches('/'),
            path_segment(session_id),
            SUBAGENT_PARENT_MESSAGE_PAGE_SIZE,
            offset,
        );
        let body = fetch_api_json(state, url, SUBAGENT_API_DETAIL_BYTE_LIMIT).await?;
        let page = body
            .get("messages")
            .or_else(|| body.get("data"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let count = page.len();
        let oldest_in_page = page
            .first()
            .and_then(|message| number_field(message, "timestamp"));
        messages.extend(page);
        if count < SUBAGENT_PARENT_MESSAGE_PAGE_SIZE {
            break;
        }
        if window_start.is_finite()
            && oldest_in_page.is_some_and(|timestamp| timestamp <= window_start)
        {
            break;
        }
        if messages.len() >= SUBAGENT_PARENT_MESSAGE_SCAN_LIMIT {
            messages.truncate(SUBAGENT_PARENT_MESSAGE_SCAN_LIMIT);
            break;
        }
        offset = offset.saturating_add(count);
    }
    Ok(messages)
}

fn collect_known_session_ids(value: &Value, known_ids: &HashSet<String>, out: &mut HashSet<String>) {
    match value {
        Value::Object(object) => {
            for (key, child) in object {
                if matches!(key.as_str(), "process" | "process_notes" | "live_transcript" | "live_transcripts") {
                    continue;
                }
                if key == "session_id"
                    && child
                        .as_str()
                        .is_some_and(|session_id| known_ids.contains(session_id))
                    && let Some(session_id) = child.as_str()
                {
                    out.insert(session_id.to_string());
                }
                if key == "subagent_ids"
                    && let Some(ids) = child.as_array()
                {
                    out.extend(
                        ids.iter()
                            .filter_map(Value::as_str)
                            .filter(|id| known_ids.contains(*id))
                            .map(str::to_owned),
                    );
                }
                collect_known_session_ids(child, known_ids, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_known_session_ids(item, known_ids, out);
            }
        }
        Value::String(raw) => {
            if let Ok(parsed) = serde_json::from_str::<Value>(raw) {
                collect_known_session_ids(&parsed, known_ids, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
fn delegate_child_ids_from_messages(
    messages: &[Value],
    known_ids: &HashSet<String>,
    parent_session_id: &str,
) -> HashSet<String> {
    let parent_session_ids = HashSet::from([parent_session_id.to_string()]);
    delegate_child_ids_from_messages_for_parents(messages, known_ids, &parent_session_ids)
}

fn delegate_child_ids_from_messages_for_parents(
    messages: &[Value],
    known_ids: &HashSet<String>,
    parent_session_ids: &HashSet<String>,
) -> HashSet<String> {
    let mut child_ids = HashSet::new();
    for message in messages {
        collect_known_session_ids(message, known_ids, &mut child_ids);
    }
    child_ids.retain(|session_id| !parent_session_ids.contains(session_id));
    child_ids
}

fn collect_delegate_goal_texts(value: &Value, out: &mut HashSet<String>) {
    match value {
        Value::Object(object) => {
            if object
                .get("function")
                .and_then(Value::as_object)
                .and_then(|function| function.get("name"))
                .and_then(Value::as_str)
                == Some("delegate_task")
                && let Some(arguments) = object
                    .get("function")
                    .and_then(Value::as_object)
                    .and_then(|function| function.get("arguments"))
            {
                collect_delegate_goal_payload(arguments, out);
            }
            for child in object.values() {
                collect_delegate_goal_texts(child, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_delegate_goal_texts(item, out);
            }
        }
        Value::String(raw) => {
            if let Ok(parsed) = serde_json::from_str::<Value>(raw) {
                collect_delegate_goal_texts(&parsed, out);
            }
        }
        _ => {}
    }
}

fn collect_delegate_goal_payload(value: &Value, out: &mut HashSet<String>) {
    match value {
        Value::Object(object) => {
            if let Some(goal) = object.get("goal").and_then(Value::as_str)
                && !goal.trim().is_empty()
            {
                out.insert(normalize_api_match_text(goal));
            }
            for key in ["goals", "tasks"] {
                if let Some(items) = object.get(key).and_then(Value::as_array) {
                    for item in items {
                        if let Some(goal) = item.as_str() {
                            if !goal.trim().is_empty() {
                                out.insert(normalize_api_match_text(goal));
                            }
                        } else {
                            collect_delegate_goal_payload(item, out);
                        }
                    }
                }
            }
        }
        Value::String(raw) => {
            if let Ok(parsed) = serde_json::from_str::<Value>(raw) {
                collect_delegate_goal_payload(&parsed, out);
            }
        }
        _ => {}
    }
}

fn normalize_api_match_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

fn preview_matches_delegate_goal(preview: &str, goals: &HashSet<String>) -> bool {
    let preview = normalize_api_match_text(preview);
    if preview.len() < 8 {
        return false;
    }
    let truncated_prefix = preview
        .strip_suffix("...")
        .or_else(|| preview.strip_suffix('…'))
        .map(str::trim_end)
        .filter(|prefix| prefix.chars().count() >= 24);
    goals.iter().any(|goal| {
        goal == &preview
            || (goal.len() >= 8 && (goal.contains(&preview) || preview.contains(goal)))
            || truncated_prefix.is_some_and(|prefix| goal.starts_with(prefix))
    })
}

#[cfg(test)]
fn delegate_child_ids_from_goal_previews(
    messages: &[Value],
    sessions: &[Value],
    parent_session_id: &str,
) -> HashSet<String> {
    let parent_session_ids = HashSet::from([parent_session_id.to_string()]);
    delegate_child_ids_from_goal_previews_for_parents(messages, sessions, &parent_session_ids)
}

fn delegate_child_ids_from_goal_previews_for_parents(
    messages: &[Value],
    sessions: &[Value],
    parent_session_ids: &HashSet<String>,
) -> HashSet<String> {
    let mut goals = HashSet::new();
    for message in messages {
        collect_delegate_goal_texts(message, &mut goals);
    }
    if goals.is_empty() {
        return HashSet::new();
    }
    let parent_sources = parent_session_ids
        .iter()
        .map(|parent_id| {
            let source = sessions
                .iter()
                .find(|session| string_field(session, "id").as_deref() == Some(parent_id.as_str()))
                .and_then(|session| string_field(session, "source"));
            (parent_id.clone(), source)
        })
        .collect::<HashMap<_, _>>();
    sessions
        .iter()
        .filter(|session| {
            let Some(parent_id) = string_field(session, "parent_session_id") else {
                return false;
            };
            parent_session_ids.contains(&parent_id)
                && parent_sources
                    .get(&parent_id)
                    .is_some_and(|source| source.as_deref() == string_field(session, "source").as_deref())
        })
        .filter_map(|session| {
            let preview = string_field(session, "preview")?;
            preview_matches_delegate_goal(&preview, &goals).then(|| string_field(session, "id"))?
        })
        .collect()
}

async fn fetch_api_delegate_child_ids(
    state: &AppState,
    parent_session_ids: &[String],
    sessions: &[Value],
    window_start: f64,
) -> anyhow::Result<HashSet<String>> {
    let known_ids = sessions
        .iter()
        .filter_map(|session| string_field(session, "id"))
        .collect::<HashSet<_>>();
    let mut messages = Vec::new();
    for parent_session_id in parent_session_ids {
        messages.extend(fetch_recent_parent_messages(state, parent_session_id, window_start).await?);
    }
    let parent_session_ids = parent_session_ids.iter().cloned().collect::<HashSet<_>>();
    let mut child_ids = delegate_child_ids_from_messages_for_parents(
        &messages,
        &known_ids,
        &parent_session_ids,
    );
    child_ids.extend(delegate_child_ids_from_goal_previews_for_parents(
        &messages,
        sessions,
        &parent_session_ids,
    ));
    Ok(child_ids)
}

fn mark_api_discovered_subagents(sessions: &mut [Value], direct_child_ids: &HashSet<String>) {
    let parents = sessions
        .iter()
        .filter_map(|session| {
            Some((string_field(session, "id")?, string_field(session, "parent_session_id")))
        })
        .collect::<HashMap<_, _>>();
    let mut known = direct_child_ids.clone();
    loop {
        let mut added = false;
        for session in sessions.iter_mut() {
            let Some(id) = string_field(session, "id") else {
                continue;
            };
            let is_direct = direct_child_ids.contains(&id);
            let is_descendant = parents
                .get(&id)
                .and_then(Option::as_ref)
                .is_some_and(|parent| known.contains(parent));
            if is_direct || is_descendant {
                if known.insert(id.clone()) {
                    added = true;
                }
                if let Some(object) = session.as_object_mut() {
                    object.insert(API_DISCOVERED_SUBAGENT_FIELD.to_string(), Value::Bool(true));
                }
            }
        }
        if !added {
            break;
        }
    }
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

#[cfg(test)]
fn select_visible_subagent_sessions(
    parent_session_id: &str,
    sessions: &[Value],
    window_end: f64,
) -> Vec<Value> {
    let parent_session_ids = HashSet::from([parent_session_id.to_string()]);
    select_visible_subagent_sessions_for_parents(&parent_session_ids, sessions, window_end)
}

fn select_visible_subagent_sessions_for_parents(
    parent_session_ids: &HashSet<String>,
    sessions: &[Value],
    window_end: f64,
) -> Vec<Value> {
    let window_start = window_end - SUBAGENT_LOOKBACK_SECONDS;
    let session_by_id = sessions
        .iter()
        .filter_map(|session| Some((string_field(session, "id")?, session)))
        .collect::<HashMap<_, _>>();

    let mut visible = sessions
        .iter()
        .filter(|session| {
            // include_children=true can return ordinary channel sessions from
            // the same lineage; only actual subagent sessions belong here.
            if !is_subagent_session(session) {
                return false;
            }
            let Some(started_at) = number_field(session, "started_at") else {
                return false;
            };
            let ended_at = number_field(session, "ended_at");
            if started_at > window_end || (ended_at.is_some() && started_at < window_start) {
                return false;
            }
            if string_field(session, "_lineage_root_id")
                .is_some_and(|lineage_root| parent_session_ids.contains(&lineage_root))
            {
                return true;
            }
            let Some(mut current) = string_field(session, "id") else {
                return false;
            };
            let mut seen = HashSet::new();
            let mut first_hop = true;
            while seen.insert(current.clone()) {
                if parent_session_ids.contains(&current) {
                    return true;
                }
                let Some(session) = session_by_id.get(&current) else {
                    return false;
                };
                // Inherited-source children can be nested below ordinary
                // conversation continuations; only the candidate itself must
                // be a subagent, while the chain still has to reach one of the
                // selected chat family's main-session segments.
                if string_field(session, "_lineage_root_id")
                    .is_some_and(|lineage_root| parent_session_ids.contains(&lineage_root))
                {
                    return true;
                }
                if first_hop
                    && is_session_switch_continuation(
                        session,
                        string_field(session, "parent_session_id")
                            .and_then(|parent_id| session_by_id.get(&parent_id).copied()),
                    )
                {
                    // A delegated child that became the conversation
                    // continuation is a parent session, never a subagent row.
                    return false;
                }
                first_hop = false;
                let Some(parent) = string_field(session, "parent_session_id") else {
                    return false;
                };
                if parent_session_ids.contains(&parent) {
                    return true;
                }
                current = parent;
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

fn is_subagent_task_marker(text: &str) -> bool {
    let first_line = text.lines().next().unwrap_or_default().trim().to_ascii_lowercase();
    first_line.contains("context compaction")
        || first_line.contains("prior context")
        || first_line.contains("active task list was preserved across context compression")
}

fn valid_subagent_user_text(message: &Value) -> Option<String> {
    if message.get("role").and_then(Value::as_str) != Some("user") {
        return None;
    }
    let text = message.get("content").map(content_text)?;
    let text = text.trim();
    if text.is_empty() || is_subagent_task_marker(text) {
        return None;
    }
    Some(text.to_string())
}

fn first_subagent_user_text(messages: &[Value]) -> Option<String> {
    let mut candidates = messages
        .iter()
        .enumerate()
        .filter_map(|(index, message)| Some((index, message, valid_subagent_user_text(message)?)))
        .collect::<Vec<_>>();
    candidates.sort_by(|(left_index, left, _), (right_index, right, _)| {
        number_field(left, "timestamp")
            .unwrap_or(f64::INFINITY)
            .total_cmp(&number_field(right, "timestamp").unwrap_or(f64::INFINITY))
            .then_with(|| left_index.cmp(right_index))
    });
    candidates.into_iter().next().map(|(_, _, text)| text)
}

fn persisted_subagent_content_text(raw: String) -> String {
    serde_json::from_str::<Value>(&raw)
        .ok()
        .filter(|value| value.is_array() || value.is_object())
        .map(|value| content_text(&value))
        .unwrap_or(raw)
}

fn load_initial_subagent_user(hermes_home: &Path, session_id: &str) -> Option<String> {
    let db_path = hermes_home.join("state.db");
    if !db_path.exists() {
        return None;
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let mut statement = conn
        .prepare(
            "SELECT content FROM messages
             WHERE session_id = ?1 AND role = 'user' AND content IS NOT NULL
             ORDER BY timestamp, id",
        )
        .ok()?;
    let rows = statement
        .query_map(rusqlite::params![session_id], |row| row.get::<_, String>(0))
        .ok()?;
    for raw in rows.flatten() {
        let text = persisted_subagent_content_text(raw);
        if !text.trim().is_empty() && !is_subagent_task_marker(&text) {
            return Some(text.trim().to_string());
        }
    }
    None
}

fn subagent_creation_goal(hermes_home: &Path, session: &Value, messages: &[Value]) -> String {
    string_field(session, "goal")
        .filter(|text| !is_subagent_task_marker(text))
        .or_else(|| {
            session
                .get("model_config")
                .and_then(|config| string_field(config, "goal"))
                .filter(|text| !is_subagent_task_marker(text))
        })
        .or_else(|| {
            string_field(session, "id")
                .and_then(|session_id| load_initial_subagent_user(hermes_home, &session_id))
        })
        .or_else(|| first_subagent_user_text(messages))
        .or_else(|| {
            string_field(session, "preview")
                .filter(|text| !is_subagent_task_marker(text))
        })
        .unwrap_or_else(|| "Subagent".to_string())
}

fn project_subagent_session(hermes_home: &Path, session: &Value, messages: &[Value]) -> Option<SubagentProjection> {
    let session_id = string_field(session, "id")?;
    let parent_session_id = string_field(session, "parent_session_id").unwrap_or_default();
    let task = subagent_creation_goal(hermes_home, session, messages);
    let context = number_field(session, "started_at").and_then(|started_at| {
        load_subagent_context(hermes_home, &parent_session_id, started_at, &task)
    });

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
        context,
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

fn load_subagent_context(
    hermes_home: &Path,
    parent_session_id: &str,
    child_started_at: f64,
    task: &str,
) -> Option<String> {
    let db_path = hermes_home.join("state.db");
    if !db_path.exists() {
        return None;
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY
            | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()?;
    let mut statement = conn
        .prepare(
            "SELECT tool_calls FROM messages
             WHERE session_id = ?1 AND role = 'assistant' AND tool_calls IS NOT NULL
               AND timestamp <= ?2
             ORDER BY timestamp DESC, id DESC",
        )
        .ok()?;
    let rows = statement
        .query_map(rusqlite::params![parent_session_id, child_started_at], |row| {
            row.get::<_, String>(0)
        })
        .ok()?;
    for raw in rows.flatten() {
        let Ok(calls) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let Some(calls) = calls.as_array() else {
            continue;
        };
        for call in calls {
            let function = call.get("function").and_then(Value::as_object);
            if function.and_then(|item| item.get("name")).and_then(Value::as_str) != Some("delegate_task") {
                continue;
            }
            let Some(arguments) = function.and_then(|item| item.get("arguments")) else {
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
            let context = if string_field(&parsed, "goal").as_deref() == Some(task) {
                string_field(&parsed, "context")
            } else {
                parsed
                    .get("tasks")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .find(|item| string_field(item, "goal").as_deref() == Some(task))
                    .and_then(|item| string_field(item, "context"))
            };
            if let Some(context) = context.filter(|value| !value.is_empty()) {
                return Some(context);
            }
        }
    }
    None
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
