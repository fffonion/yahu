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
async fn chat_watch(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
) -> Sse<impl futures_core::Stream<Item = Result<SseEvent, Infallible>>> {
    let client = state.client.clone();
    let api_url = state.api_url.clone();
    let api_key = state.api_key.clone();
    let stream = async_stream::stream! {
        let mut last_id: i64 = 0;
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_secs(2)) => {
                    let mut req = client.get(format!("{}/api/sessions/{}/messages?limit=8", api_url, path_segment(&session_id)));
                    if let Some(key) = &api_key {
                        if !key.is_empty() { req = req.bearer_auth(key); }
                    }
                    if let Ok(resp) = req.send().await {
                        if let Ok(body) = resp.json::<serde_json::Value>().await {
                            if let Some(items) = body.get("data").and_then(|v| v.as_array()) {
                                for msg in items.iter().rev() {
                                    if let Some(id) = msg.get("id").and_then(|id| id.as_i64()) {
                                        if id > last_id {
                                            last_id = id;
                                            yield Ok(SseEvent::default().data(msg.to_string()));
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                else => break,
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::default())
}
