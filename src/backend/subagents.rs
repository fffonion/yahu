const SUBAGENT_POLL_INTERVAL: Duration = Duration::from_millis(1_000);
const SUBAGENT_IDLE_POLL_INTERVAL: Duration = Duration::from_secs(15);
const SUBAGENT_POLL_TIMEOUT: Duration = Duration::from_secs(10);
const SUBAGENT_PAGE_SIZE: usize = 200;
// Progress is always represented by the most recently active subagents. Avoid walking
// historical pages on every poll when a Hermes installation has many old child sessions.
const SUBAGENT_MAX_PAGES: usize = 1;
const SUBAGENT_BATCH_WINDOW_SECONDS: f64 = 2.0;
const SUBAGENT_RECENT_ROOT_LIMIT: usize = 5;
const SUBAGENT_ACTIVITY_LIMIT: usize = 8;
const SUBAGENT_SUMMARY_LIMIT: usize = 600;

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
    goal: String,
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

#[derive(Serialize)]
struct SubagentSnapshot<'a> {
    #[serde(rename = "type")]
    kind: &'static str,
    session_id: &'a str,
    generated_at: f64,
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

fn subagent_poll_delay(subagents: &[SubagentProjection]) -> Duration {
    if subagents.is_empty() || subagents.iter().any(|item| item.status == "running") {
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
    let mut current_subagents = Vec::<SubagentProjection>::new();
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

        let error = match timeout(
            SUBAGENT_POLL_TIMEOUT,
            fetch_subagent_projection_snapshot(&state, &session_id, &mut cache),
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
        let fingerprint = serde_json::to_string(&(
            current_subagents.as_slice(),
            error.as_deref(),
        ))
        .unwrap_or_default();
        if fingerprint != last_fingerprint {
            last_fingerprint = fingerprint;
            let payload = SubagentSnapshot {
                kind: "subagents.snapshot",
                session_id: &session_id,
                generated_at: unix_now_seconds(),
                subagents: &current_subagents,
                error: error.as_deref(),
            };
            if let Ok(text) = serde_json::to_string(&payload) {
                sender.send_replace(text);
            }
        }
        next_poll = Instant::now() + subagent_poll_delay(&current_subagents);
    }
}

async fn fetch_subagent_projection_snapshot(
    state: &AppState,
    parent_session_id: &str,
    cache: &mut HashMap<String, CachedSubagentProjection>,
) -> anyhow::Result<Vec<SubagentProjection>> {
    let sessions = fetch_subagent_sessions(state).await?;
    let visible = select_visible_subagent_sessions(parent_session_id, &sessions);
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
            out.push(cached.projection.clone());
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
        out.push(projection);
    }
    Ok(out)
}

async fn fetch_subagent_sessions(state: &AppState) -> anyhow::Result<Vec<Value>> {
    let mut sessions = Vec::new();
    for page in 0..SUBAGENT_MAX_PAGES {
        let offset = page * SUBAGENT_PAGE_SIZE;
        let url = format!(
            "{}/api/sessions?source=subagent&include_children=true&limit={}&offset={}",
            state.api_url.trim_end_matches('/'),
            SUBAGENT_PAGE_SIZE,
            offset,
        );
        let body = fetch_api_json(state, url).await?;
        let data = body
            .get("data")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let has_more = body.get("has_more").and_then(Value::as_bool).unwrap_or(false);
        let count = data.len();
        sessions.extend(data);
        if !has_more || count < SUBAGENT_PAGE_SIZE {
            break;
        }
    }
    Ok(sessions)
}

async fn fetch_session_messages(state: &AppState, session_id: &str) -> anyhow::Result<Vec<Value>> {
    let url = format!(
        "{}/api/sessions/{}/messages",
        state.api_url.trim_end_matches('/'),
        path_segment(session_id),
    );
    let body = fetch_api_json(state, url).await?;
    Ok(body
        .get("data")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

async fn fetch_api_json(state: &AppState, url: String) -> anyhow::Result<Value> {
    let mut request = state.client.get(url);
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        request = request.bearer_auth(key);
    }
    let response = request.send().await?.error_for_status()?;
    Ok(response.json::<Value>().await?)
}

fn select_visible_subagent_sessions(parent_session_id: &str, sessions: &[Value]) -> Vec<Value> {
    let direct_roots = sessions
        .iter()
        .filter(|session| string_field(session, "parent_session_id").as_deref() == Some(parent_session_id))
        .collect::<Vec<_>>();
    if direct_roots.is_empty() {
        return Vec::new();
    }

    let parent_by_id = sessions
        .iter()
        .filter_map(|session| {
            Some((
                string_field(session, "id")?,
                string_field(session, "parent_session_id")?,
            ))
        })
        .collect::<HashMap<_, _>>();

    let mut selected_roots = HashSet::<String>::new();
    let active_sessions = sessions
        .iter()
        .filter(|session| session.get("ended_at").is_none_or(Value::is_null))
        .filter_map(|session| string_field(session, "id"))
        .collect::<Vec<_>>();

    for active_id in active_sessions {
        let mut current = active_id;
        let mut seen = HashSet::new();
        while seen.insert(current.clone()) {
            let Some(parent) = parent_by_id.get(&current) else {
                break;
            };
            if parent == parent_session_id {
                selected_roots.insert(current);
                break;
            }
            current = parent.clone();
        }
    }

    let anchor_times = if selected_roots.is_empty() {
        direct_roots
            .iter()
            .filter_map(|session| number_field(session, "started_at"))
            .max_by(f64::total_cmp)
            .into_iter()
            .collect::<Vec<_>>()
    } else {
        direct_roots
            .iter()
            .filter(|session| string_field(session, "id").is_some_and(|id| selected_roots.contains(&id)))
            .filter_map(|session| number_field(session, "started_at"))
            .collect::<Vec<_>>()
    };

    for session in &direct_roots {
        let Some(id) = string_field(session, "id") else {
            continue;
        };
        let Some(started_at) = number_field(session, "started_at") else {
            continue;
        };
        if anchor_times
            .iter()
            .any(|anchor| (started_at - anchor).abs() <= SUBAGENT_BATCH_WINDOW_SECONDS)
        {
            selected_roots.insert(id);
        }
    }

    let mut recent_roots = direct_roots.clone();
    recent_roots.sort_by(|left, right| {
        number_field(right, "started_at")
            .unwrap_or(f64::NEG_INFINITY)
            .total_cmp(&number_field(left, "started_at").unwrap_or(f64::NEG_INFINITY))
    });
    for session in recent_roots.into_iter().take(SUBAGENT_RECENT_ROOT_LIMIT) {
        if let Some(id) = string_field(session, "id") {
            selected_roots.insert(id);
        }
    }

    let mut visible = sessions
        .iter()
        .filter(|session| {
            let Some(mut current) = string_field(session, "id") else {
                return false;
            };
            let mut seen = HashSet::new();
            while seen.insert(current.clone()) {
                if selected_roots.contains(&current) {
                    return true;
                }
                let Some(parent) = parent_by_id.get(&current) else {
                    return false;
                };
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
    visible
}

fn project_subagent_session(session: &Value, messages: &[Value]) -> Option<SubagentProjection> {
    let session_id = string_field(session, "id")?;
    let parent_session_id = string_field(session, "parent_session_id").unwrap_or_default();
    let goal = messages
        .iter()
        .find(|message| message.get("role").and_then(Value::as_str) == Some("user"))
        .and_then(|message| message.get("content"))
        .map(content_text)
        .filter(|text| !text.trim().is_empty())
        .unwrap_or_else(|| "Subagent".to_string());

    let mut pending_tools = Vec::<(String, String)>::new();
    let mut completed_tool_ids = HashSet::<String>::new();
    let mut activity = Vec::<SubagentActivity>::new();
    let mut todos = Vec::<SubagentTodo>::new();
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
                if tool == "todo" {
                    let content = message.get("content").map(content_text).unwrap_or_default();
                    if let Ok(value) = serde_json::from_str::<Value>(&content)
                        && let Some(items) = value.get("todos").and_then(Value::as_array)
                    {
                        todos = items
                            .iter()
                            .filter_map(|item| {
                                let content = string_field(item, "content")?;
                                Some(SubagentTodo {
                                    id: string_field(item, "id").unwrap_or_default(),
                                    content: truncate_chars(content.trim(), 240),
                                    status: normalize_todo_status(item.get("status").and_then(Value::as_str)),
                                })
                            })
                            .collect();
                    }
                }
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
        goal: truncate_chars(goal.trim(), 500),
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
    match status.unwrap_or_default() {
        "completed" => "completed",
        "in_progress" => "in_progress",
        "cancelled" => "cancelled",
        _ => "pending",
    }
    .to_string()
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
