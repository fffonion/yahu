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

#[derive(Deserialize)]
struct SessionRenamePayload {
    title: String,
}

fn session_title_for_lineage_index(base: &str, index: usize, is_target: bool) -> String {
    if is_target {
        base.to_string()
    } else {
        format!("{base} #{}", index + 1)
    }
}

fn session_title_base(title: &str) -> String {
    let trimmed = title.trim();
    if let Some((base, suffix)) = trimmed.rsplit_once(" #") {
        if suffix.parse::<usize>().is_ok_and(|value| value > 1) {
            return base.trim().to_string();
        }
    }
    trimmed.to_string()
}

async fn rename_session_lineage(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
    Json(payload): Json<SessionRenamePayload>,
) -> Response<Body> {
    let base_title = session_title_base(&payload.title);
    if base_title.is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "title is required");
    }
    let entries = session_rename_entries(&state, &session_id).await;
    let mut updated_ids = Vec::new();
    let mut titles = serde_json::Map::new();
    let mut requested_title = base_title.clone();
    for (index, entry) in entries.into_iter().enumerate() {
        if updated_ids.iter().any(|id: &String| id == &entry.id) {
            continue;
        }
        let is_target = entry.id == session_id;
        let title = session_title_for_lineage_index(&base_title, index, is_target);
        if is_target {
            requested_title = title.clone();
        }
        match patch_session_title(&state, &entry.id, &title).await {
            Ok(_) => {
                titles.insert(entry.id.clone(), serde_json::json!(title));
                updated_ids.push(entry.id);
            }
            Err(err) => {
                return json_error(
                    StatusCode::BAD_GATEWAY,
                    &format!("session title update failed for {}: {err}", entry.id),
                );
            }
        }
    }
    if updated_ids.is_empty() {
        match patch_session_title(&state, &session_id, &base_title).await {
            Ok(_) => {
                titles.insert(session_id.clone(), serde_json::json!(base_title));
                updated_ids.push(session_id.clone());
            }
            Err(err) => {
                return json_error(
                    StatusCode::BAD_GATEWAY,
                    &format!("session title update failed for {session_id}: {err}"),
                );
            }
        }
    }
    Json(serde_json::json!({
        "object": "yahu.session.lineage_rename",
        "id": session_id,
        "title": requested_title,
        "base_title": base_title,
        "titles": titles,
        "updated_ids": updated_ids,
    })).into_response()
}

async fn session_rename_entries(state: &AppState, session_id: &str) -> Vec<SessionLineageEntry> {
    let mut entries = session_history_entries(state, session_id).await;
    match fetch_local_rename_entries(state, session_id) {
        Ok(Some(local_entries)) if local_entries.len() > entries.len() => {
            entries = local_entries;
        }
        Ok(_) => {}
        Err(err) => warn!(session_id = %session_id, error = %err, "cannot read local rename session history metadata"),
    }
    entries
}

async fn patch_session_title(state: &AppState, session_id: &str, title: &str) -> anyhow::Result<()> {
    let mut req = state.client.patch(format!(
        "{}/api/sessions/{}",
        state.api_url.trim_end_matches('/'),
        path_segment(session_id)
    )).json(&serde_json::json!({"title": title}));
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await?;
    if resp.status().is_success() {
        Ok(())
    } else {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("{status}: {text}")
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
            rows.push(row);
            if rows.len() >= limit {
                return Ok(session_rows_with_local_previews(state, rows));
            }
        }

        let has_more = body
            .get("has_more")
            .and_then(|value| value.as_bool())
            .unwrap_or(data_len == page_size);
        offset = offset.saturating_add(page_size);
        if trimmed.is_empty() || !has_more || data_len == 0 || offset >= max_scan {
            return Ok(session_rows_with_local_previews(state, rows));
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

fn session_rows_with_local_previews(
    state: &AppState,
    mut rows: Vec<serde_json::Value>,
) -> Vec<serde_json::Value> {
    if let Err(err) = enrich_session_previews_from_local_db(state, &mut rows) {
        warn!(error = %err, "cannot enrich session list previews from local message history");
    }
    rows
}

fn enrich_session_previews_from_local_db(
    state: &AppState,
    rows: &mut [serde_json::Value],
) -> anyhow::Result<()> {
    if rows.iter().all(session_row_has_preview) {
        return Ok(());
    }
    let db_path = state.hermes_home.join("state.db");
    if !db_path.exists() {
        return Ok(());
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    if !sqlite_table_has_columns(&conn, "messages", &["session_id", "role", "content"])? {
        return Ok(());
    }
    let has_active = sqlite_table_has_columns(&conn, "messages", &["active"])?;
    for row in rows.iter_mut() {
        if session_row_has_preview(row) {
            continue;
        }
        let Some(session_id) = row.get("id").and_then(|value| value.as_str()).filter(|id| !id.is_empty()) else {
            continue;
        };
        let Some(preview) = local_latest_session_preview(&conn, session_id, has_active)? else {
            continue;
        };
        if let Some(obj) = row.as_object_mut() {
            obj.insert("preview".to_string(), serde_json::Value::String(preview));
        }
    }
    Ok(())
}

fn session_row_has_preview(row: &serde_json::Value) -> bool {
    row.get("preview")
        .and_then(|value| value.as_str())
        .is_some_and(|text| !text.trim().is_empty())
}

fn local_latest_session_preview(
    conn: &rusqlite::Connection,
    session_id: &str,
    has_active: bool,
) -> rusqlite::Result<Option<String>> {
    let sql = if has_active {
        "SELECT content FROM messages
         WHERE active = 1
           AND session_id = ?1
           AND role IN ('assistant', 'user')
           AND content IS NOT NULL
           AND trim(content) != ''
         ORDER BY id DESC
         LIMIT 20"
    } else {
        "SELECT content FROM messages
         WHERE session_id = ?1
           AND role IN ('assistant', 'user')
           AND content IS NOT NULL
           AND trim(content) != ''
         ORDER BY id DESC
         LIMIT 20"
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([session_id], |row| row.get::<_, String>(0))?;
    for row in rows {
        let preview = session_preview_from_raw_content(&row?);
        if !preview.is_empty() {
            return Ok(Some(preview));
        }
    }
    Ok(None)
}

fn session_preview_from_raw_content(raw: &str) -> String {
    fn value_text(value: &serde_json::Value) -> String {
        match value {
            serde_json::Value::String(text) => text.clone(),
            serde_json::Value::Object(map) => ["text", "content", "output", "message"]
                .iter()
                .find_map(|key| map.get(*key).map(value_text).filter(|text| !text.trim().is_empty()))
                .unwrap_or_default(),
            serde_json::Value::Array(items) => items
                .iter()
                .map(value_text)
                .find(|text| !text.trim().is_empty())
                .unwrap_or_default(),
            _ => String::new(),
        }
    }
    let value = json_or_string_field(Some(raw.to_string()));
    nav_text_excerpt(&value_text(&value), 180)
}

fn inject_turn_durations(messages: &mut Vec<serde_json::Value>) {
    let mut last_user_ts: Option<f64> = None;
    for message in messages.iter_mut() {
        let role = message
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let ts = message.get("timestamp").and_then(|v| v.as_f64());
        if role == "user" {
            last_user_ts = ts;
        } else if role == "assistant" {
            let content = message
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if content.is_empty() {
                continue;
            }
            if let Some(msg_ts) = ts {
                if let Some(prev_user_ts) = last_user_ts {
                    if msg_ts >= prev_user_ts {
                        let duration_ms = (msg_ts - prev_user_ts) * 1000.0;
                        if let Some(obj) = message.as_object_mut() {
                            if let Some(n) = serde_json::Number::from_f64(duration_ms) {
                                obj.insert("duration_ms".to_string(), serde_json::Value::Number(n));
                            }
                        }
                    }
                }
            }
        }
    }
}

fn message_i64_id(message: &serde_json::Value) -> Option<i64> {
    message.get("id").and_then(|id| id.as_i64().or_else(|| id.as_str()?.parse().ok()))
}

fn message_role(message: &serde_json::Value) -> &str {
    message.get("role").and_then(|role| role.as_str()).unwrap_or("")
}

fn message_text(message: &serde_json::Value) -> &str {
    message.get("content").and_then(|value| value.as_str()).unwrap_or("")
}

fn message_has_tool_calls(message: &serde_json::Value) -> bool {
    let Some(value) = message.get("tool_calls").or_else(|| message.get("toolCalls")) else {
        return false;
    };
    match value {
        serde_json::Value::Array(items) => !items.is_empty(),
        serde_json::Value::String(text) => {
            let trimmed = text.trim();
            !trimmed.is_empty() && trimmed != "[]" && trimmed != "null"
        }
        serde_json::Value::Null => false,
        _ => true,
    }
}

fn message_has_reasoning(message: &serde_json::Value) -> bool {
    ["reasoning", "reasoning_content", "reasoningContent"]
        .iter()
        .any(|key| message.get(*key).and_then(|value| value.as_str()).is_some_and(|text| !text.trim().is_empty()))
}

fn is_completed_final_assistant_message(message: &serde_json::Value) -> bool {
    message_role(message) == "assistant" && !message_text(message).trim().is_empty() && !message_has_tool_calls(message)
}

fn is_rootless_history_detail_candidate(message: &serde_json::Value) -> bool {
    message_role(message) == "tool" || message_has_tool_calls(message)
}

fn annotate_turn_details(final_message: &mut serde_json::Value, details: &[serde_json::Value], after_id: Option<String>) {
    if details.is_empty() {
        return;
    }
    let tool_count = details
        .iter()
        .filter(|message| message_role(message) == "tool")
        .count();
    let thinking_count = details.iter().filter(|message| message_has_reasoning(message)).count();
    let Some(before_id) = nav_message_id(final_message) else {
        return;
    };
    let mut detail = serde_json::Map::new();
    detail.insert("count".to_string(), serde_json::json!(details.len()));
    detail.insert("tool_count".to_string(), serde_json::json!(tool_count));
    detail.insert("thinking_count".to_string(), serde_json::json!(thinking_count));
    if let Some(after_id) = after_id.filter(|id| !id.is_empty()) {
        detail.insert("after_id".to_string(), serde_json::json!(after_id));
    }
    detail.insert("before_id".to_string(), serde_json::json!(before_id));
    if let Some(obj) = final_message.as_object_mut() {
        obj.insert("turn_details".to_string(), serde_json::Value::Object(detail));
    }
}

fn history_skeleton_messages(messages: &[serde_json::Value]) -> Vec<serde_json::Value> {
    let mut skeleton = Vec::new();
    let mut detail_buffer: Vec<serde_json::Value> = Vec::new();
    let mut active_anchor_id: Option<String> = None;
    let mut previous_visible_id: Option<String> = None;

    for message in messages {
        let role = message_role(message);
        if role == "user" || role == "system" {
            skeleton.append(&mut detail_buffer);
            active_anchor_id = nav_message_id(message);
            previous_visible_id = active_anchor_id.clone();
            skeleton.push(message.clone());
            continue;
        }

        if is_completed_final_assistant_message(message) {
            let mut final_message = message.clone();
            annotate_turn_details(&mut final_message, &detail_buffer, active_anchor_id.clone().or_else(|| previous_visible_id.clone()));
            detail_buffer.clear();
            active_anchor_id = None;
            previous_visible_id = nav_message_id(&final_message);
            skeleton.push(final_message);
            continue;
        }

        if active_anchor_id.is_some() || is_rootless_history_detail_candidate(message) {
            if active_anchor_id.is_none() {
                active_anchor_id = previous_visible_id.clone();
            }
            detail_buffer.push(message.clone());
            continue;
        }

        previous_visible_id = nav_message_id(message);
        skeleton.push(message.clone());
    }

    skeleton
}

fn page_bounds(messages: &[serde_json::Value], query: &ChatMessagesQuery, limit: usize) -> (usize, usize) {
    if let Some(around) = query.around {
        let center = messages
            .iter()
            .position(|msg| message_i64_id(msg) == Some(around))
            .unwrap_or_else(|| messages.len().saturating_sub(1));
        let half = limit / 2;
        let start = center.saturating_sub(half);
        let end = (start + limit).min(messages.len());
        (end.saturating_sub(limit), end)
    } else if let Some(before) = query.before {
        let end = messages
            .iter()
            .position(|msg| message_i64_id(msg) == Some(before))
            .unwrap_or(messages.len());
        (end.saturating_sub(limit), end)
    } else if let Some(after) = query.after {
        let start = messages
            .iter()
            .position(|msg| message_i64_id(msg) == Some(after))
            .map(|idx| idx + 1)
            .unwrap_or(0);
        (start, (start + limit).min(messages.len()))
    } else {
        (messages.len().saturating_sub(limit), messages.len())
    }
}

fn detail_range_messages(messages: &[serde_json::Value], query: &ChatMessagesQuery, limit: usize) -> (Vec<serde_json::Value>, bool, bool, usize) {
    let start = query
        .after
        .and_then(|after| messages.iter().position(|msg| message_i64_id(msg) == Some(after)).map(|idx| idx + 1))
        .unwrap_or(0);
    let end = query
        .before
        .and_then(|before| messages.iter().position(|msg| message_i64_id(msg) == Some(before)))
        .unwrap_or(messages.len())
        .max(start);
    let total = end.saturating_sub(start);
    let page_end = (start + limit).min(end);
    (messages[start..page_end].to_vec(), false, page_end < end, total)
}

async fn chat_messages_page(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
    Query(query): Query<ChatMessagesQuery>,
) -> Response<Body> {
    let limit = query.limit.unwrap_or(24).clamp(1, 80);
    let mut all = match fetch_session_history_messages(&state, &session_id).await {
        Ok(messages) => messages,
        Err(err) => {
            return json_error(
                StatusCode::BAD_GATEWAY,
                &format!("message list request failed: {err}"),
            );
        }
    };
    inject_turn_durations(&mut all);
    let view = query.view.as_deref().unwrap_or("full");
    let (started_at, last_active) = stitched_message_boundary_times(&all);
    if view == "details" {
        let (page, has_older, has_newer, total) = detail_range_messages(&all, &query, limit);
        return Json(serde_json::json!({
            "object": "list",
            "data": page,
            "total": total,
            "has_older": has_older,
            "has_newer": has_newer,
            "started_at": started_at,
            "last_active": last_active
        }))
        .into_response();
    }

    let page_source = if view == "skeleton" { history_skeleton_messages(&all) } else { all.clone() };
    let (start, end) = page_bounds(&page_source, &query, limit);
    let page: Vec<_> = page_source[start..end].to_vec();
    Json(serde_json::json!({
        "object": "list",
        "data": page,
        "total": all.len(),
        "has_older": start > 0,
        "has_newer": end < page_source.len(),
        "started_at": started_at,
        "last_active": last_active
    }))
    .into_response()
}

fn stitched_message_boundary_times(messages: &[serde_json::Value]) -> (Option<serde_json::Value>, Option<serde_json::Value>) {
    let started_at = messages.iter().find_map(nav_message_timestamp);
    let last_active = messages.iter().rev().find_map(nav_message_timestamp);
    (started_at, last_active)
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
    #[serde(skip_serializing_if = "Option::is_none")]
    assistant_preview: Option<String>,
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

fn nav_message_text(message: &serde_json::Value) -> String {
    let mut text = String::new();
    if let Some(content) = message.get("content") {
        collect_json_text(content, &mut text);
    }
    text
}

fn strip_platform_sender_prefix(text: &str) -> &str {
    let trimmed = text.trim_start();
    let Some(rest) = trimmed.strip_prefix('[') else {
        return trimmed;
    };
    let Some(end) = rest.find(']') else {
        return trimmed;
    };
    if rest[..end].contains('|') {
        rest[end + 1..].trim_start()
    } else {
        trimmed
    }
}

fn nav_text_excerpt(text: &str, max_chars: usize) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut excerpt = String::new();
    for ch in compact.chars().take(max_chars) {
        excerpt.push(ch);
    }
    if compact.chars().count() > excerpt.chars().count() {
        excerpt.push('…');
    }
    excerpt
}

fn nav_message_excerpt(message: &serde_json::Value) -> String {
    nav_text_excerpt(strip_platform_sender_prefix(&nav_message_text(message)), 160)
}

fn nav_assistant_preview(messages: &[serde_json::Value], user_index: usize) -> Option<String> {
    let end = messages
        .iter()
        .enumerate()
        .skip(user_index + 1)
        .find_map(|(index, message)| {
            (message.get("role").and_then(|role| role.as_str()) == Some("user")).then_some(index)
        })
        .unwrap_or(messages.len());
    messages[user_index + 1..end]
        .iter()
        .rev()
        .find_map(|message| {
            (message.get("role").and_then(|role| role.as_str()) == Some("assistant"))
                .then(|| nav_text_excerpt(&nav_message_text(message), 96))
                .filter(|text| !text.is_empty())
        })
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
                assistant_preview: nav_assistant_preview(messages, index),
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
    match fetch_session_history_messages(&state, &session_id).await {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SessionMessageJoinMode {
    ContextWindow,
    VisibleHistory,
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

fn sqlite_table_has_columns(
    conn: &rusqlite::Connection,
    table: &str,
    required: &[&str],
) -> rusqlite::Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(1))?;
    let columns = rows.collect::<Result<HashSet<_>, _>>()?;
    Ok(required.iter().all(|column| columns.contains(*column)))
}

type SessionResetLookupRow = (
    f64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

fn local_session_reset_predecessor_id(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> rusqlite::Result<Option<String>> {
    if !sqlite_table_has_columns(
        conn,
        "sessions",
        &[
            "started_at",
            "ended_at",
            "end_reason",
            "source",
            "session_key",
            "chat_id",
            "thread_id",
        ],
    )? {
        return Ok(None);
    }
    let row: Option<SessionResetLookupRow> = conn
        .query_row(
            "SELECT started_at, session_key, source, chat_id, thread_id FROM sessions WHERE id = ?1",
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((started_at, session_key, source, chat_id, thread_id)) = row else {
        return Ok(None);
    };
    if let Some(session_key) = session_key.filter(|value| !value.trim().is_empty()) {
        return conn
            .query_row(
                "SELECT id FROM sessions
                 WHERE id != ?1
                   AND end_reason IN ('session_reset', 'agent_close')
                   AND ended_at IS NOT NULL
                   AND ended_at <= ?2 + 1.0
                   AND session_key = ?3
                   AND source IS ?4
                   AND chat_id IS ?5
                   AND thread_id IS ?6
                 ORDER BY ended_at DESC
                 LIMIT 1",
                rusqlite::params![
                    session_id,
                    started_at,
                    session_key,
                    source,
                    chat_id,
                    thread_id
                ],
                |row| row.get::<_, String>(0),
            )
            .optional();
    }
    if chat_id.as_deref().unwrap_or("").trim().is_empty()
        && thread_id.as_deref().unwrap_or("").trim().is_empty()
    {
        return Ok(None);
    }
    conn.query_row(
        "SELECT id FROM sessions
         WHERE id != ?1
           AND end_reason IN ('session_reset', 'agent_close')
           AND ended_at IS NOT NULL
           AND ended_at <= ?2 + 1.0
           AND source IS ?3
           AND chat_id IS ?4
           AND thread_id IS ?5
         ORDER BY ended_at DESC
         LIMIT 1",
        rusqlite::params![session_id, started_at, source, chat_id, thread_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
}

fn local_session_reset_successor_id(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> rusqlite::Result<Option<String>> {
    if !sqlite_table_has_columns(
        conn,
        "sessions",
        &[
            "started_at",
            "ended_at",
            "end_reason",
            "source",
            "session_key",
            "chat_id",
            "thread_id",
        ],
    )? {
        return Ok(None);
    }
    let row: Option<(Option<f64>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT ended_at, end_reason, session_key, source, chat_id, thread_id FROM sessions WHERE id = ?1",
            [session_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
        )
        .optional()?;
    let Some((Some(ended_at), Some(end_reason), session_key, source, chat_id, thread_id)) = row else {
        return Ok(None);
    };
    if end_reason != "session_reset" && end_reason != "agent_close" {
        return Ok(None);
    }
    if let Some(session_key) = session_key.filter(|value| !value.trim().is_empty()) {
        return conn
            .query_row(
                "SELECT id FROM sessions
                 WHERE id != ?1
                   AND started_at >= ?2 - 1.0
                   AND session_key = ?3
                   AND source IS ?4
                   AND chat_id IS ?5
                   AND thread_id IS ?6
                 ORDER BY started_at ASC
                 LIMIT 1",
                rusqlite::params![
                    session_id,
                    ended_at,
                    session_key,
                    source,
                    chat_id,
                    thread_id
                ],
                |row| row.get::<_, String>(0),
            )
            .optional();
    }
    if chat_id.as_deref().unwrap_or("").trim().is_empty()
        && thread_id.as_deref().unwrap_or("").trim().is_empty()
    {
        return Ok(None);
    }
    conn.query_row(
        "SELECT id FROM sessions
         WHERE id != ?1
           AND started_at >= ?2 - 1.0
           AND source IS ?3
           AND chat_id IS ?4
           AND thread_id IS ?5
         ORDER BY started_at ASC
         LIMIT 1",
        rusqlite::params![session_id, ended_at, source, chat_id, thread_id],
        |row| row.get::<_, String>(0),
    )
    .optional()
}

fn local_session_rename_entries(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> rusqlite::Result<Vec<SessionLineageEntry>> {
    const MAX_SESSION_RENAME_DEPTH: usize = 100;
    let mut entries = local_session_history_entries(conn, session_id)?;
    let mut known = entries
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<HashSet<_>>();
    let mut cursor = entries.last().map(|entry| entry.id.clone());
    while let Some(current_id) = cursor.clone() {
        if entries.len() >= MAX_SESSION_RENAME_DEPTH {
            break;
        }
        let Some(successor_id) = local_session_reset_successor_id(conn, &current_id)? else {
            break;
        };
        if known.contains(&successor_id) {
            break;
        }
        let successor_entries = local_session_lineage_entries(conn, &successor_id)?;
        if successor_entries.is_empty() {
            break;
        }
        let mut appended = false;
        for entry in successor_entries {
            if known.insert(entry.id.clone()) {
                cursor = Some(entry.id.clone());
                entries.push(entry);
                appended = true;
            }
        }
        if !appended {
            break;
        }
    }
    Ok(entries)
}

fn local_session_history_entries(
    conn: &rusqlite::Connection,
    session_id: &str,
) -> rusqlite::Result<Vec<SessionLineageEntry>> {
    let entries = local_session_lineage_entries(conn, session_id)?;
    let Some(root) = entries.first() else {
        return Ok(entries);
    };
    let Some(predecessor_id) = local_session_reset_predecessor_id(conn, &root.id)? else {
        return Ok(entries);
    };
    let mut predecessor = local_session_lineage_entries(conn, &predecessor_id)?;
    let known = predecessor
        .iter()
        .map(|entry| entry.id.clone())
        .collect::<HashSet<_>>();
    predecessor.extend(entries.into_iter().filter(|entry| !known.contains(&entry.id)));
    Ok(predecessor)
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

fn nav_message_timestamp_seconds(message: &serde_json::Value) -> Option<f64> {
    ["timestamp", "created_at", "createdAt", "time"].iter().find_map(|key| {
        let value = message.get(*key)?;
        value.as_f64().or_else(|| value.as_str().and_then(|text| text.parse::<f64>().ok()))
    })
}

fn compression_carryover_prefix_len(messages: &[serde_json::Value]) -> usize {
    const CARRYOVER_BATCH_GAP_SECONDS: f64 = 2.0;
    if messages.is_empty() || nav_message_timestamp_seconds(&messages[0]).is_none() {
        return 0;
    }
    let mut batch_end = messages.len();
    let mut previous_timestamp = nav_message_timestamp_seconds(&messages[0]);
    for (index, message) in messages.iter().enumerate().skip(1) {
        let timestamp = nav_message_timestamp_seconds(message);
        if let (Some(previous), Some(current)) = (previous_timestamp, timestamp)
            && current - previous > CARRYOVER_BATCH_GAP_SECONDS
        {
            batch_end = index;
            break;
        }
        if timestamp.is_some() {
            previous_timestamp = timestamp;
        }
    }
    let leading_batch = &messages[..batch_end];
    if leading_batch
        .iter()
        .filter(|message| message.get("role").and_then(|role| role.as_str()) == Some("user"))
        .take(2)
        .count()
        < 2
    {
        return 0;
    }
    leading_batch
        .iter()
        .rposition(|message| message.get("role").and_then(|role| role.as_str()) == Some("user"))
        .map(|index| index + 1)
        .unwrap_or(0)
}

fn trim_compression_carryover_prefix(
    messages: &mut Vec<serde_json::Value>,
    previous_entry: Option<&SessionLineageEntry>,
    mode: SessionMessageJoinMode,
) {
    if mode != SessionMessageJoinMode::VisibleHistory
        || !previous_entry.is_some_and(SessionLineageEntry::compression_ended)
    {
        return;
    }
    let prefix_len = compression_carryover_prefix_len(messages).min(messages.len());
    if prefix_len > 0 {
        messages.drain(..prefix_len);
    }
}

fn messages_with_context_boundary_from_entries(
    entries: &[SessionLineageEntry],
    mode: SessionMessageJoinMode,
    mut fetch_entry_messages: impl FnMut(&str) -> anyhow::Result<Vec<serde_json::Value>>,
) -> anyhow::Result<ContextWindowMessages> {
    let mut messages = Vec::new();
    let mut seen = HashSet::new();
    let mut boundary_start = 0usize;
    let mut compression_boundary_id = None;
    for (idx, entry) in entries.iter().enumerate() {
        let mut entry_messages = fetch_entry_messages(&entry.id)?;
        trim_compression_carryover_prefix(&mut entry_messages, idx.checked_sub(1).and_then(|prev| entries.get(prev)), mode);
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

fn fetch_local_lineage_entries(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Option<Vec<SessionLineageEntry>>> {
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
    Ok(Some(entries))
}

fn fetch_local_lineage_context_messages(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Option<ContextWindowMessages>> {
    fetch_local_context_messages_with_entries(
        state,
        session_id,
        SessionMessageJoinMode::ContextWindow,
        local_session_lineage_entries,
    )
}

fn fetch_local_history_context_messages(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Option<ContextWindowMessages>> {
    fetch_local_context_messages_with_entries(
        state,
        session_id,
        SessionMessageJoinMode::VisibleHistory,
        local_session_history_entries,
    )
}

fn fetch_local_history_entries(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Option<Vec<SessionLineageEntry>>> {
    let db_path = state.hermes_home.join("state.db");
    if !db_path.exists() {
        return Ok(None);
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let entries = local_session_history_entries(&conn, session_id)?;
    if entries.is_empty() {
        return Ok(None);
    }
    Ok(Some(entries))
}

fn fetch_local_rename_entries(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Option<Vec<SessionLineageEntry>>> {
    let db_path = state.hermes_home.join("state.db");
    if !db_path.exists() {
        return Ok(None);
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let entries = local_session_rename_entries(&conn, session_id)?;
    if entries.is_empty() {
        return Ok(None);
    }
    Ok(Some(entries))
}

fn fetch_local_context_messages_with_entries(
    state: &AppState,
    session_id: &str,
    mode: SessionMessageJoinMode,
    entry_loader: fn(&rusqlite::Connection, &str) -> rusqlite::Result<Vec<SessionLineageEntry>>,
) -> anyhow::Result<Option<ContextWindowMessages>> {
    let db_path = state.hermes_home.join("state.db");
    if !db_path.exists() {
        return Ok(None);
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let entries = entry_loader(&conn, session_id)?;
    if entries.is_empty() {
        return Ok(None);
    }
    let context = messages_with_context_boundary_from_entries(&entries, mode, |entry_id| {
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
    mode: SessionMessageJoinMode,
) -> anyhow::Result<ContextWindowMessages> {
    let mut messages = Vec::new();
    let mut seen = HashSet::new();
    let mut boundary_start = 0usize;
    let mut compression_boundary_id = None;
    for (idx, entry) in entries.iter().enumerate() {
        let mut entry_messages = fetch_session_messages_by_id(state, &entry.id).await?;
        trim_compression_carryover_prefix(&mut entry_messages, idx.checked_sub(1).and_then(|prev| entries.get(prev)), mode);
        let mut entry_boundary_id = None;
        let mut saw_session_scoped_message = false;
        let mut saw_entry_message = false;
        for message in entry_messages {
            if let Some(key) = nav_message_id(&message)
                && !seen.insert(key)
            {
                continue;
            }
            let message_session_id = message.get("session_id").and_then(|value| value.as_str());
            let belongs_to_entry = message_session_id == Some(entry.id.as_str());
            saw_session_scoped_message |= message_session_id.is_some();
            saw_entry_message |= belongs_to_entry;
            if belongs_to_entry {
                entry_boundary_id = message.get("id").cloned();
            }
            messages.push(message);
        }
        if saw_session_scoped_message && !saw_entry_message {
            anyhow::bail!(
                "API messages for session {} did not include that session segment",
                entry.id
            );
        }
        if entry.compression_ended() && idx + 1 < entries.len() && entry_boundary_id.is_some() {
            boundary_start = messages.len();
            compression_boundary_id = entry_boundary_id;
        }
    }
    Ok(ContextWindowMessages { messages, boundary_start, compression_boundary_id })
}

async fn context_window_entries(state: &AppState, session_id: &str) -> Vec<SessionLineageEntry> {
    let mut entries = session_lineage_entries(state, session_id).await;
    if entries.len() <= 1 {
        match fetch_local_lineage_entries(state, session_id) {
            Ok(Some(local_entries)) if local_entries.len() > entries.len() => {
                entries = local_entries;
            }
            Ok(_) => {}
            Err(err) => warn!(session_id = %session_id, error = %err, "cannot read local session lineage metadata"),
        }
    }
    entries
}

async fn fetch_context_window_messages(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<ContextWindowMessages> {
    let entries = context_window_entries(state, session_id).await;
    match fetch_api_context_window_messages(state, &entries, SessionMessageJoinMode::ContextWindow).await {
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

async fn session_history_entries(state: &AppState, session_id: &str) -> Vec<SessionLineageEntry> {
    let mut entries = session_lineage_entries(state, session_id).await;
    match fetch_local_history_entries(state, session_id) {
        Ok(Some(local_entries)) if local_entries.len() > entries.len() => {
            entries = local_entries;
        }
        Ok(_) => {}
        Err(err) => warn!(session_id = %session_id, error = %err, "cannot read local session history metadata"),
    }
    entries
}

async fn fetch_session_history_context_messages(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<ContextWindowMessages> {
    let entries = session_history_entries(state, session_id).await;
    match fetch_api_context_window_messages(state, &entries, SessionMessageJoinMode::VisibleHistory).await {
        Ok(context) => Ok(context),
        Err(err) => {
            warn!(session_id = %session_id, error = %err, "API Server history fetch failed; falling back to local state.db");
            match fetch_local_history_context_messages(state, session_id) {
                Ok(Some(local_context)) => Ok(local_context),
                Ok(None) => anyhow::bail!("neither API Server nor local state.db has history messages for session lineage of {session_id}"),
                Err(local_err) => anyhow::bail!("both API Server and local state.db history failed: API={err}, local={local_err}"),
            }
        }
    }
}

async fn fetch_session_history_messages(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    Ok(fetch_session_history_context_messages(state, session_id).await?.messages)
}

#[cfg(test)]
async fn fetch_all_session_messages_for_context(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    fetch_session_history_messages(state, session_id).await
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
