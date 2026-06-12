const API_SESSION_PAGE_SIZE: usize = 200;
const API_SESSION_SEARCH_SCAN_LIMIT: usize = 2_000;
const API_SESSION_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

async fn sessions_search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SessionSearchQuery>,
) -> Response<Body> {
    let limit = query.limit.unwrap_or(80).clamp(1, 100);
    let q = query.q.unwrap_or_default();
    match fetch_sessions_from_api_server(&state, &q, limit).await {
        Ok(data) => Json(serde_json::json!({
            "object": "list",
            "data": data,
            "limit": limit,
            "q": q,
        }))
        .into_response(),
        Err(err) => json_error(
            StatusCode::BAD_GATEWAY,
            &format!("session search API request failed: {err}"),
        ),
    }
}

async fn fetch_sessions_from_api_server(
    state: &AppState,
    query: &str,
    limit: usize,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let trimmed = query.trim();
    let mut offset = 0usize;
    let mut rows = Vec::new();
    let page_size = if trimmed.is_empty() {
        limit.saturating_mul(2).clamp(1, API_SESSION_PAGE_SIZE)
    } else {
        API_SESSION_PAGE_SIZE
    };
    let max_scan = if trimmed.is_empty() {
        API_SESSION_PAGE_SIZE
    } else {
        API_SESSION_SEARCH_SCAN_LIMIT.max(limit)
    };

    loop {
        let url = api_sessions_url(&state.api_url, page_size, offset, trimmed)?;
        let mut req = state.client.get(url);
        if let Some(key) = &state.api_key
            && !key.is_empty()
        {
            req = req.bearer_auth(key);
        }
        let resp = timeout(API_SESSION_REQUEST_TIMEOUT, req.send()).await??;
        if !resp.status().is_success() {
            anyhow::bail!("session list request failed: {}", resp.status());
        }
        let body = resp.json::<serde_json::Value>().await?;
        let data = body
            .get("data")
            .and_then(|value| value.as_array())
            .cloned()
            .unwrap_or_default();
        let data_len = data.len();
        for row in data {
            if !is_client_visible_session(&row) {
                continue;
            }
            let matches = session_matches_query(&row, trimmed)
                || session_messages_match_query(state, &row, trimmed)
                    .await
                    .unwrap_or(false);
            if matches {
                rows.push(row);
                if rows.len() >= limit {
                    return Ok(rows);
                }
            }
        }

        let has_more = body
            .get("has_more")
            .and_then(|value| value.as_bool())
            .unwrap_or(data_len == page_size);
        offset = offset.saturating_add(page_size);
        if trimmed.is_empty() || !has_more || data_len == 0 || offset >= max_scan {
            return Ok(rows);
        }
    }
}

fn api_sessions_url(
    api_url: &str,
    limit: usize,
    offset: usize,
    query: &str,
) -> anyhow::Result<String> {
    let mut params = vec![
        ("limit", limit.to_string()),
        ("offset", offset.to_string()),
        ("include_children", "false".to_string()),
    ];
    let trimmed = query.trim();
    if !trimmed.is_empty() {
        params.push(("q", trimmed.to_string()));
    }
    Ok(format!(
        "{}/api/sessions?{}",
        api_url.trim_end_matches('/'),
        serde_urlencoded::to_string(params)?
    ))
}

fn is_client_visible_session(row: &serde_json::Value) -> bool {
    row.get("source")
        .and_then(|value| value.as_str())
        .map(|source| source != "tool")
        .unwrap_or(true)
}

fn session_matches_query(row: &serde_json::Value, query: &str) -> bool {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return true;
    }
    ["id", "title", "preview", "model", "source"]
        .iter()
        .filter_map(|key| row.get(*key).and_then(|value| value.as_str()))
        .any(|value| value.to_lowercase().contains(&needle))
}

async fn session_messages_match_query(
    state: &AppState,
    row: &serde_json::Value,
    query: &str,
) -> anyhow::Result<bool> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Ok(true);
    }
    if row
        .get("message_count")
        .and_then(|value| value.as_i64())
        .is_some_and(|count| count <= 0)
    {
        return Ok(false);
    }
    let Some(session_id) = row.get("id").and_then(|value| value.as_str()) else {
        return Ok(false);
    };
    let encoded_id = utf8_percent_encode(session_id, NON_ALPHANUMERIC).to_string();
    let url = format!(
        "{}/api/sessions/{}/messages",
        state.api_url.trim_end_matches('/'),
        encoded_id
    );
    let mut req = state.client.get(url);
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        req = req.bearer_auth(key);
    }
    let resp = timeout(API_SESSION_REQUEST_TIMEOUT, req.send()).await??;
    if !resp.status().is_success() {
        return Ok(false);
    }
    let body = resp.json::<serde_json::Value>().await?;
    Ok(body
        .get("data")
        .and_then(|value| value.as_array())
        .map(|messages| {
            messages
                .iter()
                .any(|message| json_value_contains_query(message, &needle))
        })
        .unwrap_or(false))
}

fn json_value_contains_query(value: &serde_json::Value, needle: &str) -> bool {
    match value {
        serde_json::Value::String(text) => text.to_lowercase().contains(needle),
        serde_json::Value::Array(items) => items
            .iter()
            .any(|item| json_value_contains_query(item, needle)),
        serde_json::Value::Object(map) => map
            .values()
            .any(|item| json_value_contains_query(item, needle)),
        _ => false,
    }
}

async fn chat_messages_page(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
    Query(query): Query<ChatMessagesQuery>,
) -> Response<Body> {
    let limit = query.limit.unwrap_or(24).clamp(1, 80);
    let mut req = state.client.get(format!(
        "{}/api/sessions/{}/messages",
        state.api_url,
        path_segment(&session_id)
    ));
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        req = req.bearer_auth(key);
    }
    let resp = match req.send().await {
        Ok(resp) => resp,
        Err(err) => {
            return json_error(
                StatusCode::BAD_GATEWAY,
                &format!("message list proxy failed: {err}"),
            );
        }
    };
    if !resp.status().is_success() {
        return json_error(
            StatusCode::BAD_GATEWAY,
            &format!("message list request failed: {}", resp.status()),
        );
    }
    let body = match resp.json::<serde_json::Value>().await {
        Ok(body) => body,
        Err(err) => {
            return json_error(
                StatusCode::BAD_GATEWAY,
                &format!("cannot parse message list: {err}"),
            );
        }
    };
    let all = body
        .get("data")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let (start, end) = if let Some(before) = query.before {
        let end = all
            .iter()
            .position(|msg| msg.get("id").and_then(|id| id.as_i64()) == Some(before))
            .unwrap_or(all.len());
        (end.saturating_sub(limit), end)
    } else if let Some(after) = query.after {
        let start = all
            .iter()
            .position(|msg| msg.get("id").and_then(|id| id.as_i64()) == Some(after))
            .map(|idx| idx + 1)
            .unwrap_or(0);
        (start, (start + limit).min(all.len()))
    } else {
        (all.len().saturating_sub(limit), all.len())
    };
    let page: Vec<_> = all[start..end].to_vec();
    Json(serde_json::json!({
        "object": "list",
        "data": page,
        "total": all.len(),
        "has_older": start > 0,
        "has_newer": end < all.len()
    }))
    .into_response()
}
fn session_message_items(body: &serde_json::Value) -> Vec<serde_json::Value> {
    body.get("data")
        .or_else(|| body.get("messages"))
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default()
}

#[derive(Debug, Serialize)]
struct ContextWindowUsage {
    used: i64,
    approximate: bool,
    compressed: bool,
    compression_boundary_id: Option<serde_json::Value>,
    counted_messages: usize,
    total_messages: usize,
}

fn numeric_json_field(value: &serde_json::Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|v| i64::try_from(v).ok()))
        .or_else(|| value.as_f64().map(|v| v.round() as i64))
        .or_else(|| value.as_str().and_then(|v| v.trim().parse::<i64>().ok()))
        .filter(|v| *v > 0)
}

fn message_token_count(message: &serde_json::Value) -> Option<i64> {
    message
        .get("token_count")
        .or_else(|| message.get("tokenCount"))
        .or_else(|| message.get("tokens"))
        .and_then(numeric_json_field)
        .or_else(|| {
            message
                .get("usage")
                .and_then(|usage| usage.get("total_tokens"))
                .and_then(numeric_json_field)
        })
}

fn collect_json_text(value: &serde_json::Value, out: &mut String) {
    match value {
        serde_json::Value::String(text) => {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(text);
        }
        serde_json::Value::Array(items) => {
            for item in items {
                collect_json_text(item, out);
            }
        }
        serde_json::Value::Object(map) => {
            for key in ["text", "content", "value", "message", "summary", "reasoning"] {
                if let Some(item) = map.get(key) {
                    collect_json_text(item, out);
                }
            }
        }
        _ => {}
    }
}

fn message_context_text(message: &serde_json::Value) -> String {
    let mut text = String::new();
    if let Some(content) = message.get("content") {
        collect_json_text(content, &mut text);
    }
    if let Some(reasoning) = message.get("reasoning").or_else(|| message.get("thinking")) {
        collect_json_text(reasoning, &mut text);
    }
    text
}

fn rough_context_token_count(text: &str) -> i64 {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return 0;
    }
    let mut cjk = 0i64;
    let mut rest = 0i64;
    for ch in trimmed.chars() {
        if ('\u{3040}'..='\u{30ff}').contains(&ch)
            || ('\u{3400}'..='\u{9fff}').contains(&ch)
            || ('\u{f900}'..='\u{faff}').contains(&ch)
        {
            cjk += 1;
        } else {
            rest += 1;
        }
    }
    ((cjk as f64 * 1.5) + (rest as f64 / 4.0)).ceil().max(1.0) as i64
}

fn message_estimated_token_count(message: &serde_json::Value) -> i64 {
    rough_context_token_count(&message_context_text(message))
}

fn truthy_field(message: &serde_json::Value, keys: &[&str]) -> bool {
    keys.iter().any(|key| {
        message
            .get(*key)
            .and_then(|value| value.as_bool())
            .unwrap_or(false)
    })
}

fn is_context_compression_summary(message: &serde_json::Value) -> bool {
    if truthy_field(
        message,
        &[
            "compression_summary",
            "compressed_context",
            "context_compressed",
            "is_compression_summary",
        ],
    ) {
        return true;
    }
    let mut marker = String::new();
    for key in ["type", "event", "kind", "name", "summary_type"] {
        if let Some(text) = message.get(key).and_then(|value| value.as_str()) {
            marker.push_str(text);
            marker.push('\n');
        }
    }
    marker.push_str(&message_context_text(message));
    let lower = marker.to_lowercase();
    lower.contains("context compressed")
        || lower.contains("compressed context")
        || lower.contains("compressed conversation")
        || lower.contains("conversation summary")
        || lower.contains("compression summary")
        || lower.contains("上下文压缩")
        || lower.contains("压缩摘要")
}

fn latest_context_compression_boundary(messages: &[serde_json::Value]) -> Option<usize> {
    messages.iter().rposition(is_context_compression_summary)
}

fn estimate_context_window_usage(messages: &[serde_json::Value]) -> ContextWindowUsage {
    let boundary = latest_context_compression_boundary(messages);
    let start = boundary.unwrap_or(0);
    let counted = &messages[start..];
    let mut used = 0i64;
    let mut missing_tokens = false;
    for message in counted {
        if let Some(tokens) = message_token_count(message) {
            used += tokens;
        } else {
            missing_tokens = true;
            used += message_estimated_token_count(message);
        }
    }
    ContextWindowUsage {
        used,
        approximate: missing_tokens || boundary.is_some(),
        compressed: boundary.is_some(),
        compression_boundary_id: boundary.and_then(|idx| messages[idx].get("id").cloned()),
        counted_messages: counted.len(),
        total_messages: messages.len(),
    }
}

async fn fetch_all_session_messages_for_context(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut req = state.client.get(format!(
        "{}/api/sessions/{}/messages",
        state.api_url,
        path_segment(session_id)
    ));
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("message context request failed: {}", resp.status());
    }
    let body = resp.json::<serde_json::Value>().await?;
    Ok(session_message_items(&body))
}

async fn chat_context_window(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
) -> Response<Body> {
    match fetch_all_session_messages_for_context(&state, &session_id).await {
        Ok(messages) => Json(estimate_context_window_usage(&messages)).into_response(),
        Err(err) => json_error(
            StatusCode::BAD_GATEWAY,
            &format!("message context request failed: {err}"),
        ),
    }
}

fn session_message_id(message: &serde_json::Value) -> Option<i64> {
    message.get("id").and_then(|id| id.as_i64()).or_else(|| {
        message
            .get("id")
            .and_then(|id| id.as_str())
            .and_then(|id| id.parse::<i64>().ok())
    })
}

fn latest_session_message_id(items: &[serde_json::Value]) -> i64 {
    items.iter().filter_map(session_message_id).max().unwrap_or(0)
}

#[cfg(test)]
fn unseen_session_messages(
    items: &[serde_json::Value],
    last_id: i64,
) -> (Vec<serde_json::Value>, i64) {
    let mut pairs: Vec<(i64, serde_json::Value)> = items
        .iter()
        .filter_map(|message| session_message_id(message).map(|id| (id, message.clone())))
        .filter(|(id, _)| *id > last_id)
        .collect();
    pairs.sort_by_key(|(id, _)| *id);
    let next_last_id = pairs
        .iter()
        .map(|(id, _)| *id)
        .max()
        .unwrap_or(last_id);
    (
        pairs.into_iter().map(|(_, message)| message).collect(),
        next_last_id,
    )
}

#[derive(Clone, Debug, Default)]
struct SessionMessageWatchState {
    last_id: i64,
    fingerprints: HashMap<i64, String>,
}

fn session_message_fingerprint(message: &serde_json::Value) -> String {
    serde_json::to_string(message).unwrap_or_else(|_| message.to_string())
}

fn session_message_watch_state(items: &[serde_json::Value]) -> SessionMessageWatchState {
    let mut state = SessionMessageWatchState {
        last_id: latest_session_message_id(items),
        fingerprints: HashMap::new(),
    };
    for message in items {
        if let Some(id) = session_message_id(message) {
            state
                .fingerprints
                .insert(id, session_message_fingerprint(message));
        }
    }
    state
}

fn changed_session_messages(
    items: &[serde_json::Value],
    state: &mut SessionMessageWatchState,
) -> Vec<serde_json::Value> {
    let mut pairs: Vec<(i64, serde_json::Value, String)> = items
        .iter()
        .filter_map(|message| {
            session_message_id(message).map(|id| {
                (id, message.clone(), session_message_fingerprint(message))
            })
        })
        .collect();
    pairs.sort_by_key(|(id, _, _)| *id);
    let mut changed = Vec::new();
    for (id, message, fingerprint) in pairs {
        let is_new = id > state.last_id;
        let is_changed = state
            .fingerprints
            .get(&id)
            .map(|old| old != &fingerprint)
            .unwrap_or(false);
        if is_new || is_changed {
            changed.push(message);
        }
        state.fingerprints.insert(id, fingerprint);
        if id > state.last_id {
            state.last_id = id;
        }
    }
    changed
}

async fn fetch_session_messages_for_watch(
    client: &reqwest::Client,
    api_url: &str,
    api_key: &Option<String>,
    session_id: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut req = client.get(format!(
        "{}/api/sessions/{}/messages?limit=24",
        api_url.trim_end_matches('/'),
        path_segment(session_id)
    ));
    if let Some(key) = api_key
        && !key.is_empty()
    {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("message watch request failed: {}", resp.status());
    }
    let body = resp.json::<serde_json::Value>().await?;
    Ok(session_message_items(&body))
}

async fn chat_watch(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
) -> Sse<impl futures_core::Stream<Item = Result<SseEvent, Infallible>>> {
    let client = state.client.clone();
    let api_url = state.api_url.clone();
    let api_key = state.api_key.clone();
    let active_chat_streams = state.active_chat_streams.clone();
    let mut chat_stream_rx = state.chat_streams.subscribe();
    let stream = async_stream::stream! {
        let mut watch_state = match fetch_session_messages_for_watch(&client, &api_url, &api_key, &session_id).await {
            Ok(items) => session_message_watch_state(&items),
            Err(_) => SessionMessageWatchState::default(),
        };
        if let Some(messages) = active_chat_streams.read().await.get(&session_id).cloned() {
            for msg in messages {
                yield Ok(SseEvent::default().data(msg.to_string()));
            }
        }
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(1)) => {
                    if let Ok(items) = fetch_session_messages_for_watch(&client, &api_url, &api_key, &session_id).await {
                        for msg in changed_session_messages(&items, &mut watch_state) {
                            yield Ok(SseEvent::default().data(msg.to_string()));
                        }
                    }
                }
                Ok(text) = chat_stream_rx.recv() => {
                    if let Ok(envelope) = serde_json::from_str::<serde_json::Value>(&text)
                        && envelope.get("session_id").and_then(|value| value.as_str()) == Some(session_id.as_str())
                        && let Some(message) = envelope.get("message")
                    {
                        yield Ok(SseEvent::default().data(message.to_string()));
                    }
                }
                else => break,
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::default())
}
