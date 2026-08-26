const API_SESSION_PAGE_SIZE: usize = 200;
const API_SESSION_SEARCH_SCAN_LIMIT: usize = 2_000;
const API_SESSION_SOURCE_FILTER_SCAN_LIMIT: usize = 10_000;
const API_SESSION_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PINNED_SESSION_IDS: usize = 100;

async fn sessions_search(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SessionSearchQuery>,
) -> Response<Body> {
    let limit = query.limit.unwrap_or(80).clamp(1, 100);
    let q = query.q.unwrap_or_default();
    let hide_cron_cli = query.hide_cron_cli.unwrap_or(false);
    let pinned_ids = parse_pinned_session_ids(query.pinned_ids.as_deref());
    let data = if hide_cron_cli && q.trim().is_empty() {
        match fetch_filtered_sidebar_sessions_from_local_db(&state, limit) {
            Ok(Some(rows)) => Ok(rows),
            Ok(None) => fetch_sessions_from_api_server(&state, &q, limit, true).await,
            Err(err) => {
                warn!(error = %err, "cannot read filtered sidebar sessions from local metadata");
                fetch_sessions_from_api_server(&state, &q, limit, true).await
            }
        }
    } else {
        fetch_sessions_from_api_server(&state, &q, limit, hide_cron_cli).await
    };
    match data {
        Ok(data) => {
            let data = append_pinned_session_rows(&state, data, &pinned_ids).await;
            Json(serde_json::json!({
                "object": "list",
                "data": data,
                "limit": limit,
                "q": q,
            }))
            .into_response()
        }
        Err(err) => json_error(
            StatusCode::BAD_GATEWAY,
            &format!("session search API request failed: {err}"),
        ),
    }
}

async fn session_reasoning_effort(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
) -> Response<Body> {
    let db_path = state.hermes_home.join("state.db");
    if !db_path.exists() {
        return Json(serde_json::json!({ "reasoning_effort": null })).into_response();
    }
    let conn = match rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(conn) => conn,
        Err(err) => {
            warn!(session_id = %session_id, error = %err, "cannot read session reasoning metadata");
            return Json(serde_json::json!({ "reasoning_effort": null })).into_response();
        }
    };
    let model_config = match conn
        .query_row(
            "SELECT model_config FROM sessions WHERE id = ?1",
            [&session_id],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()
    {
        Ok(value) => value,
        Err(err) => {
            warn!(session_id = %session_id, error = %err, "cannot query session reasoning metadata");
            None
        }
    };
    let effort = model_config
        .flatten()
        .as_deref()
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
        .and_then(|value| {
            value
                .pointer("/reasoning_config/effort")
                .or_else(|| value.get("reasoning_effort"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        });
    Json(serde_json::json!({ "reasoning_effort": effort })).into_response()
}

fn parse_pinned_session_ids(value: Option<&str>) -> Vec<String> {
    let mut seen = HashSet::new();
    value
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|id| !id.is_empty())
        .filter(|id| seen.insert((*id).to_string()))
        .take(MAX_PINNED_SESSION_IDS)
        .map(str::to_string)
        .collect()
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
    if let Some((base, _)) = trimmed
        .rsplit_once(" #")
        .filter(|(_, suffix)| suffix.parse::<usize>().is_ok_and(|value| value > 1))
    {
        return base.trim().to_string();
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
    let mut assigned_titles = HashSet::new();
    for (index, entry) in entries.into_iter().enumerate() {
        if updated_ids.iter().any(|id: &String| id == &entry.id) {
            continue;
        }
        let is_target = entry.id == session_id;
        let preferred_title = session_title_for_lineage_index(&base_title, index, is_target);
        match patch_session_title_avoiding_conflicts(&state, &entry.id, &base_title, &preferred_title, &mut assigned_titles).await {
            Ok(title) => {
                if is_target {
                    requested_title = title.clone();
                }
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

fn is_session_title_conflict_error(err: &anyhow::Error) -> bool {
    let text = err.to_string();
    text.contains("invalid_title") || text.contains("already in use")
}

fn lineage_title_suffix_start(base: &str, title: &str) -> usize {
    if title == base {
        return 1;
    }
    title
        .strip_prefix(&format!("{base} #"))
        .and_then(|suffix| suffix.parse::<usize>().ok())
        .map(|value| value.saturating_add(1))
        .unwrap_or(1)
}

fn next_lineage_title_candidate(base: &str, next_suffix: &mut usize, assigned_titles: &HashSet<String>) -> String {
    loop {
        let candidate = format!("{base} #{}", *next_suffix);
        *next_suffix = next_suffix.saturating_add(1);
        if !assigned_titles.contains(&candidate.to_lowercase()) {
            return candidate;
        }
    }
}

async fn patch_session_title_avoiding_conflicts(
    state: &AppState,
    session_id: &str,
    base: &str,
    preferred_title: &str,
    assigned_titles: &mut HashSet<String>,
) -> anyhow::Result<String> {
    let mut title = preferred_title.to_string();
    let mut next_suffix = lineage_title_suffix_start(base, preferred_title);
    for _ in 0..200 {
        if assigned_titles.contains(&title.to_lowercase()) {
            title = next_lineage_title_candidate(base, &mut next_suffix, assigned_titles);
            continue;
        }
        match patch_session_title(state, session_id, &title).await {
            Ok(()) => {
                assigned_titles.insert(title.to_lowercase());
                return Ok(title);
            }
            Err(err) if is_session_title_conflict_error(&err) => {
                assigned_titles.insert(title.to_lowercase());
                title = next_lineage_title_candidate(base, &mut next_suffix, assigned_titles);
            }
            Err(err) => return Err(err),
        }
    }
    anyhow::bail!("no available title found for base '{base}'")
}

async fn fetch_sessions_from_api_server(
    state: &AppState,
    query: &str,
    limit: usize,
    hide_cron_cli: bool,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let trimmed = query.trim();
    let mut offset = 0usize;
    let mut rows = Vec::new();
    let page_size = if trimmed.is_empty() && !hide_cron_cli {
        limit.saturating_mul(2).clamp(1, API_SESSION_PAGE_SIZE)
    } else {
        API_SESSION_PAGE_SIZE
    };
    let max_scan = if hide_cron_cli {
        API_SESSION_SOURCE_FILTER_SCAN_LIMIT.max(limit)
    } else if trimmed.is_empty() {
        API_SESSION_PAGE_SIZE
    } else {
        API_SESSION_SEARCH_SCAN_LIMIT.max(limit)
    };

    loop {
        let batch_width = if hide_cron_cli && offset > 0 { 4 } else { 1 };
        let offsets = (0..batch_width)
            .map(|index| offset.saturating_add(index * page_size))
            .take_while(|page_offset| *page_offset < max_scan)
            .collect::<Vec<_>>();
        if offsets.is_empty() {
            let mut visible_rows = session_rows_with_local_previews(state, rows);
            visible_rows.truncate(limit);
            return Ok(visible_rows);
        }
        let pages = futures_util::future::join_all(offsets.iter().map(|page_offset| {
            fetch_api_session_page(state, page_size, *page_offset, trimmed)
        }))
        .await;

        for (page_offset, page) in offsets.into_iter().zip(pages) {
            let page = page?;
            let data_len = page.data.len();
            for row in page.data {
                if is_client_visible_session(&row, hide_cron_cli) {
                    rows.push(row);
                }
            }
            let mut visible_rows = session_rows_with_local_previews(state, rows.clone());
            if visible_rows.len() >= limit {
                visible_rows.truncate(limit);
                return Ok(visible_rows);
            }

            offset = page_offset.saturating_add(page_size);
            if (trimmed.is_empty() && !hide_cron_cli)
                || !page.has_more
                || data_len == 0
                || offset >= max_scan
            {
                visible_rows.truncate(limit);
                return Ok(visible_rows);
            }
        }
    }
}

fn fetch_filtered_sidebar_sessions_from_local_db(
    state: &AppState,
    limit: usize,
) -> anyhow::Result<Option<Vec<serde_json::Value>>> {
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
        "sessions",
        &[
            "id",
            "source",
            "model",
            "model_config",
            "billing_provider",
            "parent_session_id",
            "started_at",
            "ended_at",
            "end_reason",
            "message_count",
            "title",
            "archived",
            "session_key",
            "chat_id",
            "thread_id",
        ],
    )? {
        return Ok(None);
    }
    let mut statement = conn.prepare(
        "SELECT id, source, model, model_config, billing_provider,
                started_at, ended_at, message_count, title
         FROM sessions
         WHERE parent_session_id IS NULL
           AND COALESCE(archived, 0) = 0
           AND (source IS NULL OR source NOT IN ('tool', 'cron', 'cli', 'alp-worker'))
         ORDER BY started_at DESC
         LIMIT ?1",
    )?;
    let mapped = statement.query_map([API_SESSION_SOURCE_FILTER_SCAN_LIMIT as i64], |row| {
        let model_config: Option<String> = row.get(3)?;
        let billing_provider: Option<String> = row.get(4)?;
        let provider = model_config
            .as_deref()
            .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
            .and_then(|value| {
                value
                    .pointer("/gateway_runtime/provider")
                    .and_then(|provider| provider.as_str())
                    .map(str::to_string)
            })
            .or_else(|| billing_provider.filter(|value| !value.trim().is_empty()));
        Ok(serde_json::json!({
            "id": row.get::<_, String>(0)?,
            "source": row.get::<_, Option<String>>(1)?,
            "model": row.get::<_, Option<String>>(2)?,
            "provider": provider,
            "started_at": row.get::<_, Option<f64>>(5)?,
            "ended_at": row.get::<_, Option<f64>>(6)?,
            "message_count": row.get::<_, Option<i64>>(7)?.unwrap_or_default(),
            "title": row.get::<_, Option<String>>(8)?,
        }))
    })?;
    let mut rows = mapped.collect::<rusqlite::Result<Vec<_>>>()?;
    if let Err(err) = filter_session_rows_shadowed_by_local_successors(state, &mut rows) {
        warn!(error = %err, "cannot filter stitched predecessor sessions from local metadata");
    }
    rows.truncate(limit);
    if let Err(err) = enrich_session_previews_from_local_db(state, &mut rows) {
        warn!(error = %err, "cannot enrich filtered session list previews from local message history");
    }
    sanitize_session_row_previews(&mut rows);
    Ok(Some(rows))
}

async fn append_pinned_session_rows(
    state: &AppState,
    mut rows: Vec<serde_json::Value>,
    pinned_ids: &[String],
) -> Vec<serde_json::Value> {
    if pinned_ids.is_empty() {
        return rows;
    }
    let existing_ids = rows
        .iter()
        .filter_map(|row| row.get("id").and_then(|value| value.as_str()))
        .collect::<HashSet<_>>();
    let missing_ids = pinned_ids
        .iter()
        .filter(|id| !existing_ids.contains(id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    if missing_ids.is_empty() {
        return rows;
    }

    let mut pinned_by_id = HashMap::new();
    match fetch_pinned_session_rows_from_local_db(state, &missing_ids) {
        Ok(Some(local_rows)) => {
            for row in local_rows {
                if let Some(id) = row.get("id").and_then(|value| value.as_str()).map(str::to_string) {
                    pinned_by_id.insert(id, row);
                }
            }
        }
        Ok(None) => {}
        Err(err) => warn!(error = %err, "cannot read pinned sessions from local metadata"),
    }

    let unresolved_ids = missing_ids
        .iter()
        .filter(|id| !pinned_by_id.contains_key(id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let fetched = futures_util::future::join_all(
        unresolved_ids
            .iter()
            .map(|session_id| fetch_api_pinned_session(state, session_id)),
    )
    .await;
    for (session_id, result) in unresolved_ids.iter().zip(fetched) {
        match result {
            Ok(Some(row)) => {
                pinned_by_id.insert(session_id.clone(), row);
            }
            Ok(None) => {}
            Err(err) => warn!(session_id = %session_id, error = %err, "cannot fetch pinned session metadata"),
        }
    }

    let mut pinned_rows = missing_ids
        .iter()
        .filter_map(|id| pinned_by_id.remove(id))
        .collect::<Vec<_>>();
    if let Err(err) = enrich_session_previews_from_local_db(state, &mut pinned_rows) {
        warn!(error = %err, "cannot enrich pinned session previews from local message history");
    }
    sanitize_session_row_previews(&mut pinned_rows);
    rows.extend(pinned_rows);
    rows
}

fn fetch_pinned_session_rows_from_local_db(
    state: &AppState,
    pinned_ids: &[String],
) -> anyhow::Result<Option<Vec<serde_json::Value>>> {
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
        "sessions",
        &[
            "id",
            "source",
            "model",
            "model_config",
            "billing_provider",
            "parent_session_id",
            "started_at",
            "ended_at",
            "message_count",
            "title",
            "archived",
        ],
    )? {
        return Ok(None);
    }
    let mut statement = conn.prepare(
        "SELECT id, source, model, model_config, billing_provider,
                started_at, ended_at, message_count, title
         FROM sessions
         WHERE id = ?1
           AND parent_session_id IS NULL
           AND COALESCE(archived, 0) = 0
         LIMIT 1",
    )?;
    let mut rows = Vec::new();
    for session_id in pinned_ids {
        let row = statement
            .query_row([session_id], |row| {
                let model_config: Option<String> = row.get(3)?;
                let billing_provider: Option<String> = row.get(4)?;
                let provider = model_config
                    .as_deref()
                    .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
                    .and_then(|value| {
                        value
                            .pointer("/gateway_runtime/provider")
                            .and_then(|provider| provider.as_str())
                            .map(str::to_string)
                    })
                    .or_else(|| billing_provider.filter(|value| !value.trim().is_empty()));
                Ok(serde_json::json!({
                    "id": row.get::<_, String>(0)?,
                    "source": row.get::<_, Option<String>>(1)?,
                    "model": row.get::<_, Option<String>>(2)?,
                    "provider": provider,
                    "started_at": row.get::<_, Option<f64>>(5)?,
                    "ended_at": row.get::<_, Option<f64>>(6)?,
                    "message_count": row.get::<_, Option<i64>>(7)?.unwrap_or_default(),
                    "title": row.get::<_, Option<String>>(8)?,
                }))
            })
            .optional()?;
        if let Some(row) = row {
            rows.push(row);
        }
    }
    Ok(Some(rows))
}

async fn fetch_api_pinned_session(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Option<serde_json::Value>> {
    let url = format!(
        "{}/api/sessions/{}",
        state.api_url.trim_end_matches('/'),
        path_segment(session_id),
    );
    let mut req = state.client.get(url);
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        req = req.bearer_auth(key);
    }
    let resp = timeout(API_SESSION_REQUEST_TIMEOUT, req.send()).await??;
    if resp.status() == StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !resp.status().is_success() {
        anyhow::bail!("pinned session detail request failed: {}", resp.status());
    }
    let body = resp.json::<serde_json::Value>().await?;
    let row = body
        .get("data")
        .or_else(|| body.get("session"))
        .cloned()
        .unwrap_or(body);
    if row.get("id").and_then(|value| value.as_str()) != Some(session_id)
        || !is_client_visible_session(&row, false)
    {
        return Ok(None);
    }
    Ok(Some(row))
}

struct ApiSessionPage {
    data: Vec<serde_json::Value>,
    has_more: bool,
}

async fn fetch_api_session_page(
    state: &AppState,
    page_size: usize,
    offset: usize,
    query: &str,
) -> anyhow::Result<ApiSessionPage> {
    let url = api_sessions_url(&state.api_url, page_size, offset, query)?;
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
    let has_more = body
        .get("has_more")
        .and_then(|value| value.as_bool())
        .unwrap_or(data.len() == page_size);
    Ok(ApiSessionPage { data, has_more })
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
        ("exclude_sources", "tool".to_string()),
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

fn is_client_visible_session(row: &serde_json::Value, hide_cron_cli: bool) -> bool {
    row.get("source")
        .and_then(|value| value.as_str())
        .map(|source| {
            source != "tool"
                && (!hide_cron_cli || !matches!(source, "cron" | "cli" | "alp-worker"))
        })
        .unwrap_or(true)
}

fn session_rows_with_local_previews(
    state: &AppState,
    mut rows: Vec<serde_json::Value>,
) -> Vec<serde_json::Value> {
    if let Err(err) = filter_session_rows_shadowed_by_local_successors(state, &mut rows) {
        warn!(error = %err, "cannot filter stitched predecessor sessions from local metadata");
    }
    if let Err(err) = enrich_session_previews_from_local_db(state, &mut rows) {
        warn!(error = %err, "cannot enrich session list previews from local message history");
    }
    sanitize_session_row_previews(&mut rows);
    rows
}

fn sanitize_session_row_previews(rows: &mut [serde_json::Value]) {
    for row in rows {
        let Some(preview) = row
            .get("preview")
            .and_then(|value| value.as_str())
            .map(str::to_string)
        else {
            continue;
        };
        let cleaned = session_preview_from_raw_content(&preview);
        if cleaned != preview
            && let Some(obj) = row.as_object_mut()
        {
            obj.insert("preview".to_string(), serde_json::Value::String(cleaned));
        }
    }
}

fn filter_session_rows_shadowed_by_local_successors(
    state: &AppState,
    rows: &mut Vec<serde_json::Value>,
) -> anyhow::Result<()> {
    if rows.len() <= 1 {
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
    if !sqlite_table_has_columns(
        &conn,
        "sessions",
        &[
            "id",
            "parent_session_id",
            "started_at",
            "ended_at",
            "end_reason",
            "source",
            "session_key",
            "chat_id",
            "thread_id",
        ],
    )? {
        return Ok(());
    }
    let row_ids = rows
        .iter()
        .filter_map(|row| row.get("id").and_then(|value| value.as_str()).filter(|id| !id.is_empty()))
        .map(str::to_string)
        .collect::<HashSet<_>>();
    let mut statement = conn.prepare(
        "SELECT id, started_at, ended_at, end_reason, session_key, source, chat_id, thread_id
         FROM sessions
         ORDER BY started_at ASC",
    )?;
    let metadata = statement
        .query_map([], |row| {
            Ok(LocalSessionListMetadata {
                id: row.get(0)?,
                started_at: row.get(1)?,
                ended_at: row.get(2)?,
                end_reason: row.get(3)?,
                session_key: row.get(4)?,
                source: row.get(5)?,
                chat_id: row.get(6)?,
                thread_id: row.get(7)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let metadata_by_id = metadata
        .iter()
        .enumerate()
        .map(|(index, session)| (session.id.as_str(), index))
        .collect::<HashMap<_, _>>();
    let hidden = row_ids
        .iter()
        .filter(|session_id| {
            let Some(index) = metadata_by_id.get(session_id.as_str()) else {
                return false;
            };
            local_session_reset_successor_from_metadata(&metadata[*index], &metadata)
                .is_some_and(|successor_id| row_ids.contains(successor_id))
        })
        .cloned()
        .collect::<HashSet<_>>();
    if hidden.is_empty() {
        return Ok(());
    }
    rows.retain(|row| row.get("id").and_then(|value| value.as_str()).is_none_or(|id| !hidden.contains(id)));
    Ok(())
}

struct LocalSessionListMetadata {
    id: String,
    started_at: f64,
    ended_at: Option<f64>,
    end_reason: Option<String>,
    session_key: Option<String>,
    source: Option<String>,
    chat_id: Option<String>,
    thread_id: Option<String>,
}

fn local_session_reset_successor_from_metadata<'a>(
    current: &LocalSessionListMetadata,
    sessions: &'a [LocalSessionListMetadata],
) -> Option<&'a str> {
    let ended_at = current.ended_at?;
    if !matches!(current.end_reason.as_deref(), Some("session_reset" | "agent_close")) {
        return None;
    }
    if let Some(session_key) = current.session_key.as_deref().filter(|value| !value.trim().is_empty()) {
        return sessions
            .iter()
            .find(|candidate| {
                candidate.id != current.id
                    && candidate.started_at >= ended_at - 1.0
                    && candidate.session_key.as_deref() == Some(session_key)
                    && candidate.source == current.source
                    && candidate.chat_id == current.chat_id
                    && candidate.thread_id == current.thread_id
            })
            .map(|candidate| candidate.id.as_str());
    }
    if current.chat_id.as_deref().unwrap_or("").trim().is_empty()
        && current.thread_id.as_deref().unwrap_or("").trim().is_empty()
    {
        let mut candidates = sessions.iter().filter(|candidate| {
            candidate.id != current.id
                && candidate.started_at >= ended_at - 0.1
                && candidate.started_at <= ended_at + 2.0
                && candidate.source == current.source
        });
        let candidate = candidates.next()?;
        return candidates.next().is_none().then_some(candidate.id.as_str());
    }
    sessions
        .iter()
        .find(|candidate| {
            candidate.id != current.id
                && candidate.started_at >= ended_at - 1.0
                && candidate.source == current.source
                && candidate.chat_id == current.chat_id
                && candidate.thread_id == current.thread_id
        })
        .map(|candidate| candidate.id.as_str())
}

fn enrich_session_previews_from_local_db(
    state: &AppState,
    rows: &mut [serde_json::Value],
) -> anyhow::Result<()> {
    let missing_ids = rows
        .iter()
        .filter(|row| !session_row_has_preview(row))
        .filter_map(|row| row.get("id").and_then(|value| value.as_str()).filter(|id| !id.is_empty()))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if missing_ids.is_empty() {
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
    if !sqlite_table_has_columns(&conn, "messages", &["id", "session_id", "role", "content"])? {
        return Ok(());
    }
    let has_active = sqlite_table_has_columns(&conn, "messages", &["active"])?;
    let has_tool_calls = sqlite_table_has_columns(&conn, "messages", &["tool_calls"])?;
    let has_finish_reason = sqlite_table_has_columns(&conn, "messages", &["finish_reason"])?;
    let preview_role_clause = match (has_tool_calls, has_finish_reason) {
        (true, true) => "(role = 'user' OR (role = 'assistant' AND (tool_calls IS NULL OR trim(tool_calls) IN ('', '[]', '{}', 'null')) AND (finish_reason IS NULL OR lower(trim(finish_reason)) NOT IN ('tool_calls', 'function_call'))))",
        (true, false) => "(role = 'user' OR (role = 'assistant' AND (tool_calls IS NULL OR trim(tool_calls) IN ('', '[]', '{}', 'null'))))",
        (false, true) => "(role = 'user' OR (role = 'assistant' AND (finish_reason IS NULL OR lower(trim(finish_reason)) NOT IN ('tool_calls', 'function_call'))))",
        (false, false) => "role IN ('assistant', 'user')",
    };
    let mut previews = HashMap::new();
    for id_chunk in missing_ids.chunks(200) {
        let placeholders = std::iter::repeat_n("?", id_chunk.len()).collect::<Vec<_>>().join(",");
        let active_clause = if has_active { "active = 1 AND" } else { "" };
        let sql = format!(
            "SELECT session_id, content
             FROM (
                 SELECT session_id, content,
                        ROW_NUMBER() OVER (PARTITION BY session_id ORDER BY id DESC) AS preview_rank
                 FROM messages
                 WHERE {active_clause}
                       session_id IN ({placeholders})
                   AND ({preview_role_clause})
                   AND content IS NOT NULL
                   AND trim(content) != ''
             )
             WHERE preview_rank <= 20
             ORDER BY preview_rank ASC"
        );
        let mut statement = conn.prepare(&sql)?;
        let candidates = statement.query_map(rusqlite::params_from_iter(id_chunk.iter()), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for candidate in candidates {
            let (session_id, content) = candidate?;
            if previews.contains_key(&session_id) {
                continue;
            }
            let preview = session_preview_from_raw_content(&content);
            if !preview.is_empty() {
                previews.insert(session_id, preview);
            }
        }
    }
    for row in rows.iter_mut() {
        if session_row_has_preview(row) {
            continue;
        }
        let session_id = row
            .get("id")
            .and_then(|value| value.as_str())
            .filter(|id| !id.is_empty())
            .map(str::to_string);
        let Some(preview) = session_id.as_ref().and_then(|id| previews.get(id)).cloned() else {
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

fn strip_gateway_sender_prefix(text: &str) -> &str {
    let Some(line_end) = text.find('\n') else {
        return text;
    };
    let first_line = text[..line_end].trim_end_matches('\r');
    if !first_line.starts_with('[') || !first_line.ends_with(']') {
        return text;
    }
    let inner = &first_line[1..first_line.len() - 1];
    let Some((name, marker)) = inner.split_once('|') else {
        return text;
    };
    if name.trim().is_empty() || marker.trim().is_empty() {
        return text;
    }
    &text[line_end + 1..]
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
    let text = value_text(&value);
    nav_text_excerpt(strip_gateway_sender_prefix(&text), 180)
}

const MIN_HISTORICAL_TURN_DURATION_MS: f64 = 1000.0;

fn inject_turn_durations(messages: &mut [serde_json::Value]) {
    let mut last_user_ts: Option<f64> = None;
    for message in messages.iter_mut() {
        if message.get("history_gap").is_some() {
            last_user_ts = None;
            continue;
        }
        let role = message
            .get("role")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let ts = message.get("timestamp").and_then(|v| v.as_f64());
        if role == "user" {
            last_user_ts = ts;
            continue;
        }
        if role != "assistant" || message.get("content").and_then(|v| v.as_str()).unwrap_or("").is_empty() {
            continue;
        }
        let (Some(msg_ts), Some(prev_user_ts)) = (ts, last_user_ts) else {
            continue;
        };
        let duration_ms = (msg_ts - prev_user_ts) * 1000.0;
        if msg_ts < prev_user_ts || duration_ms < MIN_HISTORICAL_TURN_DURATION_MS {
            continue;
        }
        if let (Some(obj), Some(number)) = (
            message.as_object_mut(),
            serde_json::Number::from_f64(duration_ms),
        ) {
            obj.insert("duration_ms".to_string(), serde_json::Value::Number(number));
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
    [
        "reasoning",
        "reasoning_content",
        "reasoningContent",
        "reasoning_summary",
        "reasoningSummary",
    ]
    .iter()
    .filter_map(|key| message.get(*key))
    .any(|value| {
        let mut text = String::new();
        collect_json_text(value, &mut text);
        !text.trim().is_empty()
    })
}

fn is_completed_final_assistant_message(message: &serde_json::Value) -> bool {
    message_role(message) == "assistant" && ( !message_text(message).trim().is_empty() || message_has_reasoning(message)) && !message_has_tool_calls(message)
}

fn is_rootless_history_detail_candidate(message: &serde_json::Value) -> bool {
    message_role(message) == "tool" || message_has_tool_calls(message)
}

fn append_message_reasoning(message: &serde_json::Value, parts: &mut Vec<String>) {
    if let Some(value) = message.get("reasoning") {
        let mut text = String::new();
        collect_json_text(value, &mut text);
        if !text.trim().is_empty() {
            push_unique_reasoning(parts, &text);
            return;
        }
    }
    for key in ["reasoning_content", "reasoningContent", "reasoning_summary", "reasoningSummary"] {
        if let Some(value) = message.get(key) {
            let mut text = String::new();
            collect_json_text(value, &mut text);
            push_unique_reasoning(parts, &text);
        }
    }
}

fn is_visible_tool_commentary(message: &serde_json::Value) -> bool {
    message_role(message) == "assistant" && !message_text(message).trim().is_empty() && message_has_tool_calls(message)
}

fn turn_commentary_message(message: &serde_json::Value) -> serde_json::Value {
    let mut commentary = serde_json::Map::new();
    for key in ["id", "session_id", "role", "content", "timestamp", "model", "provider"] {
        if let Some(value) = message.get(key) {
            commentary.insert(key.to_string(), value.clone());
        }
    }
    serde_json::Value::Object(commentary)
}

fn turn_detail_segment(
    hidden: &[&serde_json::Value],
    after_id: &str,
    before_id: &str,
) -> serde_json::Value {
    let tool_count = hidden
        .iter()
        .filter(|message| message_role(message) == "tool")
        .count();
    let thinking_count = hidden
        .iter()
        .filter(|message| message_has_reasoning(message))
        .count();
    serde_json::json!({
        "kind": "detail",
        "count": hidden.len(),
        "tool_count": tool_count,
        "thinking_count": thinking_count,
        "after_id": after_id,
        "before_id": before_id,
    })
}

fn turn_detail_timeline(
    details: &[serde_json::Value],
    after_id: &str,
    before_id: &str,
) -> Option<Vec<serde_json::Value>> {
    let mut timeline = Vec::new();
    let mut hidden = Vec::new();
    let mut segment_after = after_id.to_string();
    let mut saw_commentary = false;

    for message in details {
        if is_visible_tool_commentary(message) {
            let commentary_id = nav_message_id(message)?;
            if !hidden.is_empty() {
                timeline.push(turn_detail_segment(&hidden, &segment_after, &commentary_id));
                hidden.clear();
            }
            timeline.push(serde_json::json!({
                "kind": "commentary",
                "message": turn_commentary_message(message),
            }));
            segment_after = commentary_id;
            saw_commentary = true;
        } else {
            hidden.push(message);
        }
    }

    if !hidden.is_empty() {
        timeline.push(turn_detail_segment(&hidden, &segment_after, before_id));
    }
    saw_commentary.then_some(timeline)
}

fn annotate_turn_details(final_message: &mut serde_json::Value, details: &[serde_json::Value], after_id: Option<String>) {
    if details.is_empty() {
        return;
    }
    let hidden_details: Vec<_> = details.iter().filter(|message| !is_visible_tool_commentary(message)).collect();
    let commentary: Vec<_> = details
        .iter()
        .filter(|message| is_visible_tool_commentary(message))
        .map(turn_commentary_message)
        .collect();
    let tool_count = hidden_details
        .iter()
        .filter(|message| message_role(message) == "tool")
        .count();
    let thinking_count = hidden_details
        .iter()
        .filter(|message| message_has_reasoning(message))
        .count();
    let Some(before_id) = nav_message_id(final_message) else {
        return;
    };
    let mut detail = serde_json::Map::new();
    detail.insert("count".to_string(), serde_json::json!(hidden_details.len()));
    detail.insert("tool_count".to_string(), serde_json::json!(tool_count));
    detail.insert("thinking_count".to_string(), serde_json::json!(thinking_count));
    if !commentary.is_empty() {
        detail.insert("commentary".to_string(), serde_json::Value::Array(commentary));
    }
    if let Some(timeline) = after_id
        .as_deref()
        .and_then(|after| turn_detail_timeline(details, after, &before_id))
    {
        detail.insert("timeline".to_string(), serde_json::Value::Array(timeline));
    }
    if let Some(after_id) = after_id.filter(|id| !id.is_empty()) {
        detail.insert("after_id".to_string(), serde_json::json!(after_id));
    }
    detail.insert("before_id".to_string(), serde_json::json!(before_id));
    let mut reasoning_parts = Vec::new();
    for message in details {
        append_message_reasoning(message, &mut reasoning_parts);
    }
    append_message_reasoning(final_message, &mut reasoning_parts);
    if let Some(obj) = final_message.as_object_mut() {
        obj.insert("turn_details".to_string(), serde_json::Value::Object(detail));
        if !reasoning_parts.is_empty() {
            obj.insert("reasoning".to_string(), serde_json::Value::String(reasoning_parts.join("\n")));
        }
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

    skeleton.append(&mut detail_buffer);
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
    let page_start = end.saturating_sub(limit).max(start);
    (messages[page_start..end].to_vec(), page_start > start, false, total)
}

fn fetch_local_active_message_tail(
    state: &AppState,
    session_id: &str,
    limit: usize,
) -> anyhow::Result<Option<(Vec<serde_json::Value>, bool)>> {
    let db_path = state.hermes_home.join("state.db");
    if !db_path.exists() {
        return Ok(None);
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let reasoning_columns = local_reasoning_select_columns(&conn)?;
    let sql = format!(
        "SELECT id, session_id, role, content, tool_call_id, tool_calls, tool_name, timestamp, token_count, finish_reason, {reasoning_columns} \
         FROM messages WHERE session_id = ?1 AND active = 1 ORDER BY id DESC LIMIT ?2"
    );
    let mut stmt = conn.prepare(&sql)?;
    let mut messages = stmt
        .query_map(rusqlite::params![session_id, i64::try_from(limit.saturating_add(1))?], row_to_session_message)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if messages.is_empty() {
        return Ok(None);
    }
    let mut has_older_active = messages.len() > limit;
    messages.truncate(limit);
    messages.reverse();

    // The local tail can start in the middle of a tool/detail run. Keep the
    // preceding visible turn anchor in the latest skeleton so its detail
    // metadata gets an `after_id`; without it, the detail request spans every
    // older turn up to the final assistant message and the transcript order is
    // visibly wrong.
    if let Some(first_id) = messages.first().and_then(message_i64_id) {
        let first_role = messages.first().map(message_role).unwrap_or_default();
        let starts_inside_detail = first_role != "user"
            && first_role != "system"
            && !messages.first().is_some_and(is_completed_final_assistant_message);
        if starts_inside_detail {
            let anchor_sql = format!(
                "SELECT id, session_id, role, content, tool_call_id, tool_calls, tool_name, timestamp, token_count, finish_reason, {reasoning_columns} FROM messages WHERE session_id = ?1 AND active = 1 AND id < ?2 AND role IN ('user', 'system') ORDER BY id DESC LIMIT 1"
            );
            if let Some(anchor) = conn
                .query_row(&anchor_sql, rusqlite::params![session_id, first_id], row_to_session_message)
                .optional()?
            {
                messages.insert(0, anchor);
                has_older_active = true;
            }
        }
    }

    Ok(Some((messages, has_older_active)))
}

fn fetch_local_message_window_around(
    state: &AppState,
    session_id: &str,
    around: i64,
    limit: usize,
) -> anyhow::Result<Option<(Vec<serde_json::Value>, bool, bool)>> {
    let db_path = state.hermes_home.join("state.db");
    if !db_path.exists() {
        return Ok(None);
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let reasoning_columns = local_reasoning_select_columns(&conn)?;
    let select = |order: &str| {
        format!(
            "SELECT id, session_id, role, content, tool_call_id, tool_calls, tool_name, timestamp, token_count, finish_reason, {reasoning_columns} FROM messages WHERE session_id = ?1 AND active = 1 AND id {order} ?2 ORDER BY id DESC LIMIT ?3"
        )
    };
    let left_limit = (limit / 2).max(1);
    let right_limit = limit.saturating_sub(left_limit).max(1);
    let mut left = conn
        .prepare(&select("<="))?
        .query_map(rusqlite::params![session_id, around, left_limit + 1], row_to_session_message)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut right = conn
        .prepare(&select(">"))?
        .query_map(rusqlite::params![session_id, around, right_limit + 1], row_to_session_message)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if left.is_empty() && right.is_empty() {
        return Ok(None);
    }
    let has_older = left.len() > left_limit;
    let has_newer = right.len() > right_limit;
    left.truncate(left_limit);
    right.truncate(right_limit);
    left.reverse();
    right.reverse();
    left.extend(right);
    Ok(Some((left, has_older, has_newer)))
}

async fn chat_messages_page(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
    Query(query): Query<ChatMessagesQuery>,
) -> Response<Body> {
    let limit = query.limit.unwrap_or(24).clamp(1, 80);
    let requested_view = query.view.as_deref().unwrap_or("skeleton");
    if requested_view == "skeleton" && (query.before.is_some() || query.after.is_some()) {
        match fetch_local_skeleton_page(&state, &session_id, query.before, query.after, limit) {
            Ok(Some((mut messages, has_older, has_newer))) => {
                inject_turn_durations(&mut messages);
                let skeleton = history_skeleton_messages(&messages);
                return Json(serde_json::json!({
                    "object": "list",
                    "data": skeleton,
                    "has_older": has_older,
                    "has_newer": has_newer,
                    "metadata_pending": true
                }))
                .into_response();
            }
            Ok(None) => {}
            Err(err) => warn!(session_id = %session_id, error = %err, "cannot read local paged skeleton window"),
        }
    }
    if query.around.is_some() && query.before.is_none() && query.after.is_none()
        && let Some(around) = query.around
    {
            match fetch_local_message_window_around(&state, &session_id, around, limit) {
                Ok(Some((mut messages, has_older, has_newer))) => {
                    inject_turn_durations(&mut messages);
                    let skeleton = if requested_view == "skeleton" {
                        history_skeleton_messages(&messages)
                    } else {
                        messages
                    };
                    return Json(serde_json::json!({
                        "object": "list",
                        "data": skeleton,
                        "has_older": has_older,
                        "has_newer": has_newer,
                        "metadata_pending": true
                    }))
                    .into_response();
                }
                Ok(None) => {}
                Err(err) => warn!(session_id = %session_id, error = %err, "cannot read local around message window"),
            }
    }
    if requested_view == "latest"
        && query.before.is_none()
        && query.after.is_none()
        && query.around.is_none()
    {
        match fetch_local_active_message_tail(&state, &session_id, limit) {
            Ok(Some((mut latest, has_older))) => {
                inject_turn_durations(&mut latest);
                let skeleton = history_skeleton_messages(&latest);
                return Json(serde_json::json!({
                    "object": "list",
                    "data": skeleton,
                    "has_older": has_older,
                    "has_newer": false,
                    "metadata_pending": true
                }))
                .into_response();
            }
            Ok(None) => {}
            Err(err) => warn!(session_id = %session_id, error = %err, "cannot read local active message tail"),
        }
        let mut updates = subscribe_shared_session_watch(&state, &session_id).await;
        match timeout(API_SESSION_REQUEST_TIMEOUT, updates.recv()).await {
            Ok(Ok(mut latest)) => {
                inject_turn_durations(&mut latest);
                let skeleton = history_skeleton_messages(&latest);
                let (start, end) = page_bounds(&skeleton, &query, limit);
                return Json(serde_json::json!({
                    "object": "list",
                    "data": skeleton[start..end].to_vec(),
                    "has_older": start > 0,
                    "has_newer": false,
                    "metadata_pending": true
                }))
                .into_response();
            }
            Ok(Err(err)) => warn!(session_id = %session_id, error = %err, "shared remote latest feed closed"),
            Err(_) => warn!(session_id = %session_id, "shared remote latest feed timed out"),
        }
        return json_error(StatusCode::BAD_GATEWAY, "latest message feed unavailable");
    }

    if requested_view == "details" {
        match fetch_local_detail_range(&state, &session_id, query.after, query.before, limit) {
            Ok(Some((page, has_older, total))) => {
                return Json(serde_json::json!({
                    "object": "list",
                    "data": page,
                    "total": total,
                    "has_older": has_older,
                    "has_newer": false,
                }))
                .into_response();
            }
            Ok(None) => {}
            Err(err) => warn!(session_id = %session_id, error = %err, "cannot read local detail range; falling back to API history"),
        }
    }

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
    let view = if requested_view == "latest" {
        "skeleton"
    } else {
        requested_view
    };
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
        .into_iter()
        .filter(|message| !is_internal_model_switch_message(message))
        .collect()
}

fn is_internal_model_switch_message(message: &serde_json::Value) -> bool {
    message.get("role").and_then(|value| value.as_str()) == Some("user")
        && nav_message_text(message).trim_start().starts_with("/model ")
        && nav_message_text(message).contains(" --session")
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
            let role = message.get("role").and_then(|role| role.as_str());
            (role == Some("user") || message.get("history_gap").is_some_and(serde_json::Value::is_object))
                .then_some(index)
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

fn fetch_local_user_nav_messages(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Option<Vec<serde_json::Value>>> {
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
    let message_filter = local_message_history_filter(&conn, SessionMessageJoinMode::VisibleHistory)?;
    let context = messages_with_context_boundary_from_entries(
        &entries,
        SessionMessageJoinMode::VisibleHistory,
        |entry_id| {
            // The navigator needs ordering, user text, and assistant previews.  Keep
            // tool rows as tiny structural placeholders so minimap positions and the
            // total remain exact without deserializing tool output or reasoning blobs.
            let sql = format!(
                "SELECT id, session_id, role, \
                 CASE WHEN role IN ('user', 'assistant') THEN content END, timestamp \
                 FROM messages WHERE {message_filter} AND session_id = ?1 ORDER BY timestamp, id"
            );
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([entry_id], |row| {
                Ok(serde_json::json!({
                    "id": row.get::<_, i64>(0)?,
                    "session_id": row.get::<_, String>(1)?,
                    "role": row.get::<_, String>(2)?,
                    "content": row.get::<_, Option<String>>(3)?,
                    "timestamp": row.get::<_, f64>(4)?,
                }))
            })?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        },
    )?;
    Ok(Some(context.messages))
}

async fn chat_user_nav(
    State(state): State<Arc<AppState>>,
    AxumPath(session_id): AxumPath<String>,
) -> Response<Body> {
    let messages = match fetch_local_user_nav_messages(&state, &session_id) {
        Ok(Some(messages)) => messages,
        Ok(None) | Err(_) => match fetch_session_history_messages(&state, &session_id).await {
            Ok(messages) => messages,
            Err(err) => {
                return json_error(
                    StatusCode::BAD_GATEWAY,
                    &format!("user message navigator request failed: {err}"),
                );
            }
        },
    };
    Json(serde_json::json!({
        "object": "list",
        "data": build_user_message_nav(&messages),
        "total": messages.len(),
    }))
    .into_response()
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

type SessionResetRow = (
    Option<f64>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

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
    let row: Option<SessionResetRow> = conn
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
        return local_unique_reset_successor_without_thread_metadata(conn, session_id, ended_at, source.as_deref());
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

fn local_unique_reset_successor_without_thread_metadata(
    conn: &rusqlite::Connection,
    session_id: &str,
    ended_at: f64,
    source: Option<&str>,
) -> rusqlite::Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT id FROM sessions
         WHERE id != ?1
           AND started_at >= ?2 - 0.1
           AND started_at <= ?2 + 2.0
           AND source IS ?3
         ORDER BY started_at ASC
         LIMIT 2",
    )?;
    let ids = stmt
        .query_map(rusqlite::params![session_id, ended_at, source], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(if ids.len() == 1 { ids.into_iter().next() } else { None })
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

fn local_reasoning_select_columns(conn: &rusqlite::Connection) -> rusqlite::Result<String> {
    let details = if sqlite_table_has_columns(conn, "messages", &["reasoning_details"])? {
        "reasoning_details"
    } else {
        "NULL AS reasoning_details"
    };
    let codex_items = if sqlite_table_has_columns(conn, "messages", &["codex_reasoning_items"])? {
        "codex_reasoning_items"
    } else {
        "NULL AS codex_reasoning_items"
    };
    Ok(format!("reasoning, reasoning_content, {details}, {codex_items}"))
}

fn push_unique_reasoning(parts: &mut Vec<String>, text: &str) {
    let text = text.trim();
    if !text.is_empty() && !parts.iter().any(|part| part == text) {
        parts.push(text.to_string());
    }
}

fn collect_direct_reasoning(raw: Option<String>, parts: &mut Vec<String>) {
    let Some(raw) = raw else {
        return;
    };
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) {
        let mut text = String::new();
        collect_json_text(&value, &mut text);
        push_unique_reasoning(parts, &text);
    } else {
        push_unique_reasoning(parts, &raw);
    }
}

fn collect_provider_thinking(value: &serde_json::Value, parts: &mut Vec<String>) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_provider_thinking(item, parts);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(text) = map.get("thinking").and_then(|value| value.as_str()) {
                push_unique_reasoning(parts, text);
            }
            let kind = map.get("type").and_then(|value| value.as_str()).unwrap_or("");
            if kind.contains("reason")
                && let Some(text) = map.get("text").and_then(|value| value.as_str())
            {
                push_unique_reasoning(parts, text);
            }
        }
        _ => {}
    }
}

fn collect_codex_reasoning_summaries(value: &serde_json::Value, parts: &mut Vec<String>) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                collect_codex_reasoning_summaries(item, parts);
            }
        }
        serde_json::Value::Object(map) => {
            if let Some(summary) = map.get("summary") {
                let mut text = String::new();
                collect_json_text(summary, &mut text);
                push_unique_reasoning(parts, &text);
            }
        }
        _ => {}
    }
}

fn canonical_stored_reasoning(
    reasoning: Option<String>,
    reasoning_content: Option<String>,
    reasoning_details: Option<String>,
    codex_reasoning_items: Option<String>,
) -> Option<String> {
    let mut parts = Vec::new();
    collect_direct_reasoning(reasoning, &mut parts);
    collect_direct_reasoning(reasoning_content, &mut parts);
    if let Some(details) = reasoning_details
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&details)
    {
        collect_provider_thinking(&value, &mut parts);
    }
    if let Some(items) = codex_reasoning_items
        && let Ok(value) = serde_json::from_str::<serde_json::Value>(&items)
    {
        collect_codex_reasoning_summaries(&value, &mut parts);
    }
    (!parts.is_empty()).then(|| parts.join("\n"))
}

fn row_to_session_message(row: &rusqlite::Row<'_>) -> rusqlite::Result<serde_json::Value> {
    let raw_reasoning = row.get::<_, Option<String>>("reasoning")?;
    let raw_reasoning_content = row.get::<_, Option<String>>("reasoning_content")?;
    let canonical_reasoning = canonical_stored_reasoning(
        raw_reasoning,
        raw_reasoning_content.clone(),
        row.get::<_, Option<String>>("reasoning_details")?,
        row.get::<_, Option<String>>("codex_reasoning_items")?,
    );
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
    map.insert("reasoning".to_string(), canonical_reasoning.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null));
    map.insert("reasoning_content".to_string(), json_or_string_field(raw_reasoning_content));
    Ok(serde_json::Value::Object(map))
}

fn request_dump_item_text(value: Option<&serde_json::Value>) -> String {
    fn collect(value: &serde_json::Value, out: &mut Vec<String>) {
        match value {
            serde_json::Value::String(text) if !text.is_empty() => out.push(text.clone()),
            serde_json::Value::Array(items) => {
                for item in items {
                    collect(item, out);
                }
            }
            serde_json::Value::Object(map) => {
                for key in ["text", "input_text", "output_text", "content", "output"] {
                    if let Some(item) = map.get(key) {
                        collect(item, out);
                    }
                }
            }
            _ => {}
        }
    }
    let mut parts = Vec::new();
    if let Some(value) = value {
        collect(value, &mut parts);
    }
    parts.join("\n")
}

fn request_dump_timestamp_seconds(value: &serde_json::Value) -> Option<f64> {
    let timestamp = value.get("timestamp").and_then(|timestamp| timestamp.as_str())?;
    chrono::DateTime::parse_from_rfc3339(timestamp)
        .map(|timestamp| timestamp.timestamp_micros() as f64 / 1_000_000.0)
        .ok()
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(timestamp, "%Y-%m-%dT%H:%M:%S%.f")
                .map(|timestamp| timestamp.and_utc().timestamp_micros() as f64 / 1_000_000.0)
                .ok()
        })
}

fn redact_request_dump_arguments(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Array(items) => {
            for item in items {
                redact_request_dump_arguments(item);
            }
        }
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                let normalized = key.to_ascii_lowercase().replace(['-', '_'], "");
                if matches!(
                    normalized.as_str(),
                    "authorization"
                        | "proxyauthorization"
                        | "cookie"
                        | "setcookie"
                        | "apikey"
                        | "accesstoken"
                        | "refreshtoken"
                        | "password"
                        | "passwd"
                        | "secret"
                        | "clientsecret"
                        | "token"
                ) {
                    *value = serde_json::json!("[REDACTED]");
                } else {
                    redact_request_dump_arguments(value);
                }
            }
        }
        _ => {}
    }
}

fn sanitized_request_dump_arguments(value: Option<&serde_json::Value>) -> String {
    let mut arguments = match value {
        Some(serde_json::Value::String(arguments)) => {
            serde_json::from_str(arguments).unwrap_or_else(|_| serde_json::json!({}))
        }
        Some(arguments) => arguments.clone(),
        None => serde_json::json!({}),
    };
    redact_request_dump_arguments(&mut arguments);
    serde_json::to_string(&arguments).unwrap_or_else(|_| "{}".to_string())
}

fn request_dump_provider_items(dump: &serde_json::Value) -> Option<Vec<serde_json::Value>> {
    if let Some(items) = dump.pointer("/request/body/input").and_then(|value| value.as_array()) {
        return Some(items.clone());
    }

    let provider_messages = dump.pointer("/request/body/messages").and_then(|value| value.as_array())?;
    let mut items = Vec::new();
    for provider_message in provider_messages {
        let Some(role) = provider_message.get("role").and_then(|value| value.as_str()) else {
            continue;
        };
        let Some(content) = provider_message.get("content") else {
            continue;
        };
        if let Some(text) = content.as_str() {
            if !text.trim().is_empty() {
                items.push(serde_json::json!({"role": role, "content": text}));
            }
            continue;
        }
        let Some(blocks) = content.as_array() else {
            continue;
        };
        for block in blocks {
            match block.get("type").and_then(|value| value.as_str()) {
                Some("thinking") => {
                    let thinking = block.get("thinking").and_then(|value| value.as_str()).unwrap_or("");
                    if !thinking.trim().is_empty() {
                        items.push(serde_json::json!({"type": "reasoning", "summary": thinking}));
                    }
                }
                Some("text") => {
                    let text = block.get("text").and_then(|value| value.as_str()).unwrap_or("");
                    if !text.trim().is_empty() {
                        items.push(serde_json::json!({"role": role, "content": text}));
                    }
                }
                Some("tool_use") => {
                    let call_id = block.get("id").and_then(|value| value.as_str()).unwrap_or("");
                    let name = block.get("name").and_then(|value| value.as_str()).unwrap_or("");
                    if !call_id.is_empty() && !name.is_empty() {
                        items.push(serde_json::json!({
                            "type": "function_call",
                            "call_id": call_id,
                            "name": name,
                            "arguments": block.get("input").cloned().unwrap_or_else(|| serde_json::json!({}))
                        }));
                    }
                }
                Some("tool_result") => {
                    let call_id = block.get("tool_use_id").and_then(|value| value.as_str()).unwrap_or("");
                    if !call_id.is_empty() {
                        items.push(serde_json::json!({
                            "type": "function_call_output",
                            "call_id": call_id,
                            "output": block.get("content").cloned().unwrap_or(serde_json::Value::Null)
                        }));
                    }
                }
                _ => {}
            }
        }
    }
    (!items.is_empty()).then_some(items)
}

fn request_dump_messages(
    session_id: &str,
    dump: &serde_json::Value,
) -> Option<(f64, Vec<serde_json::Value>)> {
    // Request dumps can contain authorization headers. Deliberately touch only
    // the embedded session id, timestamp, and provider input transcript.
    if dump.get("session_id").and_then(|value| value.as_str()) != Some(session_id) {
        return None;
    }
    let dump_timestamp = request_dump_timestamp_seconds(dump)?;
    let items = request_dump_provider_items(dump)?;
    let mut messages = Vec::new();
    let mut tool_names = HashMap::new();
    for item in &items {
        let item_type = item.get("type").and_then(|value| value.as_str());
        let role = item.get("role").and_then(|value| value.as_str());
        let mut message = serde_json::Map::new();
        match (item_type, role) {
            (Some("function_call"), _) => {
                let call_id = item.get("call_id").and_then(|value| value.as_str()).unwrap_or("");
                let name = item.get("name").and_then(|value| value.as_str()).unwrap_or("");
                if call_id.is_empty() || name.is_empty() {
                    continue;
                }
                tool_names.insert(call_id.to_string(), name.to_string());
                let arguments = sanitized_request_dump_arguments(item.get("arguments"));
                message.insert("role".to_string(), serde_json::json!("assistant"));
                message.insert("content".to_string(), serde_json::json!(""));
                message.insert(
                    "tool_calls".to_string(),
                    serde_json::json!([{
                        "id": call_id,
                        "type": "function",
                        "function": {"name": name, "arguments": arguments}
                    }]),
                );
            }
            (Some("function_call_output"), _) => {
                let call_id = item.get("call_id").and_then(|value| value.as_str()).unwrap_or("");
                if call_id.is_empty() {
                    continue;
                }
                let name = tool_names.get(call_id).cloned().unwrap_or_default();
                message.insert("role".to_string(), serde_json::json!("tool"));
                message.insert("content".to_string(), serde_json::json!(request_dump_item_text(item.get("output"))));
                message.insert("tool_call_id".to_string(), serde_json::json!(call_id));
                if !name.is_empty() {
                    message.insert("tool_name".to_string(), serde_json::json!(name));
                }
            }
            (Some("reasoning"), _) => {
                let reasoning = request_dump_item_text(item.get("summary"));
                if reasoning.trim().is_empty() {
                    continue;
                }
                message.insert("role".to_string(), serde_json::json!("assistant"));
                message.insert("content".to_string(), serde_json::json!(""));
                message.insert("reasoning".to_string(), serde_json::json!(reasoning));
            }
            (_, Some("user" | "assistant" | "system")) => {
                message.insert("role".to_string(), serde_json::json!(role.unwrap_or("assistant")));
                message.insert("content".to_string(), serde_json::json!(request_dump_item_text(item.get("content"))));
            }
            _ => continue,
        }
        if role == Some("system") {
            let gap = item.get("history_gap").and_then(|value| value.as_object());
            let after = gap.and_then(|value| value.get("after")).and_then(|value| value.as_f64());
            let before = gap.and_then(|value| value.get("before")).and_then(|value| value.as_f64());
            if let (Some(after), Some(before)) = (after, before)
                && after.is_finite()
                && before.is_finite()
                && after >= 0.0
                && after < before
                && before <= dump_timestamp
            {
                message.insert(
                    "history_gap".to_string(),
                    serde_json::json!({"after": after, "before": before}),
                );
            }
        }
        let index = messages.len();
        message.insert("id".to_string(), serde_json::json!(-9_000_000_000_000i64 + index as i64));
        message.insert("session_id".to_string(), serde_json::json!(session_id));
        let fallback_timestamp = dump_timestamp - ((items.len().saturating_sub(index)) as f64 / 1_000.0);
        let timestamp = nav_message_timestamp_seconds(item)
            .filter(|timestamp| timestamp.is_finite() && *timestamp >= 0.0 && *timestamp <= dump_timestamp)
            .unwrap_or(fallback_timestamp);
        message.insert("timestamp".to_string(), serde_json::json!(timestamp));
        messages.push(serde_json::Value::Object(message));
    }
    (!messages.is_empty()).then_some((dump_timestamp, messages))
}

fn history_coverage_gap_message(
    session_id: &str,
    dump_timestamp: f64,
    first_history_timestamp: f64,
) -> Option<serde_json::Value> {
    const MAX_CONTIGUOUS_REQUEST_GAP_SECONDS: f64 = 24.0 * 60.0 * 60.0;
    if first_history_timestamp - dump_timestamp <= MAX_CONTIGUOUS_REQUEST_GAP_SECONDS {
        return None;
    }
    Some(serde_json::json!({
        "id": -8_000_000_000_000i64,
        "session_id": session_id,
        "role": "system",
        "content": "History coverage gap",
        "history_gap": {
            "after": dump_timestamp,
            "before": first_history_timestamp,
        },
    }))
}

fn recovered_request_dump_prefix(
    state: &AppState,
    session_id: &str,
    first_history_timestamp: f64,
) -> anyhow::Result<Option<Vec<serde_json::Value>>> {
    const MAX_REQUEST_DUMP_BYTES: u64 = 64 * 1024 * 1024;
    let sessions_dir = state.hermes_home.join("sessions");
    let prefix = format!("request_dump_{session_id}_");
    let mut best: Option<(usize, f64, Vec<serde_json::Value>)> = None;
    let entries = match std::fs::read_dir(sessions_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err.into()),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !name.starts_with(&prefix) || !name.ends_with(".json") {
            continue;
        }
        let metadata = entry.metadata()?;
        if !metadata.is_file() || metadata.len() > MAX_REQUEST_DUMP_BYTES {
            continue;
        }
        let dump: serde_json::Value = match serde_json::from_slice(&std::fs::read(entry.path())?) {
            Ok(dump) => dump,
            Err(_) => continue,
        };
        let Some((dump_timestamp, messages)) = request_dump_messages(session_id, &dump) else {
            continue;
        };
        if dump_timestamp >= first_history_timestamp {
            continue;
        }
        // A later dump may already contain a compacted summary with fewer raw
        // turns. Prefer the richest recoverable prefix; use time only to break ties.
        let rank = (messages.len(), dump_timestamp, messages);
        if best.as_ref().is_none_or(|current| (rank.0, rank.1) > (current.0, current.1)) {
            best = Some(rank);
        }
    }
    Ok(best.map(|(_, dump_timestamp, mut messages)| {
        if let Some(gap) = history_coverage_gap_message(session_id, dump_timestamp, first_history_timestamp) {
            messages.push(gap);
        }
        messages
    }))
}

fn prepend_recovered_request_dump_prefix(
    state: &AppState,
    session_id: &str,
    context: &mut ContextWindowMessages,
) {
    let Some(first_history_timestamp) = context.messages.iter().find_map(nav_message_timestamp_seconds) else {
        return;
    };
    match recovered_request_dump_prefix(state, session_id, first_history_timestamp) {
        Ok(Some(mut recovered)) => {
            let recovered_len = recovered.len();
            recovered.append(&mut context.messages);
            context.messages = recovered;
            context.boundary_start = context.boundary_start.saturating_add(recovered_len);
        }
        Ok(None) => {}
        Err(err) => warn!(session_id = %session_id, error = %err, "cannot recover request-dump history prefix"),
    }
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

fn fetch_local_detached_session_switch_messages(
    state: &AppState,
    session_id: &str,
) -> anyhow::Result<Vec<serde_json::Value>> {
    let db_path = state.hermes_home.join("state.db");
    if !db_path.exists() {
        return Ok(Vec::new());
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    if !sqlite_table_has_columns(
        &conn,
        "sessions",
        &[
            "started_at",
            "end_reason",
            "source",
            "session_key",
            "chat_id",
            "thread_id",
        ],
    )? {
        return Ok(Vec::new());
    }
    let entries = local_session_history_entries(&conn, session_id)?;
    let Some(root_id) = entries.first().map(|entry| entry.id.as_str()) else {
        return Ok(Vec::new());
    };
    let root: Option<SessionResetLookupRow> = conn
        .query_row(
            "SELECT started_at, session_key, source, chat_id, thread_id FROM sessions WHERE id = ?1",
            [root_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((started_at, Some(session_key), source, chat_id, thread_id)) = root else {
        return Ok(Vec::new());
    };
    if session_key.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut session_stmt = conn.prepare(
        "SELECT id FROM sessions
         WHERE id != ?1
           AND end_reason = 'session_switch'
           AND started_at >= ?2
           AND session_key = ?3
           AND source IS ?4
           AND chat_id IS ?5
           AND thread_id IS ?6
         ORDER BY started_at, id",
    )?;
    let peer_ids = session_stmt
        .query_map(
            rusqlite::params![root_id, started_at, session_key, source, chat_id, thread_id],
            |row| row.get::<_, String>(0),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    if peer_ids.is_empty() {
        return Ok(Vec::new());
    }
    let message_filter = local_message_history_filter(&conn, SessionMessageJoinMode::VisibleHistory)?;
    let reasoning_columns = local_reasoning_select_columns(&conn)?;
    let mut messages = Vec::new();
    for peer_id in peer_ids {
        let sql = format!(
            "SELECT id, session_id, role, content, tool_call_id, tool_calls, tool_name, timestamp, token_count, finish_reason, {reasoning_columns} \
             FROM messages WHERE {message_filter} AND session_id = ?1 ORDER BY timestamp, id"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([peer_id], row_to_session_message)?;
        messages.extend(rows.collect::<rusqlite::Result<Vec<_>>>()?);
    }
    messages.sort_by(|left, right| {
        nav_message_timestamp_seconds(left)
            .partial_cmp(&nav_message_timestamp_seconds(right))
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| session_message_id(left).cmp(&session_message_id(right)))
    });
    Ok(messages)
}

fn merge_detached_session_switch_history(
    state: &AppState,
    session_id: &str,
    context: &mut ContextWindowMessages,
) {
    let mut detached = match fetch_local_detached_session_switch_messages(state, session_id) {
        Ok(messages) => messages,
        Err(err) => {
            warn!(session_id = %session_id, error = %err, "cannot read detached session-switch history");
            return;
        }
    };
    if detached.is_empty() {
        return;
    }
    let known_ids = context
        .messages
        .iter()
        .filter_map(session_message_id)
        .collect::<HashSet<_>>();
    detached.retain(|message| session_message_id(message).is_none_or(|id| !known_ids.contains(&id)));
    if detached.is_empty() {
        return;
    }
    let boundary = context.boundary_start.min(context.messages.len());
    let mut suffix = context.messages.split_off(boundary);
    context.messages.extend(detached);
    context.messages.sort_by(|left, right| {
        match (nav_message_timestamp_seconds(left), nav_message_timestamp_seconds(right)) {
            (Some(left_timestamp), Some(right_timestamp)) => left_timestamp
                .partial_cmp(&right_timestamp)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| session_message_id(left).cmp(&session_message_id(right))),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => std::cmp::Ordering::Equal,
        }
    });
    context.boundary_start = context.messages.len();
    context.messages.append(&mut suffix);
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

fn local_message_history_filter(
    conn: &rusqlite::Connection,
    mode: SessionMessageJoinMode,
) -> rusqlite::Result<&'static str> {
    if mode == SessionMessageJoinMode::VisibleHistory
        && sqlite_table_has_columns(conn, "messages", &["compacted"])?
    {
        Ok("(active = 1 OR compacted = 1)")
    } else {
        Ok("active = 1")
    }
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
    let message_filter = local_message_history_filter(&conn, mode)?;
    let reasoning_columns = local_reasoning_select_columns(&conn)?;
    let context = messages_with_context_boundary_from_entries(&entries, mode, |entry_id| {
        let sql = format!(
            "SELECT id, session_id, role, content, tool_call_id, tool_calls, tool_name, timestamp, token_count, finish_reason, {reasoning_columns} \
             FROM messages WHERE {message_filter} AND session_id = ?1 ORDER BY id"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([entry_id], row_to_session_message)?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    })?;
    Ok(Some(context))
}

fn fetch_local_detail_range(
    state: &AppState,
    session_id: &str,
    after: Option<i64>,
    before: Option<i64>,
    limit: usize,
) -> anyhow::Result<Option<(Vec<serde_json::Value>, bool, usize)>> {
    let db_path = state.hermes_home.join("state.db");
    if !db_path.exists() || (after.is_none() && before.is_none()) {
        return Ok(None);
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    let message_filter = local_message_history_filter(&conn, SessionMessageJoinMode::VisibleHistory)?;
    let reasoning_columns = local_reasoning_select_columns(&conn)?;
    let count_sql = format!(
        "SELECT COUNT(*) FROM messages WHERE {message_filter} AND session_id = ?1 AND (?2 IS NULL OR id > ?2) AND (?3 IS NULL OR id < ?3)"
    );
    let total: usize = conn.query_row(&count_sql, rusqlite::params![session_id, after, before], |row| row.get::<_, i64>(0))?.try_into()?;
    if total == 0 {
        return Ok(None);
    }
    let page_sql = format!(
        "SELECT id, session_id, role, content, tool_call_id, tool_calls, tool_name, timestamp, token_count, finish_reason, {reasoning_columns} FROM messages WHERE {message_filter} AND session_id = ?1 AND (?2 IS NULL OR id > ?2) AND (?3 IS NULL OR id < ?3) ORDER BY id DESC LIMIT ?4"
    );
    let mut stmt = conn.prepare(&page_sql)?;
    let rows = stmt.query_map(rusqlite::params![session_id, after, before, i64::try_from(limit)?], row_to_session_message)?;
    let mut page = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    page.reverse();
    Ok(Some((page, total > limit, total)))
}

fn fetch_local_skeleton_page(
    state: &AppState,
    session_id: &str,
    before: Option<i64>,
    after: Option<i64>,
    limit: usize,
) -> anyhow::Result<Option<(Vec<serde_json::Value>, bool, bool)>> {
    let Some(cursor) = before.or(after) else {
        return Ok(None);
    };
    if before.is_some() == after.is_some() {
        return Ok(None);
    }
    let db_path = state.hermes_home.join("state.db");
    if !db_path.exists() {
        return Ok(None);
    }
    let conn = rusqlite::Connection::open_with_flags(
        db_path,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    // A single local session has a globally ordered message-id space.  Keep
    // the existing full lineage/API path for reset and detached-session cases
    // until their cursor mapping can be made equally bounded.
    let entries = local_session_history_entries(&conn, session_id)?;
    if entries.len() != 1 || entries[0].id != session_id {
        return Ok(None);
    }
    let message_filter = local_message_history_filter(&conn, SessionMessageJoinMode::VisibleHistory)?;
    let reasoning_columns = local_skeleton_reasoning_select_columns(&conn)?;
    let select = format!(
        "SELECT id, session_id, role, \
         CASE WHEN role IN ('user', 'assistant', 'system') THEN content END AS content, \
         CASE WHEN role IN ('user', 'assistant', 'system') THEN tool_call_id END AS tool_call_id, \
         CASE WHEN role = 'assistant' THEN tool_calls END AS tool_calls, \
         CASE WHEN role = 'tool' THEN tool_name END AS tool_name, \
         timestamp, token_count, \
         CASE WHEN role = 'assistant' THEN finish_reason END AS finish_reason, \
         {reasoning_columns} \
         FROM messages WHERE {message_filter} AND session_id = ?1 AND id {} ?2 ORDER BY id {} LIMIT ?3",
        if before.is_some() { "<" } else { ">" },
        if before.is_some() { "DESC" } else { "ASC" },
    );
    let mut stmt = conn.prepare(&select)?;
    let mut messages = stmt
        .query_map(
            rusqlite::params![session_id, cursor, i64::try_from(limit.saturating_add(1))?],
            row_to_session_message,
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let has_more = messages.len() > limit;
    messages.truncate(limit);
    if messages.is_empty() {
        return Ok(None);
    }
    if before.is_some() {
        messages.reverse();
        Ok(Some((messages, has_more, true)))
    } else {
        Ok(Some((messages, true, has_more)))
    }
}

fn local_skeleton_reasoning_select_columns(conn: &rusqlite::Connection) -> rusqlite::Result<String> {
    let details = if sqlite_table_has_columns(conn, "messages", &["reasoning_details"])? {
        "CASE WHEN role = 'assistant' THEN reasoning_details END AS reasoning_details"
    } else {
        "NULL AS reasoning_details"
    };
    let codex_items = if sqlite_table_has_columns(conn, "messages", &["codex_reasoning_items"])? {
        "CASE WHEN role = 'assistant' THEN codex_reasoning_items END AS codex_reasoning_items"
    } else {
        "NULL AS codex_reasoning_items"
    };
    Ok(format!(
        "CASE WHEN role = 'assistant' THEN reasoning END AS reasoning, \
         CASE WHEN role = 'assistant' THEN reasoning_content END AS reasoning_content, \
         {details}, {codex_items}"
    ))
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
    let local_context = match fetch_local_history_context_messages(state, session_id) {
        Ok(context) => context,
        Err(err) => {
            warn!(session_id = %session_id, error = %err, "cannot read local visible history");
            None
        }
    };
    let mut context = match fetch_api_context_window_messages(state, &entries, SessionMessageJoinMode::VisibleHistory).await {
        Ok(api_context) => match local_context {
            Some(local_context) if local_context.messages.len() > api_context.messages.len() => local_context,
            _ => api_context,
        },
        Err(err) => {
            warn!(session_id = %session_id, error = %err, "API Server history fetch failed; falling back to local state.db");
            match local_context {
                Some(local_context) => local_context,
                None => anyhow::bail!("neither API Server nor local state.db has history messages for session lineage of {session_id}: API={err}"),
            }
        }
    };
    prepend_recovered_request_dump_prefix(state, session_id, &mut context);
    merge_detached_session_switch_history(state, session_id, &mut context);
    Ok(context)
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

async fn shared_session_watch_feed(
    state: &AppState,
    session_id: &str,
) -> Arc<SharedSessionWatchFeed> {
    let mut feeds = state.session_watch_feeds.write().await;
    feeds
        .entry(session_id.to_string())
        .or_insert_with(|| Arc::new(SharedSessionWatchFeed::new()))
        .clone()
}

async fn subscribe_shared_session_watch(
    state: &Arc<AppState>,
    session_id: &str,
) -> broadcast::Receiver<Vec<serde_json::Value>> {
    let feed = shared_session_watch_feed(state, session_id).await;
    let receiver = feed.updates.subscribe();
    if feed
        .worker_started
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        let worker_state = state.clone();
        let worker_session_id = session_id.to_string();
        let worker_feed = feed.clone();
        tokio::spawn(async move {
            run_shared_session_watch(worker_state, worker_session_id, worker_feed).await;
        });
    }
    receiver
}

async fn run_shared_session_watch(
    state: Arc<AppState>,
    session_id: String,
    feed: Arc<SharedSessionWatchFeed>,
) {
    let mut initialized = false;
    let mut watch_state = SessionMessageWatchState::default();
    loop {
        if feed.updates.receiver_count() == 0 {
            feed.worker_started.store(false, Ordering::Release);
            if feed.updates.receiver_count() == 0 {
                let mut feeds = state.session_watch_feeds.write().await;
                if feeds
                    .get(&session_id)
                    .is_some_and(|current| Arc::ptr_eq(current, &feed))
                    && feed.updates.receiver_count() == 0
                {
                    feeds.remove(&session_id);
                }
                return;
            }
            if feed
                .worker_started
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                return;
            }
        }
        let items = match fetch_local_active_message_tail(&state, &session_id, 24) {
            Ok(Some((items, _))) => Ok(items),
            Ok(None) => fetch_session_messages_for_watch(
                &state.client,
                &state.api_url,
                &state.api_key,
                &session_id,
            )
            .await,
            Err(err) => {
                warn!(session_id = %session_id, error = %err, "cannot read local message watch tail; falling back to API");
                fetch_session_messages_for_watch(
                    &state.client,
                    &state.api_url,
                    &state.api_key,
                    &session_id,
                )
                .await
            }
        };
        if let Ok(items) = items {
            let updates = if initialized {
                changed_session_messages(&items, &mut watch_state)
            } else {
                initialized = true;
                watch_state = session_message_watch_state(&items);
                items
            };
            if !updates.is_empty() {
                let _ = feed.updates.send(updates);
            }
        }
        tokio::time::sleep(CHAT_WATCH_POLL_INTERVAL).await;
    }
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
    let active_chat_streams = state.active_chat_streams.clone();
    let mut shared_watch_rx = subscribe_shared_session_watch(&state, &session_id).await;
    let mut chat_stream_rx = state.chat_streams.subscribe();
    let stream = async_stream::stream! {
        if let Some(messages) = active_chat_streams
            .read()
            .await
            .get(&session_id)
            .map(|snapshot| snapshot.messages.clone())
        {
            for msg in messages {
                yield Ok(SseEvent::default().data(msg.to_string()));
            }
        }
        'stream_loop: loop {
            tokio::select! {
                update = shared_watch_rx.recv() => match update {
                    Ok(messages) => {
                        for msg in messages {
                            yield Ok(SseEvent::default().data(msg.to_string()));
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break 'stream_loop,
                },
                event = chat_stream_rx.recv() => match event {
                    Ok(text) => {
                        if let Ok(envelope) = serde_json::from_str::<serde_json::Value>(&text)
                            && envelope.get("session_id").and_then(|value| value.as_str()) == Some(session_id.as_str())
                            && let Some(message) = envelope.get("message")
                        {
                            yield Ok(SseEvent::default().data(message.to_string()));
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => break 'stream_loop,
                },
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::default())
}
