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
    let messages = fetch_all_session_messages_for_context(state, session_id).await?;
    Ok(messages
        .iter()
        .any(|message| json_value_contains_query(message, &needle)))
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

fn is_user_message(message: &serde_json::Value) -> bool {
    message.get("role").and_then(|role| role.as_str()) == Some("user")
}

fn user_turn_page_start(messages: &[serde_json::Value], start: usize, end: usize) -> usize {
    if start >= end || messages.get(start).is_some_and(is_user_message) {
        return start;
    }
    if let Some(idx) = messages[..=start].iter().rposition(is_user_message) {
        return idx;
    }
    messages[start..end]
        .iter()
        .position(is_user_message)
        .map(|idx| start + idx)
        .unwrap_or(start)
}

fn has_older_user_turn(messages: &[serde_json::Value], start: usize) -> bool {
    messages[..start.min(messages.len())].iter().any(is_user_message)
}

async fn chat_messages_page(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
    Query(query): Query<ChatMessagesQuery>,
) -> Response<Body> {
    let limit = query.limit.unwrap_or(24).clamp(1, 80);
    let all = match fetch_all_session_messages_for_context(&state, &session_id).await {
        Ok(messages) => messages,
        Err(err) => {
            return json_error(
                StatusCode::BAD_GATEWAY,
                &format!("message list request failed: {err}"),
            );
        }
    };
    let (mut start, end, align_to_user_turn) = if let Some(around) = query.around {
        let center = all
            .iter()
            .position(|msg| msg.get("id").and_then(|id| id.as_i64()) == Some(around))
            .unwrap_or_else(|| all.len().saturating_sub(1));
        let half = limit / 2;
        let start = center.saturating_sub(half);
        let end = (start + limit).min(all.len());
        (end.saturating_sub(limit), end, true)
    } else if let Some(before) = query.before {
        let end = all
            .iter()
            .position(|msg| msg.get("id").and_then(|id| id.as_i64()) == Some(before))
            .unwrap_or(all.len());
        (end.saturating_sub(limit), end, true)
    } else if let Some(after) = query.after {
        let start = all
            .iter()
            .position(|msg| msg.get("id").and_then(|id| id.as_i64()) == Some(after))
            .map(|idx| idx + 1)
            .unwrap_or(0);
        (start, (start + limit).min(all.len()), false)
    } else {
        (all.len().saturating_sub(limit), all.len(), true)
    };
    if align_to_user_turn {
        start = user_turn_page_start(&all, start, end);
    }
    let page: Vec<_> = all[start..end].to_vec();
    Json(serde_json::json!({
        "object": "list",
        "data": page,
        "total": all.len(),
        "has_older": has_older_user_turn(&all, start),
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
struct UserMessageNavItem {
    id: String,
    role: &'static str,
    content: String,
    timestamp: Option<serde_json::Value>,
    position: f64,
    index: usize,
    total: usize,
}

fn nav_message_id(message: &serde_json::Value) -> Option<String> {
    message
        .get("id")
        .and_then(|id| id.as_str().map(ToString::to_string).or_else(|| id.as_i64().map(|v| v.to_string())))
}

fn nav_message_timestamp(message: &serde_json::Value) -> Option<serde_json::Value> {
    ["timestamp", "created_at", "createdAt", "time"]
        .iter()
        .find_map(|key| message.get(*key).cloned())
}

fn nav_message_excerpt(message: &serde_json::Value) -> String {
    let mut text = String::new();
    if let Some(content) = message.get("content") {
        collect_json_text(content, &mut text);
    }
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut excerpt = String::new();
    for ch in compact.chars().take(160) {
        excerpt.push(ch);
    }
    if compact.chars().count() > excerpt.chars().count() {
        excerpt.push('…');
    }
    excerpt
}

fn build_user_message_nav(messages: &[serde_json::Value]) -> Vec<UserMessageNavItem> {
    let total = messages.len();
    messages
        .iter()
        .enumerate()
        .filter_map(|(index, message)| {
            if message.get("role").and_then(|role| role.as_str()) != Some("user") {
                return None;
            }
            let id = nav_message_id(message)?;
            let denom = total.saturating_sub(1).max(1) as f64;
            Some(UserMessageNavItem {
                id,
                role: "user",
                content: nav_message_excerpt(message),
                timestamp: nav_message_timestamp(message),
                position: index as f64 / denom,
                index,
                total,
            })
        })
        .collect()
}

async fn chat_user_nav(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
) -> Response<Body> {
    match fetch_all_session_messages_for_context(&state, &session_id).await {
        Ok(messages) => Json(serde_json::json!({
            "object": "list",
            "data": build_user_message_nav(&messages),
            "total": messages.len(),
        }))
        .into_response(),
        Err(err) => json_error(
            StatusCode::BAD_GATEWAY,
            &format!("user message navigator request failed: {err}"),
        ),
    }
}

fn watch_message_window(mut items: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
    if items.len() <= API_MESSAGE_WATCH_WINDOW {
        return items;
    }
    let split_at = items.len().saturating_sub(API_MESSAGE_WATCH_WINDOW);
    items.drain(..split_at);
    items
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

#[derive(Debug)]
struct ContextWindowMessages {
    messages: Vec<serde_json::Value>,
    boundary_start: usize,
    compression_boundary_id: Option<serde_json::Value>,
}

#[derive(Clone, Debug)]
struct SessionLineageEntry {
    id: String,
    parent_session_id: Option<String>,
    end_reason: Option<String>,
}

impl SessionLineageEntry {
    fn compression_ended(&self) -> bool {
        self.end_reason.as_deref() == Some("compression")
    }
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

#[cfg(test)]
fn estimate_context_window_usage(messages: &[serde_json::Value]) -> ContextWindowUsage {
    estimate_context_window_usage_from(messages, 0, None)
}

fn estimate_context_window_usage_from(
    messages: &[serde_json::Value],
    explicit_start: usize,
    explicit_boundary_id: Option<serde_json::Value>,
) -> ContextWindowUsage {
    let summary_boundary = latest_context_compression_boundary(messages);
    let summary_start = summary_boundary.unwrap_or(0);
    let (start, boundary_id) = if explicit_start > summary_start {
        (explicit_start.min(messages.len()), explicit_boundary_id)
    } else {
        (
            summary_start,
            summary_boundary.and_then(|idx| messages[idx].get("id").cloned()),
        )
    };
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
    let compressed = boundary_id.is_some();
    ContextWindowUsage {
        used,
        approximate: missing_tokens || compressed,
        compressed,
        compression_boundary_id: boundary_id,
        counted_messages: counted.len(),
        total_messages: messages.len(),
    }
}

const HERMES_CONTENT_JSON_PREFIX: &str = "\0json:";

fn json_or_string_field(value: Option<String>) -> serde_json::Value {
    match value {
        Some(text) if text.starts_with(HERMES_CONTENT_JSON_PREFIX) => {
            serde_json::from_str(&text[HERMES_CONTENT_JSON_PREFIX.len()..])
                .unwrap_or(serde_json::Value::String(text))
        }
        Some(text) => serde_json::Value::String(text),
        None => serde_json::Value::Null,
    }
}

fn parsed_json_string_field(value: Option<String>) -> serde_json::Value {
    match value {
        Some(text) => serde_json::from_str(&text).unwrap_or(serde_json::Value::String(text)),
        None => serde_json::Value::Null,
    }
}

fn local_session_lineage_entries(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> rusqlite::Result<Vec<SessionLineageEntry>> {
    const MAX_SESSION_LINEAGE_DEPTH: usize = 100;
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    let mut current = Some(session_id.to_string());
    while let Some(id) = current {
        if !seen.insert(id.clone()) || entries.len() >= MAX_SESSION_LINEAGE_DEPTH {
            break;
        }
        let row: Option<(Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT parent_session_id, end_reason FROM sessions WHERE id = ?1",
                [&id],
                |row| Ok((row.get::<_, Option<String>>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        let Some((parent, end_reason)) = row else {
            if entries.is_empty() {
                return Ok(Vec::new());
            }
            break;
        };
        entries.push(SessionLineageEntry {
            id,
            parent_session_id: parent.clone().filter(|value| !value.trim().is_empty()),
            end_reason,
        });
        current = parent.filter(|value| !value.trim().is_empty());
    }
    entries.reverse();
    Ok(entries)
}

fn row_to_session_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<serde_json::Value> {
    let mut map = serde_json::Map::new();
    map.insert("id".to_string(), serde_json::json!(row.get::<_, i64>("id")?));
    map.insert("session_id".to_string(), serde_json::json!(row.get::<_, String>("session_id")?));
    map.insert("role".to_string(), serde_json::json!(row.get::<_, String>("role")?));
    map.insert("content".to_string(), json_or_string_field(row.get::<_, Option<String>>("content")?));
    map.insert("tool_call_id".to_string(), json_or_string_field(row.get::<_, Option<String>>("tool_call_id")?));
    map.insert("tool_calls".to_string(), parsed_json_string_field(row.get::<_, Option<String>>("tool_calls")?));
    map.insert("tool_name".to_string(), json_or_string_field(row.get::<_, Option<String>>("tool_name")?));
    map.insert("timestamp".to_string(), serde_json::json!(row.get::<_, f64>("timestamp")?));
    map.insert("token_count".to_string(), row.get::<_, Option<i64>>("token_count")?.map(serde_json::Value::from).unwrap_or(serde_json::Value::Null));
    map.insert("finish_reason".to_string(), json_or_string_field(row.get::<_, Option<String>>("finish_reason")?));
    map.insert("reasoning".to_string(), json_or_string_field(row.get::<_, Option<String>>("reasoning")?));
    map.insert("reasoning_content".to_string(), json_or_string_field(row.get::<_, Option<String>>("reasoning_content")?));
    Ok(serde_json::Value::Object(map))
}

fn messages_with_context_boundary_from_entries(
    entries: &[SessionLineageEntry],
    mut fetch_entry_messages: impl FnMut(&str) -> anyhow::Result<Vec<serde_json::Value>>,
) -> anyhow::Result<ContextWindowMessages> {
    let mut messages = Vec::new();
    let mut seen = HashSet::new();
    let mut boundary_start = 0usize;
    let mut compression_boundary_id = None;
    for (idx, entry) in entries.iter().enumerate() {
        let entry_messages = fetch_entry_messages(&entry.id)?;
        let mut entry_boundary_id = None;
        for message in entry_messages {
            if let Some(key) = nav_message_id(&message)
                && !seen.insert(key)
            {
                continue;
            }
            let belongs_to_entry = message
                .get("session_id")
                .and_then(|value| value.as_str())
                == Some(entry.id.as_str());
            if belongs_to_entry {
                entry_boundary_id = message.get("id").cloned();
            }
            messages.push(message);
        }
        if entry.compression_ended() && idx + 1 < entries.len() && entry_boundary_id.is_some() {
            boundary_start = messages.len();
            compression_boundary_id = entry_boundary_id;
        }
    }
    Ok(ContextWindowMessages { messages, boundary_start, compression_boundary_id })
}

fn fetch_local_lineage_context_messages(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Option<ContextWindowMessages>> {
    let db_path = state.hermes_home.join("state.db");
    if !db_path.exists() {
        return Ok(None);
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let entries = local_session_lineage_entries(&conn, session_id)?;
    if entries.is_empty() {
        return Ok(None);
    }
    let context = messages_with_context_boundary_from_entries(&entries, |entry_id| {
        let mut stmt = conn.prepare(
            "SELECT id, session_id, role, content, tool_call_id, tool_calls, tool_name, timestamp, token_count, finish_reason, reasoning, reasoning_content \
             FROM messages WHERE active = 1 AND session_id = ?1 ORDER BY id"
        )?;
        let rows = stmt.query_map([entry_id], row_to_session_message)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    })?;
    Ok(Some(context))
}

#[cfg(test)]
fn fetch_local_lineage_messages(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Option<Vec<serde_json::Value>>> {
    Ok(fetch_local_lineage_context_messages(state, session_id)?.map(|context| context.messages))
}

async fn fetch_session_messages_by_id(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let mut req = state.client.get(format!(
        "{}/api/sessions/{}/messages",
        state.api_url.trim_end_matches('/'),
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

fn session_info_from_detail(session_id: &str, body: &serde_json::Value) -> SessionLineageEntry {
    let session = body.get("session").unwrap_or(body);
    let parent_session_id = session
        .get("parent_session_id")
        .or_else(|| body.get("parent_session_id"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let end_reason = session
        .get("end_reason")
        .or_else(|| body.get("end_reason"))
        .and_then(|value| value.as_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    SessionLineageEntry {
        id: session
            .get("id")
            .and_then(|value| value.as_str())
            .unwrap_or(session_id)
            .to_string(),
        parent_session_id,
        end_reason,
    }
}

async fn fetch_session_info(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Option<SessionLineageEntry>> {
    let mut req = state.client.get(format!(
        "{}/api/sessions/{}",
        state.api_url.trim_end_matches('/'),
        path_segment(session_id)
    ));
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await?;
    if resp.status() == StatusCode::NOT_FOUND || resp.status() == StatusCode::METHOD_NOT_ALLOWED {
        return Ok(None);
    }
    if !resp.status().is_success() {
        anyhow::bail!("session detail request failed: {}", resp.status());
    }
    let body = resp.json::<serde_json::Value>().await?;
    Ok(Some(session_info_from_detail(session_id, &body)))
}

async fn session_lineage_entries(state: &AppState, session_id: &str) -> Vec<SessionLineageEntry> {
    const MAX_SESSION_LINEAGE_DEPTH: usize = 64;
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    let mut current = session_id.to_string();
    for _ in 0..MAX_SESSION_LINEAGE_DEPTH {
        if !seen.insert(current.clone()) {
            warn!(session_id = %current, "session parent chain cycle detected");
            break;
        }
        let info = match fetch_session_info(state, &current).await {
            Ok(Some(info)) => info,
            Ok(None) => SessionLineageEntry {
                id: current.clone(),
                parent_session_id: None,
                end_reason: None,
            },
            Err(err) => {
                warn!(session_id = %current, error = %err, "cannot read session detail; using available session messages");
                SessionLineageEntry {
                    id: current.clone(),
                    parent_session_id: None,
                    end_reason: None,
                }
            }
        };
        let parent = info.parent_session_id.clone();
        entries.push(info);
        let Some(parent) = parent else {
            break;
        };
        current = parent;
    }
    entries.reverse();
    entries
}

async fn fetch_api_context_window_messages(
    state: &AppState,
    entries: &[SessionLineageEntry],
) -> anyhow::Result<ContextWindowMessages> {
    let mut messages = Vec::new();
    let mut seen = HashSet::new();
    let mut boundary_start = 0usize;
    let mut compression_boundary_id = None;
    for (idx, entry) in entries.iter().enumerate() {
        let entry_messages = fetch_session_messages_by_id(state, &entry.id).await?;
        let mut entry_boundary_id = None;
        for message in entry_messages {
            if let Some(key) = nav_message_id(&message)
                && !seen.insert(key)
            {
                continue;
            }
            let belongs_to_entry = message
                .get("session_id")
                .and_then(|value| value.as_str())
                == Some(entry.id.as_str());
            if belongs_to_entry {
                entry_boundary_id = message.get("id").cloned();
            }
            messages.push(message);
        }
        if entry.compression_ended() && idx + 1 < entries.len() && entry_boundary_id.is_some() {
            boundary_start = messages.len();
            compression_boundary_id = entry_boundary_id;
        }
    }
    Ok(ContextWindowMessages { messages, boundary_start, compression_boundary_id })
}

async fn fetch_context_window_messages(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<ContextWindowMessages> {
    let entries = session_lineage_entries(state, session_id).await;
    match fetch_api_context_window_messages(state, &entries).await {
        Ok(context) => Ok(context),
        Err(err) => {
            warn!(session_id = %session_id, error = %err, "API Server fetch failed; falling back to local state.db");
            match fetch_local_lineage_context_messages(state, session_id) {
                Ok(Some(local_context)) => Ok(local_context),
                Ok(None) => anyhow::bail!("neither API Server nor local state.db has context messages for session lineage of {session_id}"),
                Err(local_err) => anyhow::bail!("both API Server and local state.db failed: API={err}, local={local_err}"),
            }
        }
    }
}

async fn fetch_all_session_messages_for_context(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    Ok(fetch_context_window_messages(state, session_id).await?.messages)
}

async fn chat_context_window(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
) -> Response<Body> {
    match fetch_context_window_messages(&state, &session_id).await {
        Ok(context) => Json(estimate_context_window_usage_from(
            &context.messages,
            context.boundary_start,
            context.compression_boundary_id,
        ))
        .into_response(),
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
    Ok(watch_message_window(session_message_items(&body)))
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
