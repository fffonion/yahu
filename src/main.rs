use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
    convert::Infallible,
    env,
    ffi::OsStr,
    io::Read,
    net::IpAddr,
    path::{Component, Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use axum::{
    Json, Router,
    body::{Body, to_bytes},
    extract::{Path as AxumPath, Query, State},
    http::{HeaderMap, HeaderValue, Method, Request, Response, StatusCode, Uri, header},
    middleware::{self, Next},
    response::{
        Html, IntoResponse,
        sse::{Event as SseEvent, KeepAlive, Sse},
    },
    routing::{any, get, patch, post},
};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use clap::Parser;
use filetime::FileTime;
use yet_another_hermes_ui::{ModelCache, fresh_model_cache_body, model_cache_payload_from_source};
use hmac::{Hmac, Mac};
use include_dir::{Dir, include_dir};
use notify::{
    EventKind, RecommendedWatcher, RecursiveMode, Watcher,
    event::{AccessKind, AccessMode, ModifyKind},
};
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tokio::{
    fs,
    net::TcpListener,
    process::Command,
    sync::{RwLock, broadcast, mpsc},
    time::{Instant, sleep, sleep_until, timeout},
};
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

type HmacSha256 = Hmac<Sha256>;

static ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/dist");
const SESSION_COOKIE: &str = "hermes_webui_session";
const MAX_PROXY_BODY: usize = 32 * 1024 * 1024;
const SESSION_TTL: u64 = 7 * 24 * 60 * 60;

#[derive(Parser, Debug)]
#[command(author, version, about = "Yet Another Hermes UI — single-binary web interface for Hermes Agent")]
struct Args {
    #[arg(long, env = "HERMES_WEBUI_HOST", default_value = "127.0.0.1")]
    host: IpAddr,

    #[arg(long, env = "HERMES_WEBUI_PORT", default_value_t = 9642)]
    port: u16,

    #[arg(long, env = "HERMES_API_URL", default_value = "http://127.0.0.1:8642")]
    api_url: String,

    #[arg(long, env = "HERMES_API_KEY")]
    api_key: Option<String>,

    #[arg(long, env = "HERMES_WEBUI_AUTH_KEY")]
    auth_key: Option<String>,

    #[arg(long, default_value_t = false)]
    insecure: bool,

    #[arg(long, env = "HERMES_WEBUI_WORKSPACE", default_value = ".")]
    workspace: PathBuf,

    #[arg(long, env = "HERMES_HOME")]
    hermes_home: Option<PathBuf>,

    #[arg(long, env = "HERMES_WEBUI_IMAGE_DIR")]
    image_dir: Option<PathBuf>,
}

#[derive(Clone)]
struct AppState {
    client: reqwest::Client,
    api_url: String,
    api_key: Option<String>,
    auth_key: Option<String>,
    insecure: bool,
    workspace: PathBuf,
    hermes_home: PathBuf,
    image_dir: PathBuf,
    updates: broadcast::Sender<String>,
    deletes: broadcast::Sender<String>,
    model_cache: Arc<RwLock<ModelCache>>,
}

#[derive(Deserialize)]
struct WorkspaceQuery {
    path: Option<String>,
    download: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceList {
    root: String,
    path: String,
    entries: Vec<WorkspaceEntry>,
}

#[derive(Serialize)]
struct WorkspaceEntry {
    name: String,
    path: String,
    kind: String,
    size: Option<u64>,
    modified: Option<String>,
}

#[derive(Deserialize)]
struct WorkspaceRenamePayload {
    name: String,
}

#[derive(Deserialize)]
struct SkillQuery {
    name: Option<String>,
    path: Option<String>,
}

#[derive(Serialize, Clone)]
struct SkillInfo {
    name: String,
    description: String,
    category: String,
    enabled: bool,
}

#[derive(Deserialize)]
struct SkillTogglePayload {
    enabled: bool,
}

#[derive(Deserialize)]
struct MemoryPayload {
    memory: String,
    user: String,
}

#[derive(Serialize)]
struct MemoryResponse {
    memory: String,
    user: String,
}

#[derive(Deserialize)]
struct ChatMessagesQuery {
    before: Option<i64>,
    after: Option<i64>,
    limit: Option<usize>,
}

#[derive(Deserialize)]
struct SessionSearchQuery {
    q: Option<String>,
    limit: Option<usize>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if !args.insecure && args.auth_key.as_deref().unwrap_or_default().is_empty() {
        anyhow::bail!(
            "set HERMES_WEBUI_AUTH_KEY / --auth-key, or pass --insecure for local-only testing"
        );
    }

    let workspace = args.workspace.canonicalize().unwrap_or(args.workspace);
    let hermes_home = args.hermes_home.unwrap_or_else(|| {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".hermes")
    });
    let image_dir = args
        .image_dir
        .unwrap_or_else(|| hermes_home.join("cache/images"));
    fs::create_dir_all(&image_dir).await?;
    let image_dir = image_dir.canonicalize().unwrap_or(image_dir);
    let (updates, _) = broadcast::channel::<String>(128);
    let (deletes, _) = broadcast::channel::<String>(128);
    let (fs_tx, fs_rx) = mpsc::unbounded_channel::<PathBuf>();
    let _image_watcher = start_image_watcher(&image_dir, fs_tx)?;
    tokio::spawn(process_fs_events(image_dir.clone(), fs_rx, updates.clone()));

    let state = Arc::new(AppState {
        client: reqwest::Client::new(),
        api_url: args.api_url.trim_end_matches('/').to_string(),
        api_key: args
            .api_key
            .or_else(|| std::env::var("API_SERVER_KEY").ok()),
        auth_key: args.auth_key,
        insecure: args.insecure,
        workspace,
        hermes_home,
        image_dir,
        updates,
        deletes,
        model_cache: Arc::new(RwLock::new(ModelCache::default())),
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/login", get(login_page).post(login_submit))
        .route("/logout", post(logout))
        .route("/workspace/list", get(workspace_list))
        .route("/workspace/file", get(workspace_file))
        // Keep route shape discoverable for regression tests: .route("/workspace/item", patch(workspace_rename).delete(workspace_delete))
        .route(
            "/workspace/item",
            patch(workspace_rename).delete(workspace_delete),
        )
        .route("/skills/list", get(skills_list))
        .route("/skills/files", get(skill_files))
        .route("/skills/file", get(skill_file))
        .route("/skills/toggle/{name}", post(skill_toggle))
        .route("/memory", get(memory_get).put(memory_put))
        .route("/models-cache", get(models_cached))
        .route("/sessions/search", get(sessions_search))
        .route("/chat/messages/{session_id}", get(chat_messages_page))
        .route("/chat/watch/{session_id}", get(chat_watch))
        .route("/image-api/images", get(list_images))
        .route("/image-api/images/refresh", get(refresh_images))
        .route("/image-api/stats", get(image_stats))
        .route("/image-api/events", get(image_events))
        .route("/image-files/{filename}", get(serve_png))
        .route("/image-download/{filename}", get(download_heic))
        .route("/image-api/images/{filename}/metadata", get(image_metadata))
        // Keep route shape discoverable for regression tests: .route("/image-api/images/{filename}", get(image_entry).delete(delete_image))
        .route(
            "/image-api/images/{filename}",
            get(image_entry).delete(delete_image),
        )
        .route("/image-api/images/{filename}/heic", post(generate_heic))
        .route("/image-api/batch-delete", post(batch_delete))
        .route("/image-api/batch-download", post(batch_download))
        .route("/image-api/batch-mtime", post(batch_mtime))
        .route("/hermes/{*path}", any(proxy_hermes))
        .fallback(static_assets)
        .layer(middleware::from_fn_with_state(state.clone(), require_auth))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = TcpListener::bind((args.host, args.port)).await?;
    eprintln!(
        "Hermes Headless WebUI listening on http://{}:{}",
        args.host, args.port
    );
    axum::serve(listener, app).await?;
    Ok(())
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok"}))
}

async fn require_auth(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let path = req.uri().path();
    if state.insecure || path == "/health" || path == "/login" {
        return next.run(req).await;
    }
    if valid_cookie(req.headers(), &state) {
        return next.run(req).await;
    }
    if path.starts_with("/hermes")
        || path.starts_with("/workspace")
        || path.starts_with("/models-cache")
        || path.starts_with("/skills")
        || path.starts_with("/sessions/search")
        || path.starts_with("/chat/messages")
        || path.starts_with("/image-api")
        || path.starts_with("/image-files")
        || path.starts_with("/image-download")
    {
        return json_error(StatusCode::UNAUTHORIZED, "login required");
    }
    Html(login_html("Login required")).into_response()
}

async fn login_page(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    if state.insecure {
        return Response::builder()
            .status(StatusCode::SEE_OTHER)
            .header(header::LOCATION, "/")
            .body(Body::empty())
            .unwrap();
    }
    Html(login_html("")).into_response()
}

async fn login_submit(State(state): State<Arc<AppState>>, req: Request<Body>) -> Response<Body> {
    let body = match to_bytes(req.into_body(), 64 * 1024).await {
        Ok(bytes) => bytes,
        Err(_) => return json_error(StatusCode::BAD_REQUEST, "invalid login body"),
    };
    let params: std::collections::HashMap<String, String> =
        serde_urlencoded::from_bytes(&body).unwrap_or_default();
    let password = params.get("password").map(String::as_str).unwrap_or("");
    let expected = state.auth_key.as_deref().unwrap_or("");
    if expected.is_empty() || password != expected {
        return Html(login_html("Wrong key")).into_response();
    }
    let token = make_session_token(expected);
    let cookie = format!(
        "{}={}; Path=/; Max-Age={}; HttpOnly; SameSite=Lax",
        SESSION_COOKIE, token, SESSION_TTL
    );
    Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(header::LOCATION, "/")
        .header(header::SET_COOKIE, cookie)
        .body(Body::empty())
        .unwrap()
}

async fn logout() -> Response<Body> {
    Response::builder()
        .status(StatusCode::SEE_OTHER)
        .header(header::LOCATION, "/login")
        .header(
            header::SET_COOKIE,
            format!(
                "{}=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax",
                SESSION_COOKIE
            ),
        )
        .body(Body::empty())
        .unwrap()
}

async fn static_assets(uri: Uri) -> Response<Body> {
    let req_path = uri.path().trim_start_matches('/');
    let file_path = if req_path.is_empty() {
        "index.html"
    } else {
        req_path
    };
    let file = ASSETS
        .get_file(file_path)
        .or_else(|| ASSETS.get_file("index.html"));
    match file {
        Some(file) => {
            let mime = mime_guess::from_path(file.path()).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(file.contents().to_vec()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("asset not found"))
            .unwrap(),
    }
}

async fn proxy_hermes(
    State(state): State<Arc<AppState>>,
    AxumPath(path): AxumPath<String>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Body,
) -> Response<Body> {
    let query = uri.query().map(|q| format!("?{}", q)).unwrap_or_default();
    let url = format!("{}/{}{}", state.api_url, path, query);
    let req_method =
        reqwest::Method::from_bytes(method.as_str().as_bytes()).unwrap_or(reqwest::Method::GET);
    let bytes = match to_bytes(body, MAX_PROXY_BODY).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return json_error(StatusCode::BAD_REQUEST, &format!("cannot read body: {err}"));
        }
    };
    let mut builder = state.client.request(req_method, url).body(bytes);
    for (key, value) in headers.iter() {
        let name = key.as_str().to_ascii_lowercase();
        if !should_forward_proxy_header(&name) {
            continue;
        }
        builder = builder.header(key.as_str(), value.as_bytes());
    }
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        builder = builder.bearer_auth(key);
    }
    match builder.send().await {
        Ok(resp) => response_from_reqwest(resp).await,
        Err(err) => json_error(
            StatusCode::BAD_GATEWAY,
            &format!("Hermes API proxy failed: {err}"),
        ),
    }
}

fn should_forward_proxy_header(name: &str) -> bool {
    !matches!(
        name,
        "host"
            | "cookie"
            | "authorization"
            | "origin"
            | "referer"
            | "connection"
            | "content-length"
            | "transfer-encoding"
    ) && !name.starts_with("sec-fetch-")
        && !name.starts_with("sec-ch-")
}

async fn response_from_reqwest(resp: reqwest::Response) -> Response<Body> {
    let status = StatusCode::from_u16(resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut builder = Response::builder().status(status);
    for (key, value) in resp.headers() {
        if matches!(
            key.as_str(),
            "content-type" | "cache-control" | "x-hermes-session-id" | "x-hermes-session-key"
        ) {
            builder = builder.header(key, value);
        }
    }
    builder
        .body(Body::from_stream(resp.bytes_stream()))
        .unwrap()
}

async fn models_cached(State(state): State<Arc<AppState>>) -> Response<Body> {
    const MODEL_CACHE_TTL: Duration = Duration::from_secs(300);
    {
        let cache = state.model_cache.read().await;
        if let Some(body) = fresh_model_cache_body(&cache, MODEL_CACHE_TTL) {
            return Json(body).into_response();
        }
    }

    let body = match load_hermes_model_inventory(&state).await {
        Ok(inventory) => model_cache_payload_from_source(&inventory, "hermes_inventory"),
        Err(inventory_err) => match fetch_api_server_models(&state).await {
            Ok(body) => model_cache_payload_from_source(&body, "api_server"),
            Err(proxy_err) => {
                return json_error(
                    StatusCode::BAD_GATEWAY,
                    &format!(
                        "model list unavailable: inventory: {inventory_err}; api_server: {proxy_err}"
                    ),
                );
            }
        },
    };

    let mut cache = state.model_cache.write().await;
    cache.fetched_at = Some(std::time::Instant::now());
    cache.body = Some(body.clone());
    Json(body).into_response()
}

async fn fetch_api_server_models(state: &AppState) -> anyhow::Result<serde_json::Value> {
    let mut req = state.client.get(format!("{}/v1/models", state.api_url));
    if let Some(key) = &state.api_key
        && !key.is_empty()
    {
        req = req.bearer_auth(key);
    }
    let resp = req.send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("model list request failed: {}", resp.status());
    }
    Ok(resp.json::<serde_json::Value>().await?)
}

async fn load_hermes_model_inventory(state: &AppState) -> anyhow::Result<serde_json::Value> {
    let agent_dir = env::var("HERMES_AGENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| state.hermes_home.join("hermes-agent"));
    if !agent_dir.join("hermes_cli/inventory.py").exists() {
        anyhow::bail!("Hermes agent source not found at {}", agent_dir.display());
    }
    let python = env::var("HERMES_WEBUI_PYTHON").unwrap_or_else(|_| "python3".to_string());
    let script = r#"
import json, os, sys
agent_dir = os.environ.get('HERMES_AGENT_DIR')
sys.path.insert(0, agent_dir)
from hermes_cli.inventory import build_models_payload, load_picker_context
payload = build_models_payload(load_picker_context(), max_models=80, capabilities=True)
print(json.dumps(payload))
"#;
    let output = timeout(
        Duration::from_secs(45),
        Command::new(python)
            .arg("-c")
            .arg(script)
            .env("HERMES_AGENT_DIR", &agent_dir)
            .env("HERMES_HOME", &state.hermes_home)
            .output(),
    )
    .await??;
    if !output.status.success() {
        anyhow::bail!(
            "inventory command failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(serde_json::from_slice(&output.stdout)?)
}

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
        state.api_url, session_id
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

async fn workspace_list(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> Response<Body> {
    let rel = query.path.unwrap_or_default();
    let dir = match resolve_workspace_path(&state.workspace, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    let mut reader = match fs::read_dir(&dir).await {
        Ok(reader) => reader,
        Err(err) => {
            return json_error(
                StatusCode::NOT_FOUND,
                &format!("cannot read directory: {err}"),
            );
        }
    };
    let mut entries = Vec::new();
    while let Ok(Some(entry)) = reader.next_entry().await {
        let meta = match entry.metadata().await {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        let path = rel_join(&rel, &name);
        let kind = if meta.is_dir() { "dir" } else { "file" }.to_string();
        let modified = meta.modified().ok().and_then(system_time_string);
        entries.push(WorkspaceEntry {
            name,
            path,
            kind,
            size: if meta.is_file() {
                Some(meta.len())
            } else {
                None
            },
            modified,
        });
    }
    entries.sort_by(|a, b| {
        (a.kind.as_str() != "dir", a.name.to_lowercase())
            .cmp(&(b.kind.as_str() != "dir", b.name.to_lowercase()))
    });
    Json(WorkspaceList {
        root: state.workspace.display().to_string(),
        path: rel,
        entries,
    })
    .into_response()
}

async fn workspace_file(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> Response<Body> {
    let rel = query.path.unwrap_or_default();
    let file = match resolve_workspace_path(&state.workspace, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    let meta = match fs::metadata(&file).await {
        Ok(meta) => meta,
        Err(err) => return json_error(StatusCode::NOT_FOUND, &format!("cannot stat file: {err}")),
    };
    if !meta.is_file() {
        return json_error(StatusCode::BAD_REQUEST, "path is not a file");
    }
    let bytes = match fs::read(&file).await {
        Ok(bytes) => bytes,
        Err(err) => return json_error(StatusCode::NOT_FOUND, &format!("cannot read file: {err}")),
    };
    let mime = mime_guess::from_path(&file).first_or_octet_stream();
    let filename = file
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("download");
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref());
    if query.download.as_deref() == Some("1") {
        builder = builder.header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", filename.replace('"', "")),
        );
    }
    builder.body(Body::from(bytes)).unwrap()
}

async fn workspace_rename(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
    Json(payload): Json<WorkspaceRenamePayload>,
) -> Response<Body> {
    let rel = query.path.unwrap_or_default();
    if rel.trim().is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "path is required");
    }
    let source = match resolve_workspace_path(&state.workspace, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    if fs::metadata(&source).await.is_err() {
        return json_error(StatusCode::NOT_FOUND, "workspace item not found");
    }
    let target = match workspace_destination_path(&state.workspace, &rel, &payload.name) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    if fs::metadata(&target).await.is_ok() {
        return json_error(StatusCode::CONFLICT, "target already exists");
    }
    if let Err(err) = fs::rename(&source, &target).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot rename workspace item: {err}"),
        );
    }
    let parent = rel_parent(&rel);
    let path = rel_join(&parent, &payload.name);
    Json(serde_json::json!({"ok": true, "path": path, "name": payload.name})).into_response()
}

async fn workspace_delete(
    State(state): State<Arc<AppState>>,
    Query(query): Query<WorkspaceQuery>,
) -> Response<Body> {
    let rel = query.path.unwrap_or_default();
    if rel.trim().is_empty() {
        return json_error(StatusCode::BAD_REQUEST, "path is required");
    }
    let path = match resolve_workspace_path(&state.workspace, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    let meta = match fs::metadata(&path).await {
        Ok(meta) => meta,
        Err(_) => return json_error(StatusCode::NOT_FOUND, "workspace item not found"),
    };
    let result = if meta.is_dir() {
        fs::remove_dir_all(&path).await
    } else {
        fs::remove_file(&path).await
    };
    if let Err(err) = result {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot delete workspace item: {err}"),
        );
    }
    Json(serde_json::json!({"ok": true, "path": rel})).into_response()
}

async fn skills_list(State(state): State<Arc<AppState>>) -> Response<Body> {
    let disabled = load_disabled_skills(&state).await.unwrap_or_default();
    let mut found = HashMap::<String, (SkillInfo, PathBuf)>::new();
    for root in skill_roots(&state) {
        collect_skill_dirs(&root, &root, &disabled, &mut found);
    }
    let mut data: Vec<SkillInfo> = found.into_values().map(|(skill, _)| skill).collect();
    data.sort_by(|a, b| {
        (a.category.to_lowercase(), a.name.to_lowercase())
            .cmp(&(b.category.to_lowercase(), b.name.to_lowercase()))
    });
    Json(serde_json::json!({"object": "list", "data": data})).into_response()
}

async fn skill_files(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SkillQuery>,
) -> Response<Body> {
    let name = query.name.unwrap_or_default();
    let rel = query.path.unwrap_or_default();
    let root = match find_skill_dir(&state, &name) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::NOT_FOUND, &err.to_string()),
    };
    let dir = match resolve_skill_file_path(&root, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    let mut reader = match fs::read_dir(&dir).await {
        Ok(reader) => reader,
        Err(err) => {
            return json_error(
                StatusCode::NOT_FOUND,
                &format!("cannot read skill directory: {err}"),
            );
        }
    };
    let mut entries = Vec::new();
    while let Ok(Some(entry)) = reader.next_entry().await {
        let meta = match entry.metadata().await {
            Ok(meta) => meta,
            Err(_) => continue,
        };
        let name = entry.file_name().to_string_lossy().to_string();
        let path = rel_join(&rel, &name);
        entries.push(WorkspaceEntry {
            name,
            path,
            kind: if meta.is_dir() { "dir" } else { "file" }.to_string(),
            size: meta.is_file().then_some(meta.len()),
            modified: meta.modified().ok().and_then(system_time_string),
        });
    }
    entries.sort_by(|a, b| {
        (a.kind.as_str() != "dir", a.name.to_lowercase())
            .cmp(&(b.kind.as_str() != "dir", b.name.to_lowercase()))
    });
    Json(WorkspaceList {
        root: root.display().to_string(),
        path: rel,
        entries,
    })
    .into_response()
}

async fn skill_file(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SkillQuery>,
) -> Response<Body> {
    let name = query.name.unwrap_or_default();
    let rel = query.path.unwrap_or_else(|| "SKILL.md".to_string());
    let root = match find_skill_dir(&state, &name) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::NOT_FOUND, &err.to_string()),
    };
    let file = match resolve_skill_file_path(&root, &rel) {
        Ok(path) => path,
        Err(err) => return json_error(StatusCode::BAD_REQUEST, &err.to_string()),
    };
    let meta = match fs::metadata(&file).await {
        Ok(meta) => meta,
        Err(err) => {
            return json_error(
                StatusCode::NOT_FOUND,
                &format!("cannot stat skill file: {err}"),
            );
        }
    };
    if !meta.is_file() {
        return json_error(StatusCode::BAD_REQUEST, "path is not a file");
    }
    let bytes = match fs::read(&file).await {
        Ok(bytes) => bytes,
        Err(err) => {
            return json_error(
                StatusCode::NOT_FOUND,
                &format!("cannot read skill file: {err}"),
            );
        }
    };
    let mime = mime_guess::from_path(&file).first_or_text_plain();
    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .body(Body::from(bytes))
        .unwrap()
}

async fn skill_toggle(
    State(state): State<Arc<AppState>>,
    AxumPath(name): AxumPath<String>,
    Json(payload): Json<SkillTogglePayload>,
) -> Response<Body> {
    if find_skill_dir(&state, &name).is_err() {
        return json_error(StatusCode::NOT_FOUND, "skill not found");
    }
    match set_skill_enabled(&state, &name, payload.enabled).await {
        Ok(()) => Json(serde_json::json!({"ok": true, "name": name, "enabled": payload.enabled}))
            .into_response(),
        Err(err) => json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot toggle skill: {err}"),
        ),
    }
}

async fn memory_get(State(state): State<Arc<AppState>>) -> Response<Body> {
    let mem_dir = state.hermes_home.join("memories");
    let memory = fs::read_to_string(mem_dir.join("MEMORY.md"))
        .await
        .unwrap_or_default();
    let user = fs::read_to_string(mem_dir.join("USER.md"))
        .await
        .unwrap_or_default();
    Json(MemoryResponse { memory, user }).into_response()
}

async fn memory_put(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<MemoryPayload>,
) -> Response<Body> {
    let mem_dir = state.hermes_home.join("memories");
    if let Err(err) = fs::create_dir_all(&mem_dir).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot create memory directory: {err}"),
        );
    }
    if let Err(err) = fs::write(mem_dir.join("MEMORY.md"), payload.memory).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot write MEMORY.md: {err}"),
        );
    }
    if let Err(err) = fs::write(mem_dir.join("USER.md"), payload.user).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot write USER.md: {err}"),
        );
    }
    Json(serde_json::json!({"status":"ok"})).into_response()
}

fn skill_roots(state: &AppState) -> Vec<PathBuf> {
    let mut roots = vec![state.hermes_home.join("skills")];
    let agent_dir = env::var("HERMES_AGENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| state.hermes_home.join("hermes-agent"));
    roots.push(agent_dir.join("optional-skills"));
    roots
}

fn collect_skill_dirs(
    root: &Path,
    dir: &Path,
    disabled: &HashSet<String>,
    found: &mut HashMap<String, (SkillInfo, PathBuf)>,
) {
    let skill_md = dir.join("SKILL.md");
    if skill_md.is_file() {
        if let Ok(text) = std::fs::read_to_string(&skill_md) {
            if let Some(name) = frontmatter_value(&text, "name") {
                let description = frontmatter_value(&text, "description").unwrap_or_default();
                let category = dir
                    .parent()
                    .and_then(|p| p.strip_prefix(root).ok())
                    .and_then(|p| p.to_str())
                    .filter(|s| !s.is_empty())
                    .unwrap_or("uncategorized")
                    .to_string();
                found.entry(name.clone()).or_insert_with(|| {
                    (
                        SkillInfo {
                            name: name.clone(),
                            description,
                            category,
                            enabled: !disabled.contains(&name),
                        },
                        dir.to_path_buf(),
                    )
                });
            }
        }
        return;
    }
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in read_dir.flatten() {
        if entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false) {
            collect_skill_dirs(root, &entry.path(), disabled, found);
        }
    }
}

fn frontmatter_value(text: &str, key: &str) -> Option<String> {
    let body = text.strip_prefix("---")?.splitn(2, "---").next()?;
    for line in body.lines() {
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        if k.trim() == key {
            return Some(v.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    None
}

fn find_skill_dir(state: &AppState, name: &str) -> anyhow::Result<PathBuf> {
    let disabled = HashSet::new();
    let mut found = HashMap::<String, (SkillInfo, PathBuf)>::new();
    for root in skill_roots(state) {
        collect_skill_dirs(&root, &root, &disabled, &mut found);
    }
    found
        .remove(name)
        .map(|(_, dir)| dir)
        .ok_or_else(|| anyhow::anyhow!("skill not found"))
}

fn resolve_skill_file_path(root: &Path, rel: &str) -> anyhow::Result<PathBuf> {
    let mut clean = PathBuf::new();
    for component in Path::new(rel).components() {
        match component {
            Component::Normal(seg) => clean.push(seg),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("invalid skill file path")
            }
        }
    }
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    let candidate = root.join(clean);
    let canonical = candidate.canonicalize().unwrap_or(candidate);
    if !canonical.starts_with(&root) {
        anyhow::bail!("skill file path escapes root");
    }
    Ok(canonical)
}

async fn load_disabled_skills(state: &AppState) -> anyhow::Result<HashSet<String>> {
    let agent_dir = env::var("HERMES_AGENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| state.hermes_home.join("hermes-agent"));
    let python = env::var("HERMES_WEBUI_PYTHON").unwrap_or_else(|_| "python3".to_string());
    let script = "import json, os, sys\nsys.path.insert(0, os.environ['HERMES_AGENT_DIR'])\nfrom hermes_cli.config import load_config\nfrom hermes_cli.skills_config import get_disabled_skills\nprint(json.dumps(sorted(get_disabled_skills(load_config()))))";
    let output = timeout(
        Duration::from_secs(20),
        Command::new(python)
            .arg("-c")
            .arg(script)
            .env("HERMES_AGENT_DIR", &agent_dir)
            .env("HERMES_HOME", &state.hermes_home)
            .output(),
    )
    .await??;
    if !output.status.success() {
        anyhow::bail!(String::from_utf8_lossy(&output.stderr).to_string());
    }
    let list: Vec<String> = serde_json::from_slice(&output.stdout)?;
    Ok(list.into_iter().collect())
}

async fn set_skill_enabled(state: &AppState, name: &str, enabled: bool) -> anyhow::Result<()> {
    let agent_dir = env::var("HERMES_AGENT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| state.hermes_home.join("hermes-agent"));
    let python = env::var("HERMES_WEBUI_PYTHON").unwrap_or_else(|_| "python3".to_string());
    let script = "import os, sys\nsys.path.insert(0, os.environ['HERMES_AGENT_DIR'])\nfrom hermes_cli.config import load_config\nfrom hermes_cli.skills_config import get_disabled_skills, save_disabled_skills\nname=os.environ['SKILL_NAME']; enabled=os.environ.get('SKILL_ENABLED')=='1'\nconfig=load_config(); disabled=get_disabled_skills(config)\n(disabled.discard(name) if enabled else disabled.add(name))\nsave_disabled_skills(config, disabled)";
    let output = timeout(
        Duration::from_secs(20),
        Command::new(python)
            .arg("-c")
            .arg(script)
            .env("HERMES_AGENT_DIR", &agent_dir)
            .env("HERMES_HOME", &state.hermes_home)
            .env("SKILL_NAME", name)
            .env("SKILL_ENABLED", if enabled { "1" } else { "0" })
            .output(),
    )
    .await??;
    if !output.status.success() {
        anyhow::bail!(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(())
}

fn resolve_workspace_path(root: &Path, rel: &str) -> anyhow::Result<PathBuf> {
    let mut clean = PathBuf::new();
    for component in Path::new(rel).components() {
        match component {
            Component::Normal(seg) => clean.push(seg),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                anyhow::bail!("invalid workspace path")
            }
        }
    }
    let candidate = root.join(clean);
    let canonical = candidate.canonicalize().unwrap_or(candidate);
    if !canonical.starts_with(root) {
        anyhow::bail!("workspace path escapes root");
    }
    Ok(canonical)
}

fn workspace_destination_path(root: &Path, rel: &str, new_name: &str) -> anyhow::Result<PathBuf> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() || trimmed.contains('\0') {
        anyhow::bail!("new name must be a single file name");
    }
    let mut components = Path::new(trimmed).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => {}
        _ => anyhow::bail!("new name must be a single file name"),
    }
    let source = resolve_workspace_path(root, rel)?;
    let parent = source
        .parent()
        .ok_or_else(|| anyhow::anyhow!("workspace item has no parent"))?;
    let target = parent.join(trimmed);
    if !target.starts_with(root) {
        anyhow::bail!("workspace path escapes root");
    }
    Ok(target)
}

fn rel_parent(rel: &str) -> String {
    Path::new(rel)
        .parent()
        .and_then(|p| p.to_str())
        .filter(|p| *p != ".")
        .unwrap_or("")
        .to_string()
}

fn rel_join(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_string()
    } else {
        format!("{}/{}", parent.trim_end_matches('/'), name)
    }
}

fn system_time_string(t: SystemTime) -> Option<String> {
    let secs = t.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(secs.to_string())
}

fn valid_cookie(headers: &HeaderMap, state: &AppState) -> bool {
    let key = match state.auth_key.as_deref() {
        Some(key) if !key.is_empty() => key,
        _ => return false,
    };
    let cookie_header = headers
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    for part in cookie_header.split(';') {
        let trimmed = part.trim();
        if let Some(token) = trimmed.strip_prefix(&format!("{}=", SESSION_COOKIE))
            && verify_session_token(token, key)
        {
            return true;
        }
    }
    false
}

fn make_session_token(key: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    let payload = ts.to_string();
    let sig = sign(key, &payload);
    format!("{}.{}", payload, sig)
}

fn verify_session_token(token: &str, key: &str) -> bool {
    let Some((payload, sig)) = token.split_once('.') else {
        return false;
    };
    let Ok(ts) = payload.parse::<u64>() else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs();
    if now.saturating_sub(ts) > SESSION_TTL {
        return false;
    }
    sign(key, payload) == sig
}

fn sign(key: &str, payload: &str) -> String {
    let mut mac = HmacSha256::new_from_slice(key.as_bytes()).expect("HMAC accepts any key length");
    mac.update(payload.as_bytes());
    URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
}

fn json_error(status: StatusCode, message: &str) -> Response<Body> {
    let body = serde_json::json!({ "error": { "message": message } }).to_string();
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body))
        .unwrap()
}

fn login_html(message: &str) -> String {
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Hermes WebUI Login</title><style>
        body{{margin:0;height:100vh;display:grid;place-items:center;background:#232529;color:#dedfe3;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif}}
        form{{width:min(420px,calc(100vw - 40px));background:#2b2d32;border:1px solid #3b3e45;border-radius:18px;padding:28px;box-shadow:0 18px 48px rgba(0,0,0,.28)}}
        h1{{margin:0 0 8px;font-size:24px}}p{{color:#999ba7}}input{{width:100%;height:44px;border-radius:12px;border:1px solid #565a64;background:#232529;color:#fff;padding:0 14px;font-size:16px}}
        button{{margin-top:14px;width:100%;height:44px;border:0;border-radius:12px;background:#7ea8ff;color:#101318;font-weight:800;font-size:15px}}.err{{color:#f04e71}}
        </style></head><body><form method="post" action="/login"><h1>Hermes WebUI</h1><p>Enter the WebUI login key.</p><input name="password" type="password" autofocus autocomplete="current-password"/><button>Login</button><p class="err">{}</p></form></body></html>"#,
        html_escape(message)
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

const HEIC_GENERATION_MIN_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
struct ImageEntry {
    filename: String,
    heic_filename: Option<String>,
    image_url: String,
    png_url: String,
    heic_url: Option<String>,
    heic_status: String,
    download_filename: String,
    download_url: String,
    download_label: String,
    created_at: i64,
    modified_at: i64,
    size: u64,
}

#[derive(Debug, Clone, Copy, Deserialize)]
struct ListQuery {
    offset: Option<usize>,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
struct RefreshQuery {
    after: Option<i64>,
    limit: Option<usize>,
    check: Option<String>,
}

#[derive(Debug, Serialize)]
struct RefreshResult {
    new_items: Vec<ImageEntry>,
    checked_items: Vec<ImageEntry>,
}

#[derive(Debug, Deserialize)]
struct BatchRequest {
    filenames: Vec<String>,
}

#[derive(Debug, Serialize)]
struct BatchResult {
    success_count: usize,
    fail_count: usize,
    errors: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    download_pngs: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct ImageStats {
    total_images: usize,
    total_bytes: u64,
}

#[derive(Debug, Serialize)]
struct ImageDimensions {
    width: u32,
    height: u32,
}

#[derive(Debug, Serialize)]
struct FileMetadata {
    filename: String,
    url: String,
    size: u64,
    modified_at: i64,
}

#[derive(Debug, Serialize)]
struct PngTextChunk {
    keyword: String,
    value: String,
}

#[derive(Debug, Serialize)]
struct ImageMetadata {
    filename: String,
    dimensions: Option<ImageDimensions>,
    png: FileMetadata,
    webp: Option<FileMetadata>,
    heic: Option<FileMetadata>,
    heic_status: String,
    png_text: Vec<PngTextChunk>,
}

fn start_image_watcher(
    dir: &Path,
    tx: mpsc::UnboundedSender<PathBuf>,
) -> anyhow::Result<RecommendedWatcher> {
    let watch_dir = dir.to_path_buf();
    let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        let Ok(event) = res else {
            if let Err(err) = res {
                warn!("image directory watch error: {}", err);
            }
            return;
        };
        if !is_interesting_event(&event.kind) {
            return;
        }
        for path in event.paths {
            if is_png_heic_or_webp(&path) {
                let _ = tx.send(path);
            }
        }
    })?;
    watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;
    info!("watching {} with inotify", watch_dir.display());
    Ok(watcher)
}

const FS_EVENT_DEBOUNCE: Duration = Duration::from_millis(1500);
const FILE_STABILITY_PROBE: Duration = Duration::from_millis(250);
const FILE_STABILITY_ATTEMPTS: usize = 12;
const FILE_STABILITY_REQUEUE_ATTEMPTS: usize = 8;
type ImageFingerprint = (i64, u64, Option<String>, String, String, Option<String>);

fn is_interesting_event(kind: &EventKind) -> bool {
    matches!(
        kind,
        // CloseWrite is the important inotify signal for files written in-place:
        // do not refresh while ImageMagick/heif-enc is still transcoding.
        EventKind::Access(AccessKind::Close(AccessMode::Write))
            // Atomic-renames into the watched directory are already complete.
            | EventKind::Modify(ModifyKind::Name(_))
            // Metadata-only mtime changes (external organize/reorder tools) must
            // still refresh clients. These are also debounced before broadcast.
            | EventKind::Modify(ModifyKind::Metadata(_))
            // Keep Create and Data as fallbacks for platforms/backends that do
            // not expose close-write. process_fs_events still debounces and
            // probes for stable non-zero files before broadcasting.
            | EventKind::Create(_)
            | EventKind::Modify(ModifyKind::Data(_))
    )
}

async fn process_fs_events(
    image_dir: PathBuf,
    mut rx: mpsc::UnboundedReceiver<PathBuf>,
    updates: broadcast::Sender<String>,
) {
    let mut last_sent: HashMap<String, ImageFingerprint> = HashMap::new();
    let mut pending: HashMap<PathBuf, (Instant, usize)> = HashMap::new();

    loop {
        if pending.is_empty() {
            let Some(changed_path) = rx.recv().await else {
                break;
            };
            if let Some(png_path) = png_path_for_changed_file(&image_dir, &changed_path) {
                pending.insert(png_path, (Instant::now() + FS_EVENT_DEBOUNCE, 0));
            }
            continue;
        }

        let next_deadline = pending
            .values()
            .map(|(deadline, _)| *deadline)
            .min()
            .unwrap();
        tokio::select! {
            maybe_path = rx.recv() => {
                let Some(changed_path) = maybe_path else {
                    break;
                };
                if let Some(png_path) = png_path_for_changed_file(&image_dir, &changed_path) {
                    // Coalesce PNG + derived HEIC/WebP bursts. Every related
                    // event pushes the deadline out and resets the retry count,
                    // so one /image generation should produce one refresh after
                    // all conversions close.
                    pending.insert(png_path, (Instant::now() + FS_EVENT_DEBOUNCE, 0));
                }
            }
            _ = sleep_until(next_deadline) => {
                let now = Instant::now();
                let due: Vec<PathBuf> = pending
                    .iter()
                    .filter_map(|(path, (deadline, _))| (*deadline <= now).then_some(path.clone()))
                    .collect();
                for png_path in due {
                    let retry_count = pending.remove(&png_path).map(|(_, retry_count)| retry_count).unwrap_or(0);
                    if !wait_for_stable_image_files(&image_dir, &png_path).await {
                        if retry_count + 1 >= FILE_STABILITY_REQUEUE_ATTEMPTS {
                            warn!(
                                "dropping unstable image update after {} retries: {}",
                                retry_count + 1,
                                png_path.display()
                            );
                        } else {
                            pending.insert(png_path, (Instant::now() + FILE_STABILITY_PROBE, retry_count + 1));
                        }
                        continue;
                    }
                    let Some(entry) = image_entry_for_png(&image_dir, &png_path).await else {
                        continue;
                    };
                    let fingerprint = (
                        entry.modified_at,
                        entry.size,
                        entry.heic_filename.clone(),
                        entry.image_url.clone(),
                        entry.png_url.clone(),
                        entry.heic_url.clone(),
                    );
                    if last_sent.get(&entry.filename) == Some(&fingerprint) {
                        continue;
                    }
                    last_sent.insert(entry.filename.clone(), fingerprint);
                    let msg = serde_json::json!({"type": "image", "data": entry}).to_string();
                    let _ = updates.send(msg);
                }
            }
        }
    }
}

async fn wait_for_stable_image_files(dir: &Path, png_path: &Path) -> bool {
    for _ in 0..FILE_STABILITY_ATTEMPTS {
        let Some(first) = image_file_snapshot(dir, png_path).await else {
            return false;
        };
        if first.iter().any(|(_, size, _)| *size == 0) {
            sleep(FILE_STABILITY_PROBE).await;
            continue;
        }
        sleep(FILE_STABILITY_PROBE).await;
        let Some(second) = image_file_snapshot(dir, png_path).await else {
            continue;
        };
        if first == second {
            return true;
        }
    }
    false
}

async fn image_file_snapshot(dir: &Path, png_path: &Path) -> Option<Vec<(String, u64, i64)>> {
    let mut names = Vec::new();
    let png_name = safe_file_name_from_path(png_path)?;
    names.push(png_name);
    names.extend(find_all_heic_for_png(dir, png_path).await);
    names.extend(find_all_webp_for_png(dir, png_path).await);
    names.sort();
    names.dedup();

    let mut snapshot = Vec::with_capacity(names.len());
    for name in names {
        let metadata = tokio::fs::symlink_metadata(dir.join(&name)).await.ok()?;
        if !metadata.is_file() {
            return None;
        }
        let modified_at = system_time_to_millis(metadata.modified().unwrap_or(UNIX_EPOCH));
        snapshot.push((name, metadata.len(), modified_at));
    }
    Some(snapshot)
}

async fn list_images(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListQuery>,
) -> Result<Json<Vec<ImageEntry>>, StatusCode> {
    let mut entries = list_image_entries(&state.image_dir).await?;
    let offset = query.offset.unwrap_or(0);
    let limit = query.limit.unwrap_or(48).clamp(1, 120);
    if offset >= entries.len() {
        entries.clear();
    } else {
        entries = entries.into_iter().skip(offset).take(limit).collect();
    }
    Ok(Json(entries))
}

async fn refresh_images(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RefreshQuery>,
) -> Result<Json<RefreshResult>, StatusCode> {
    let after = query.after.unwrap_or(0);
    let limit = query.limit.unwrap_or(48).clamp(1, 120);
    let check_names = parse_check_names(query.check.as_deref());
    let mut new_items = Vec::new();
    let mut checked_items = Vec::new();
    let mut read_dir = tokio::fs::read_dir(&state.image_dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        let path = entry.path();
        if !is_source_image(&path) {
            continue;
        }
        let Some(filename) = safe_file_name_from_path(&path) else {
            continue;
        };
        let modified_at = tokio::fs::symlink_metadata(&path)
            .await
            .ok()
            .and_then(|m| m.modified().ok())
            .map(system_time_to_millis)
            .unwrap_or(0);
        if modified_at > after {
            if let Some(image_entry) = image_entry_for_png(&state.image_dir, &path).await {
                new_items.push(image_entry);
            }
        } else if check_names.contains(&filename)
            && let Some(image_entry) = image_entry_for_png(&state.image_dir, &path).await
        {
            checked_items.push(image_entry);
        }
    }

    new_items.sort_by_key(|entry| Reverse((entry.modified_at, entry.filename.clone())));
    if new_items.len() > limit {
        new_items.truncate(limit);
    }
    checked_items.sort_by_key(|entry| Reverse((entry.modified_at, entry.filename.clone())));
    Ok(Json(RefreshResult {
        new_items,
        checked_items,
    }))
}

fn parse_check_names(input: Option<&str>) -> HashSet<String> {
    input
        .unwrap_or("")
        .split(',')
        .filter_map(|name| {
            let trimmed = name.trim();
            (!trimmed.is_empty() && is_safe_filename(trimmed)).then(|| trimmed.to_string())
        })
        .take(240)
        .collect()
}

async fn image_stats(State(state): State<Arc<AppState>>) -> Result<Json<ImageStats>, StatusCode> {
    Ok(Json(compute_image_stats(&state.image_dir).await?))
}

async fn image_entry(
    State(state): State<Arc<AppState>>,
    AxumPath(filename): AxumPath<String>,
) -> Result<Json<ImageEntry>, StatusCode> {
    let path = resolve_file(
        &state.image_dir,
        &filename,
        &["png", "jpg", "jpeg", "heic", "webp"],
    )?;
    let png_path =
        png_path_for_related_file(&state.image_dir, &path).ok_or(StatusCode::NOT_FOUND)?;
    let entry = image_entry_for_png(&state.image_dir, &png_path)
        .await
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(entry))
}

async fn image_metadata(
    State(state): State<Arc<AppState>>,
    AxumPath(filename): AxumPath<String>,
) -> Result<Json<ImageMetadata>, StatusCode> {
    let path = resolve_file(
        &state.image_dir,
        &filename,
        &["png", "jpg", "jpeg", "heic", "webp"],
    )?;
    let png_path =
        png_path_for_related_file(&state.image_dir, &path).ok_or(StatusCode::NOT_FOUND)?;
    if !is_regular_file(&png_path) {
        return Err(StatusCode::NOT_FOUND);
    }
    let png_name = safe_file_name_from_path(&png_path).ok_or(StatusCode::BAD_REQUEST)?;
    let png = file_metadata(&state.image_dir, &png_name, false).ok_or(StatusCode::NOT_FOUND)?;
    let webp = find_webp_for_png(&state.image_dir, &png_path)
        .await
        .and_then(|name| file_metadata(&state.image_dir, &name, false));
    let heic = find_heic_for_png(&state.image_dir, &png_path)
        .await
        .and_then(|name| file_metadata(&state.image_dir, &name, true));
    let heic_status = heic_status_for_source(&png_path, png.size, heic.is_some()).to_string();
    let bytes = tokio::fs::read(&png_path)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let (dimensions, png_text) = parse_image_info(&png_path, &bytes);
    Ok(Json(ImageMetadata {
        filename: png_name,
        dimensions,
        png,
        webp,
        heic,
        heic_status,
        png_text,
    }))
}

fn file_metadata(dir: &Path, filename: &str, download: bool) -> Option<FileMetadata> {
    if !is_safe_filename(filename) {
        return None;
    }
    let path = dir.join(filename);
    let metadata = std::fs::symlink_metadata(&path).ok()?;
    if !metadata.is_file() {
        return None;
    }
    let modified_at = system_time_to_millis(metadata.modified().unwrap_or(UNIX_EPOCH));
    let url = if download {
        format!("/image-download/{}?v={}", filename, file_version(&metadata))
    } else {
        format!("/image-files/{}?v={}", filename, file_version(&metadata))
    };
    Some(FileMetadata {
        filename: filename.to_string(),
        url,
        size: metadata.len(),
        modified_at,
    })
}

fn source_can_generate_heic(path: &Path, source_size: u64) -> bool {
    matches!(
        path.extension().and_then(OsStr::to_str),
        Some(ext) if ext.eq_ignore_ascii_case("png")
    ) && source_size > HEIC_GENERATION_MIN_BYTES
}

fn heic_status_for_source(path: &Path, source_size: u64, has_heic: bool) -> &'static str {
    if has_heic {
        return "available";
    }
    if source_can_generate_heic(path, source_size) {
        "missing"
    } else {
        "not_applicable"
    }
}

fn source_download_label(filename: &str) -> &'static str {
    match Path::new(filename).extension().and_then(OsStr::to_str) {
        Some(ext) if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") => {
            "下载 JPEG"
        }
        Some(ext) if ext.eq_ignore_ascii_case("webp") => "下载 WebP",
        _ => "下载 PNG",
    }
}

fn parse_image_info(path: &Path, bytes: &[u8]) -> (Option<ImageDimensions>, Vec<PngTextChunk>) {
    match path.extension().and_then(OsStr::to_str) {
        Some(ext) if ext.eq_ignore_ascii_case("jpg") || ext.eq_ignore_ascii_case("jpeg") => {
            (parse_jpeg_dimensions(bytes), Vec::new())
        }
        _ => parse_png_info(bytes),
    }
}

fn parse_jpeg_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return None;
    }
    let mut pos = 2usize;
    while pos + 3 < bytes.len() {
        while pos < bytes.len() && bytes[pos] == 0xff {
            pos += 1;
        }
        let marker = *bytes.get(pos)?;
        pos += 1;
        if marker == 0xd9 || marker == 0xda {
            break;
        }
        let len = u16::from_be_bytes([*bytes.get(pos)?, *bytes.get(pos + 1)?]) as usize;
        if len < 2 || pos.checked_add(len)? > bytes.len() {
            break;
        }
        let data = &bytes[pos + 2..pos + len];
        if matches!(
            marker,
            0xc0 | 0xc1
                | 0xc2
                | 0xc3
                | 0xc5
                | 0xc6
                | 0xc7
                | 0xc9
                | 0xca
                | 0xcb
                | 0xcd
                | 0xce
                | 0xcf
        ) && data.len() >= 5
        {
            let height = u16::from_be_bytes([data[1], data[2]]) as u32;
            let width = u16::from_be_bytes([data[3], data[4]]) as u32;
            return Some(ImageDimensions { width, height });
        }
        pos += len;
    }
    None
}

fn parse_png_info(bytes: &[u8]) -> (Option<ImageDimensions>, Vec<PngTextChunk>) {
    const PNG_SIG: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 8 || &bytes[..8] != PNG_SIG {
        return (None, Vec::new());
    }
    let mut pos = 8usize;
    let mut dimensions = None;
    let mut text = Vec::new();
    let mut seen_text = HashSet::new();
    while pos.checked_add(12).is_some_and(|end| end <= bytes.len()) {
        let len = u32::from_be_bytes(bytes[pos..pos + 4].try_into().unwrap()) as usize;
        let chunk_type = &bytes[pos + 4..pos + 8];
        let data_start = pos + 8;
        let Some(data_end) = data_start.checked_add(len) else {
            break;
        };
        let Some(next_pos) = data_end.checked_add(4) else {
            break;
        };
        if next_pos > bytes.len() {
            break;
        }
        let data = &bytes[data_start..data_end];
        match chunk_type {
            b"IHDR" if data.len() >= 8 => {
                dimensions = Some(ImageDimensions {
                    width: u32::from_be_bytes(data[0..4].try_into().unwrap()),
                    height: u32::from_be_bytes(data[4..8].try_into().unwrap()),
                });
            }
            b"tEXt" => {
                if let Some((keyword, value)) = split_png_text_pair(data) {
                    push_unique_png_text(&mut text, &mut seen_text, keyword, value);
                }
            }
            b"zTXt" => {
                if let Some((keyword, value)) = parse_ztxt_chunk(data) {
                    push_unique_png_text(&mut text, &mut seen_text, keyword, value);
                }
            }
            b"iTXt" => {
                if let Some((keyword, value)) = parse_itxt_chunk(data) {
                    push_unique_png_text(&mut text, &mut seen_text, keyword, value);
                }
            }
            b"IEND" => break,
            _ => {}
        }
        pos = next_pos;
    }
    (dimensions, text)
}

fn push_unique_png_text(
    text: &mut Vec<PngTextChunk>,
    seen: &mut HashSet<(String, String)>,
    keyword: String,
    value: String,
) {
    if seen.insert((keyword.clone(), value.clone())) {
        text.push(PngTextChunk { keyword, value });
    }
}

fn split_png_text_pair(data: &[u8]) -> Option<(String, String)> {
    let nul = data.iter().position(|b| *b == 0)?;
    let keyword = png_text_lossy(&data[..nul]);
    let value = png_text_lossy(&data[nul + 1..]);
    (!keyword.is_empty()).then_some((keyword, value))
}

fn parse_ztxt_chunk(data: &[u8]) -> Option<(String, String)> {
    let nul = data.iter().position(|b| *b == 0)?;
    let keyword = png_text_lossy(&data[..nul]);
    let compressed = data.get(nul + 2..)?;
    let value = inflate_png_text(compressed)
        .unwrap_or_else(|| "[compressed text decode failed]".to_string());
    (!keyword.is_empty()).then_some((keyword, value))
}

fn parse_itxt_chunk(data: &[u8]) -> Option<(String, String)> {
    let keyword_end = data.iter().position(|b| *b == 0)?;
    let keyword = png_text_lossy(&data[..keyword_end]);
    let mut pos = keyword_end + 1;
    let compression_flag = *data.get(pos)?;
    pos += 2; // compression flag + compression method
    let lang_end = data.get(pos..)?.iter().position(|b| *b == 0)? + pos;
    pos = lang_end + 1;
    let translated_end = data.get(pos..)?.iter().position(|b| *b == 0)? + pos;
    pos = translated_end + 1;
    let raw_text = data.get(pos..)?;
    let value = if compression_flag == 1 {
        inflate_png_text(raw_text).unwrap_or_else(|| "[compressed iTXt decode failed]".to_string())
    } else {
        String::from_utf8_lossy(raw_text).to_string()
    };
    (!keyword.is_empty()).then_some((keyword, value))
}

fn inflate_png_text(data: &[u8]) -> Option<String> {
    let mut decoder = flate2::read::ZlibDecoder::new(data);
    let mut out = String::new();
    decoder.read_to_string(&mut out).ok()?;
    Some(out)
}

fn png_text_lossy(data: &[u8]) -> String {
    String::from_utf8_lossy(data).trim().to_string()
}

async fn compute_image_stats(dir: &Path) -> Result<ImageStats, StatusCode> {
    let mut total_images = 0usize;
    let mut png_stems = HashSet::new();
    let mut candidate_files: Vec<(String, u64, bool)> = Vec::new();
    let mut read_dir = tokio::fs::read_dir(dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        let path = entry.path();
        let Some(ext) = path.extension().and_then(OsStr::to_str) else {
            continue;
        };
        if !matches_image_ext(ext) {
            continue;
        }
        let Ok(metadata) = tokio::fs::symlink_metadata(&path).await else {
            continue;
        };
        if !metadata.is_file() {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(OsStr::to_str).map(str::to_owned) else {
            continue;
        };
        let len = metadata.len();
        if is_source_image_ext(ext) {
            total_images += 1;
            png_stems.insert(stem.clone());
            candidate_files.push((stem, len, true));
        } else {
            candidate_files.push((related_base_stem(&stem).to_string(), len, false));
        }
    }

    let total_bytes = candidate_files
        .into_iter()
        .filter(|(stem, _, is_png)| *is_png || png_stems.contains(stem))
        .fold(0u64, |sum, (_, len, _)| sum.saturating_add(len));

    Ok(ImageStats {
        total_images,
        total_bytes,
    })
}

fn matches_image_ext(ext: &str) -> bool {
    is_source_image_ext(ext) || ext.eq_ignore_ascii_case("heic") || ext.eq_ignore_ascii_case("webp")
}

fn related_base_stem(stem: &str) -> &str {
    stem.strip_suffix("_preview")
        .or_else(|| {
            let (base, quality) = stem.rsplit_once("_q")?;
            quality
                .chars()
                .all(|ch| ch.is_ascii_digit())
                .then_some(base)
        })
        .unwrap_or(stem)
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
                    let mut req = client.get(format!("{}/api/sessions/{}/messages?limit=8", api_url, session_id));
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

async fn image_events(
    State(state): State<Arc<AppState>>,
) -> Sse<impl futures_core::Stream<Item = Result<SseEvent, Infallible>>> {
    let mut rx_images = state.updates.subscribe();
    let mut rx_deletes = state.deletes.subscribe();
    let stream = async_stream::stream! {
        loop {
            tokio::select! {
                Ok(text) = rx_images.recv() => {
                    yield Ok(SseEvent::default().data(text));
                }
                Ok(text) = rx_deletes.recv() => {
                    yield Ok(SseEvent::default().event("delete").data(text));
                }
                else => break,
            }
        }
    };
    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn list_image_entries(dir: &Path) -> Result<Vec<ImageEntry>, StatusCode> {
    let mut entries = Vec::new();
    let mut read_dir = tokio::fs::read_dir(dir)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    {
        let path = entry.path();
        if !is_source_image(&path) {
            continue;
        }
        if let Some(image_entry) = image_entry_for_png(dir, &path).await {
            entries.push(image_entry);
        }
    }

    // Sort by the PNG file's timestamp. `created_at` can be misleading after
    // copies/restores or filesystem moves, while `modified_at` tracks when the
    // preview PNG was actually written/updated.
    entries.sort_by_key(|entry| Reverse((entry.modified_at, entry.filename.clone())));
    Ok(entries)
}

async fn image_entry_for_png(dir: &Path, png_path: &Path) -> Option<ImageEntry> {
    if !is_source_image(png_path) {
        return None;
    }
    let metadata = tokio::fs::symlink_metadata(png_path).await.ok()?;
    if !metadata.is_file() {
        return None;
    }
    let filename = safe_file_name_from_path(png_path)?;
    let heic_filename = find_heic_for_png(dir, png_path).await;
    let webp_filename = find_webp_for_png(dir, png_path).await;
    let display_filename = webp_filename.as_ref().unwrap_or(&filename);
    let created_at = system_time_to_millis(
        metadata
            .created()
            .unwrap_or_else(|_| metadata.modified().unwrap_or(UNIX_EPOCH)),
    );
    let modified_at = system_time_to_millis(metadata.modified().unwrap_or(UNIX_EPOCH));
    let png_version = file_version(&metadata);
    let display_version = if display_filename == &filename {
        png_version.clone()
    } else {
        tokio::fs::symlink_metadata(dir.join(display_filename))
            .await
            .ok()
            .map(|m| file_version(&m))
            .unwrap_or_else(|| png_version.clone())
    };
    let heic_url = if let Some(name) = heic_filename.as_ref() {
        let version = tokio::fs::symlink_metadata(dir.join(name))
            .await
            .ok()
            .map(|m| file_version(&m))
            .unwrap_or_else(|| png_version.clone());
        Some(format!("/image-download/{}?v={}", name, version))
    } else {
        None
    };
    let png_url = format!("/image-files/{}?v={}", filename, png_version);
    let heic_status =
        heic_status_for_source(png_path, metadata.len(), heic_url.is_some()).to_string();
    let (download_filename, download_url, download_label) =
        if let (Some(name), Some(url)) = (heic_filename.as_ref(), heic_url.as_ref()) {
            (name.clone(), url.clone(), "下载 HEIC".to_string())
        } else if heic_status == "not_applicable" {
            (
                filename.clone(),
                png_url.clone(),
                source_download_label(&filename).to_string(),
            )
        } else {
            (filename.clone(), png_url.clone(), "生成 HEIC".to_string())
        };
    Some(ImageEntry {
        image_url: format!("/image-files/{}?v={}", display_filename, display_version),
        png_url,
        heic_url,
        heic_status,
        download_filename,
        download_url,
        download_label,
        filename,
        heic_filename,
        created_at,
        modified_at,
        size: metadata.len(),
    })
}

async fn serve_png(
    State(state): State<Arc<AppState>>,
    AxumPath(filename): AxumPath<String>,
) -> axum::response::Response {
    match resolve_file(&state.image_dir, &filename, &["png", "jpg", "jpeg", "webp"]) {
        Ok(path) => serve_local_file(path, false).await,
        Err(status) => status.into_response(),
    }
}

async fn download_heic(
    State(state): State<Arc<AppState>>,
    AxumPath(filename): AxumPath<String>,
) -> axum::response::Response {
    match resolve_file(&state.image_dir, &filename, &["heic"]) {
        Ok(path) => serve_local_file(path, true).await,
        Err(status) => status.into_response(),
    }
}

async fn generate_heic(
    State(state): State<Arc<AppState>>,
    AxumPath(filename): AxumPath<String>,
) -> Result<Json<ImageEntry>, (StatusCode, String)> {
    let png_path = resolve_file(&state.image_dir, &filename, &["png"]).map_err(|status| {
        if resolve_file(&state.image_dir, &filename, &["jpg", "jpeg"]).is_ok() {
            (
                StatusCode::BAD_REQUEST,
                "HEIC generation is not supported for JPEG sources".to_string(),
            )
        } else {
            (status, "source image file not found".to_string())
        }
    })?;

    let source_metadata = tokio::fs::symlink_metadata(&png_path).await.map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            "source image file not found".to_string(),
        )
    })?;
    if !source_metadata.is_file() {
        return Err((
            StatusCode::NOT_FOUND,
            "source image file not found".to_string(),
        ));
    }
    if !source_can_generate_heic(&png_path, source_metadata.len()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "HEIC generation is only supported for PNG sources larger than {} bytes",
                HEIC_GENERATION_MIN_BYTES
            ),
        ));
    }

    if find_heic_for_png(&state.image_dir, &png_path)
        .await
        .is_none()
    {
        let script = heic_conversion_script();
        if !is_regular_file(&script) {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("HEIC conversion script not found: {}", script.display()),
            ));
        }
        let stem = png_path
            .file_stem()
            .and_then(OsStr::to_str)
            .ok_or_else(|| (StatusCode::BAD_REQUEST, "invalid PNG filename".to_string()))?;
        let output = png_path.with_file_name(format!("{stem}_q82.heic"));
        let convert = Command::new("bash")
            .arg(&script)
            .args(["-q", "82"])
            .arg(&png_path)
            .arg(&output)
            .output();
        let completed = timeout(Duration::from_secs(300), convert)
            .await
            .map_err(|_| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "HEIC conversion timed out".to_string(),
                )
            })?
            .map_err(|err| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("failed to run HEIC conversion: {err}"),
                )
            })?;
        if !completed.status.success() {
            let stderr = String::from_utf8_lossy(&completed.stderr)
                .trim()
                .to_string();
            let stdout = String::from_utf8_lossy(&completed.stdout)
                .trim()
                .to_string();
            let detail = if !stderr.is_empty() {
                stderr
            } else if !stdout.is_empty() {
                stdout
            } else {
                format!("exit code {}", completed.status)
            };
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("HEIC conversion failed: {detail}"),
            ));
        }
        if !is_regular_file(&output) {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                format!(
                    "HEIC conversion completed but output is missing: {}",
                    output.display()
                ),
            ));
        }
    }

    let entry = image_entry_for_png(&state.image_dir, &png_path)
        .await
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "source image file not found".to_string(),
            )
        })?;
    let msg = serde_json::json!({"type": "image", "data": entry.clone()}).to_string();
    let _ = state.updates.send(msg);
    Ok(Json(entry))
}

fn heic_conversion_script() -> PathBuf {
    if let Ok(path) = env::var("HERMES_WEBUI_HEIC_SCRIPT") {
        let candidate = PathBuf::from(path);
        if is_regular_file(&candidate) {
            return candidate;
        }
    }
    if let Ok(exe) = env::current_exe()
        && let Some(bin_dir) = exe.parent()
    {
        let share_script = bin_dir
            .parent()
            .unwrap_or(bin_dir)
            .join("share/yet-another-hermes-ui/scripts/png-to-ios-heic.sh");
        if is_regular_file(&share_script) {
            return share_script;
        }
    }
    if let Ok(cwd) = env::current_dir() {
        let project_script = cwd.join("scripts/png-to-ios-heic.sh");
        if is_regular_file(&project_script) {
            return project_script;
        }
    }
    if let Ok(home) = env::var("HERMES_HOME") {
        let hermes_script = PathBuf::from(home).join("scripts/png-to-ios-heic.sh");
        if is_regular_file(&hermes_script) {
            return hermes_script;
        }
    }
    if let Ok(home) = env::var("HOME") {
        let hermes_script = PathBuf::from(home).join(".hermes/scripts/png-to-ios-heic.sh");
        if is_regular_file(&hermes_script) {
            return hermes_script;
        }
    }
    // Fallback: look relative to the binary or next to the installed share dir
    let cwd = std::env::current_dir().unwrap_or_default();
    let share_path = cwd.join("scripts/png-to-ios-heic.sh");
    if is_regular_file(&share_path) {
        return share_path;
    }
    PathBuf::from("/usr/local/share/yet-another-hermes-ui/scripts/png-to-ios-heic.sh")
}

async fn serve_local_file(path: PathBuf, attachment: bool) -> axum::response::Response {
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    };
    let display_name = path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("image")
        .replace('"', "");
    let mut headers = HeaderMap::new();
    let content_type = mime_guess::from_path(&path)
        .first_or_octet_stream()
        .to_string();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&content_type).unwrap(),
    );
    if attachment {
        headers.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&format!("attachment; filename=\"{}\"", display_name)).unwrap(),
        );
    } else {
        headers.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=86400, immutable"),
        );
    }
    (headers, bytes).into_response()
}

fn file_version(metadata: &std::fs::Metadata) -> String {
    let modified_at = system_time_to_millis(metadata.modified().unwrap_or(UNIX_EPOCH));
    format!("{}-{}", modified_at, metadata.len())
}

fn is_source_image(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(is_source_image_ext)
        .unwrap_or(false)
}

fn is_source_image_ext(ext: &str) -> bool {
    ext.eq_ignore_ascii_case("png")
        || ext.eq_ignore_ascii_case("jpg")
        || ext.eq_ignore_ascii_case("jpeg")
}

fn is_png_heic_or_webp(path: &Path) -> bool {
    path.extension()
        .and_then(OsStr::to_str)
        .map(|ext| {
            is_source_image_ext(ext)
                || ext.eq_ignore_ascii_case("heic")
                || ext.eq_ignore_ascii_case("webp")
        })
        .unwrap_or(false)
}

fn png_path_for_changed_file(dir: &Path, changed_path: &Path) -> Option<PathBuf> {
    png_path_for_related_file(dir, changed_path)
}

fn safe_file_name_from_path(path: &Path) -> Option<String> {
    path.file_name()
        .and_then(OsStr::to_str)
        .filter(|name| is_safe_filename(name))
        .map(str::to_owned)
}

fn is_safe_filename(name: &str) -> bool {
    !name.is_empty()
        && !name.contains('/')
        && !name.contains('\\')
        && name != "."
        && name != ".."
        && !name.contains('\0')
}

fn decode_filename(input: &str) -> Result<String, StatusCode> {
    let decoded = percent_decode_str(input)
        .decode_utf8()
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .to_string();
    if is_safe_filename(&decoded) {
        Ok(decoded)
    } else {
        Err(StatusCode::BAD_REQUEST)
    }
}

fn resolve_file(dir: &Path, filename: &str, allowed_exts: &[&str]) -> Result<PathBuf, StatusCode> {
    let decoded = decode_filename(filename)?;
    let path = dir.join(&decoded);
    let ext_ok = path
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| {
            allowed_exts
                .iter()
                .any(|allowed| ext.eq_ignore_ascii_case(allowed))
        })
        .unwrap_or(false);
    if !ext_ok {
        return Err(StatusCode::BAD_REQUEST);
    }
    if is_regular_file(&path) {
        Ok(path)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

async fn find_heic_for_png(dir: &Path, png_path: &Path) -> Option<String> {
    find_preferred_related_for_png_by_ext(dir, png_path, "heic", &["", "_q82"])
}

async fn find_all_heic_for_png(dir: &Path, png_path: &Path) -> Vec<String> {
    find_related_for_png_by_ext(dir, png_path, "heic", &["", "_q82"]).await
}

fn find_preferred_related_for_png_by_ext(
    dir: &Path,
    png_path: &Path,
    ext: &str,
    preferred_suffixes: &[&str],
) -> Option<String> {
    let stem = png_path.file_stem().and_then(OsStr::to_str)?;
    for suffix in preferred_suffixes {
        let path = dir.join(format!("{stem}{suffix}.{ext}"));
        if is_regular_file(&path)
            && let Some(name) = safe_file_name_from_path(&path)
        {
            return Some(name);
        }
    }
    None
}

async fn find_related_for_png_by_ext(
    dir: &Path,
    png_path: &Path,
    ext: &str,
    preferred_suffixes: &[&str],
) -> Vec<String> {
    let Some(stem) = png_path.file_stem().and_then(OsStr::to_str) else {
        return Vec::new();
    };
    let mut seen = std::collections::HashSet::new();
    let mut names = Vec::new();

    for suffix in preferred_suffixes {
        let path = dir.join(format!("{stem}{suffix}.{ext}"));
        if is_regular_file(&path)
            && let Some(name) = safe_file_name_from_path(&path)
            && seen.insert(name.clone())
        {
            names.push(name);
        }
    }

    let prefix = format!("{stem}_");
    let mut matches = Vec::new();
    let Ok(mut read_dir) = tokio::fs::read_dir(dir).await else {
        return names;
    };
    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let path = entry.path();
        let Some(name) = safe_file_name_from_path(&path) else {
            continue;
        };
        if name.starts_with(&prefix)
            && path
                .extension()
                .and_then(OsStr::to_str)
                .map(|path_ext| path_ext.eq_ignore_ascii_case(ext))
                .unwrap_or(false)
        {
            let Ok(metadata) = tokio::fs::symlink_metadata(&path).await else {
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            let modified = metadata.modified().map(system_time_to_millis).unwrap_or(0);
            matches.push((modified, name));
        }
    }
    matches.sort_by_key(|(modified, _)| Reverse(*modified));
    for (_, name) in matches {
        if seen.insert(name.clone()) {
            names.push(name);
        }
    }
    names
}

async fn find_webp_for_png(dir: &Path, png_path: &Path) -> Option<String> {
    find_preferred_related_for_png_by_ext(dir, png_path, "webp", &["", "_preview"])
}

async fn find_all_webp_for_png(dir: &Path, png_path: &Path) -> Vec<String> {
    find_related_for_png_by_ext(dir, png_path, "webp", &["", "_preview"]).await
}

fn png_path_for_related_file(dir: &Path, path: &Path) -> Option<PathBuf> {
    let ext = path.extension()?.to_str()?;
    if is_source_image_ext(ext) {
        return Some(path.to_path_buf());
    }
    let stem = path.file_stem()?.to_str()?;
    let png_stem = stem
        .strip_suffix("_preview")
        .or_else(|| {
            let (base, quality) = stem.rsplit_once("_q")?;
            quality
                .chars()
                .all(|ch| ch.is_ascii_digit())
                .then_some(base)
        })
        .unwrap_or(stem);
    for source_ext in ["png", "jpg", "jpeg"] {
        let source_path = dir.join(format!("{png_stem}.{source_ext}"));
        if is_regular_file(&source_path) {
            return Some(source_path);
        }
    }
    None
}

async fn related_files_for_image(dir: &Path, path: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if is_regular_file(path) {
        files.push(path.to_path_buf());
    }
    let png_path = png_path_for_related_file(dir, path);
    if let Some(png) = png_path.as_ref() {
        if is_regular_file(png) {
            files.push(png.to_path_buf());
        }
        for heic_name in find_all_heic_for_png(dir, png).await {
            let heic_path = dir.join(heic_name);
            if is_regular_file(&heic_path) {
                files.push(heic_path);
            }
        }
        for webp_name in find_all_webp_for_png(dir, png).await {
            let webp_path = dir.join(webp_name);
            if is_regular_file(&webp_path) {
                files.push(webp_path);
            }
        }
    }
    let mut seen = std::collections::HashSet::new();
    files
        .into_iter()
        .filter(|p| {
            let Some(name) = safe_file_name_from_path(p) else {
                return false;
            };
            seen.insert(name)
        })
        .collect()
}

fn delete_event_filename(dir: &Path, requested: &str, path: &Path) -> String {
    if let Some(png) = png_path_for_related_file(dir, path)
        && let Some(name) = safe_file_name_from_path(&png)
    {
        return name;
    }
    requested.to_string()
}

fn system_time_to_millis(t: SystemTime) -> i64 {
    t.duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

fn is_regular_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

async fn delete_image(
    State(state): State<Arc<AppState>>,
    AxumPath(filename): AxumPath<String>,
) -> Result<Json<BatchResult>, StatusCode> {
    let dir = &state.image_dir;
    let mut errors = Vec::new();
    let target = resolve_file_or_not(dir, &filename, &["png", "jpg", "jpeg", "heic", "webp"]);
    let mut success = false;
    let mut event_filename = filename.clone();
    if let Ok(path) = target {
        event_filename = delete_event_filename(dir, &filename, &path);
        let related = related_files_for_image(dir, &path).await;
        if related.is_empty() {
            errors.push(format!("file not found: {}", filename));
        }
        for file in related {
            match tokio::fs::remove_file(&file).await {
                Ok(_) => success = true,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => errors.push(format!("delete {}: {}", file.display(), e)),
            }
        }
    } else {
        errors.push(format!("file not found: {}", filename));
    }
    if success {
        let msg = serde_json::json!({"filename": event_filename}).to_string();
        let _ = state.deletes.send(msg);
    }
    Ok(Json(BatchResult {
        success_count: if success { 1 } else { 0 },
        fail_count: errors.len(),
        errors,
        download_pngs: None,
    }))
}

fn resolve_file_or_not(dir: &Path, filename: &str, allowed_exts: &[&str]) -> Result<PathBuf, ()> {
    let decoded = percent_decode_str(filename)
        .decode_utf8()
        .map_err(|_| ())?
        .to_string();
    if !is_safe_filename(&decoded) {
        return Err(());
    }
    let path = dir.join(&decoded);
    let ext_ok = path
        .extension()
        .and_then(OsStr::to_str)
        .map(|ext| allowed_exts.iter().any(|a| ext.eq_ignore_ascii_case(a)))
        .unwrap_or(false);
    if !ext_ok || !is_regular_file(&path) {
        return Err(());
    }
    Ok(path)
}

async fn batch_delete(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BatchRequest>,
) -> Json<BatchResult> {
    let dir = &state.image_dir;
    let mut success_count = 0usize;
    let mut fail_count = 0usize;
    let mut errors = Vec::new();
    let mut deleted_names = Vec::new();

    for filename in &req.filenames {
        let target = resolve_file_or_not(dir, filename, &["png", "jpg", "jpeg", "heic", "webp"]);
        match target {
            Ok(path) => {
                let event_filename = delete_event_filename(dir, filename, &path);
                let related = related_files_for_image(dir, &path).await;
                let mut deleted_any = false;
                for file in related {
                    match tokio::fs::remove_file(&file).await {
                        Ok(_) => deleted_any = true,
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => errors.push(format!("delete {}: {}", file.display(), e)),
                    }
                }
                if deleted_any {
                    success_count += 1;
                    deleted_names.push(event_filename);
                } else {
                    fail_count += 1;
                    errors.push(format!("file not found: {}", filename));
                }
            }
            Err(_) => {
                errors.push(format!("file not found: {}", filename));
                fail_count += 1;
            }
        }
    }

    if !deleted_names.is_empty() {
        let msg = serde_json::json!({"filenames": deleted_names}).to_string();
        let _ = state.deletes.send(msg);
    }

    Json(BatchResult {
        success_count,
        fail_count,
        errors,
        download_pngs: None,
    })
}

async fn batch_mtime(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BatchRequest>,
) -> Json<BatchResult> {
    let dir = &state.image_dir;
    let mut ordered = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut errors = Vec::new();
    let mut fail_count = 0usize;

    for filename in &req.filenames {
        let target = resolve_file_or_not(dir, filename, &["png", "jpg", "jpeg", "heic", "webp"]);
        match target {
            Ok(path) => {
                let png_path = match png_path_for_related_file(dir, &path) {
                    Some(path) if is_source_image(&path) && is_regular_file(&path) => path,
                    _ => {
                        fail_count += 1;
                        errors.push(format!("png not found for: {}", filename));
                        continue;
                    }
                };
                let Some(png_name) = safe_file_name_from_path(&png_path) else {
                    fail_count += 1;
                    errors.push(format!("unsafe filename: {}", filename));
                    continue;
                };
                if !seen.insert(png_name.clone()) {
                    continue;
                }
                match tokio::fs::metadata(&png_path).await {
                    Ok(metadata) if metadata.is_file() => {
                        let modified =
                            system_time_to_millis(metadata.modified().unwrap_or(UNIX_EPOCH));
                        ordered.push((png_name, png_path, modified));
                    }
                    _ => {
                        fail_count += 1;
                        errors.push(format!("file not found: {}", filename));
                    }
                }
            }
            Err(_) => {
                fail_count += 1;
                errors.push(format!("file not found: {}", filename));
            }
        }
    }

    if ordered.is_empty() {
        return Json(BatchResult {
            success_count: 0,
            fail_count,
            errors,
            download_pngs: None,
        });
    }

    // The request order is the current visual order (newest → oldest). Use the
    // oldest selected PNG timestamp as the base, then assign +0.01s going backwards
    // through that visual order so the selected images keep the same sort order
    // without spreading a large selected batch across many seconds.
    let base_millis = ordered.iter().map(|(_, _, m)| *m).min().unwrap_or(0);
    let count = ordered.len();
    let mut success_count = 0usize;

    for (idx, (png_name, png_path, _)) in ordered.iter().enumerate() {
        let target_millis =
            base_millis.saturating_add(((count - 1 - idx) as i64).saturating_mul(10));
        let related = related_files_for_image(dir, png_path).await;
        let mut updated_any = false;
        let mut failed_any = false;
        for file in related {
            match set_file_mtime_millis(&file, target_millis) {
                Ok(_) => updated_any = true,
                Err(e) => {
                    failed_any = true;
                    errors.push(format!("set mtime {}: {}", file.display(), e));
                }
            }
        }
        if updated_any && !failed_any {
            success_count += 1;
        } else {
            fail_count += 1;
            if !updated_any {
                errors.push(format!("file not found: {}", png_name));
            }
        }
    }

    if success_count > 0 {
        let msg = serde_json::json!({"type": "resync"}).to_string();
        let _ = state.updates.send(msg);
    }

    Json(BatchResult {
        success_count,
        fail_count,
        errors,
        download_pngs: None,
    })
}

fn set_file_mtime_millis(path: &Path, millis: i64) -> std::io::Result<()> {
    let metadata = std::fs::metadata(path)?;
    let atime = metadata
        .accessed()
        .map(FileTime::from_system_time)
        .unwrap_or_else(|_| FileTime::from_unix_time(0, 0));
    let mtime = filetime_from_millis(millis);
    filetime::set_file_times(path, atime, mtime)
}

fn filetime_from_millis(millis: i64) -> FileTime {
    let secs = millis.div_euclid(1000);
    let nanos = (millis.rem_euclid(1000) as u32) * 1_000_000;
    FileTime::from_unix_time(secs, nanos)
}

async fn batch_download(
    State(state): State<Arc<AppState>>,
    Json(req): Json<BatchRequest>,
) -> axum::response::Response {
    let dir = &state.image_dir;
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    for filename in &req.filenames {
        if resolve_file_or_not(dir, filename, &["png", "jpg", "jpeg", "heic"]).is_ok() {
            let (_stem, ext) = split_stem_ext(filename);
            let chosen = if ext.eq_ignore_ascii_case("heic") {
                filename.clone()
            } else {
                find_heic_for_png(dir, &dir.join(filename))
                    .await
                    .unwrap_or_else(|| filename.clone())
            };
            let Ok(path) = resolve_file_or_not(dir, &chosen, &["png", "jpg", "jpeg", "heic"])
            else {
                continue;
            };
            let Ok(bytes) = tokio::fs::read(&path).await else {
                continue;
            };
            files.push((chosen, bytes));
        }
    }
    if files.is_empty() {
        return (StatusCode::BAD_REQUEST, "no valid files selected").into_response();
    }

    match build_zip_store(&files) {
        Ok(bytes) => {
            let mut headers = HeaderMap::new();
            headers.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/zip"),
            );
            headers.insert(
                header::CONTENT_DISPOSITION,
                HeaderValue::from_str("attachment; filename=\"hermes_batch.zip\"").unwrap(),
            );
            (headers, bytes).into_response()
        }
        Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, err).into_response(),
    }
}

fn build_zip_store(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        if !is_safe_filename(name) {
            return Err(format!("unsafe filename: {name}"));
        }
        let name_bytes = name.as_bytes();
        let size =
            u32::try_from(data.len()).map_err(|_| format!("file too large for zip: {name}"))?;
        let offset = u32::try_from(out.len()).map_err(|_| "zip too large".to_string())?;
        let crc = crc32fast::hash(data);
        write_u32(&mut out, 0x04034b50);
        write_u16(&mut out, 20);
        write_u16(&mut out, 0);
        write_u16(&mut out, 0);
        write_u16(&mut out, 0);
        write_u16(&mut out, 0);
        write_u32(&mut out, crc);
        write_u32(&mut out, size);
        write_u32(&mut out, size);
        write_u16(
            &mut out,
            u16::try_from(name_bytes.len()).map_err(|_| format!("filename too long: {name}"))?,
        );
        write_u16(&mut out, 0);
        out.extend_from_slice(name_bytes);
        out.extend_from_slice(data);

        write_u32(&mut central, 0x02014b50);
        write_u16(&mut central, 20);
        write_u16(&mut central, 20);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u32(&mut central, crc);
        write_u32(&mut central, size);
        write_u32(&mut central, size);
        write_u16(
            &mut central,
            u16::try_from(name_bytes.len()).map_err(|_| format!("filename too long: {name}"))?,
        );
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u16(&mut central, 0);
        write_u32(&mut central, 0);
        write_u32(&mut central, offset);
        central.extend_from_slice(name_bytes);
    }
    let central_offset = u32::try_from(out.len()).map_err(|_| "zip too large".to_string())?;
    let central_size = u32::try_from(central.len()).map_err(|_| "zip too large".to_string())?;
    out.extend_from_slice(&central);
    write_u32(&mut out, 0x06054b50);
    write_u16(&mut out, 0);
    write_u16(&mut out, 0);
    write_u16(
        &mut out,
        u16::try_from(files.len()).map_err(|_| "too many files for zip".to_string())?,
    );
    write_u16(
        &mut out,
        u16::try_from(files.len()).map_err(|_| "too many files for zip".to_string())?,
    );
    write_u32(&mut out, central_size);
    write_u32(&mut out, central_offset);
    write_u16(&mut out, 0);
    Ok(out)
}

fn write_u16(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn split_stem_ext(filename: &str) -> (&str, &str) {
    if let Some(dot) = filename.rfind('.') {
        let (stem, ext) = filename.split_at(dot);
        (stem, &ext[1..])
    } else {
        (filename, "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_token_round_trip_accepts_current_key() {
        let token = make_session_token("webui-secret");

        assert!(verify_session_token(&token, "webui-secret"));
        assert!(!verify_session_token(&token, "other-secret"));
    }

    #[test]
    fn frontmatter_value_reads_yaml_after_opening_blank_line() {
        let text =
            "---\nname: hermes-dashboard-webui\ndescription: \"Dashboard UI\"\n---\n# Body\n";

        assert_eq!(
            frontmatter_value(text, "name"),
            Some("hermes-dashboard-webui".to_string())
        );
        assert_eq!(
            frontmatter_value(text, "description"),
            Some("Dashboard UI".to_string())
        );
    }

    #[test]
    fn workspace_path_rejects_parent_escape() {
        let root = std::env::temp_dir().join(format!(
            "hermes-webui-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();

        let err = resolve_workspace_path(&root, "../outside.txt").unwrap_err();

        assert!(err.to_string().contains("invalid workspace path"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_path_resolves_child_file() {
        let root = std::env::temp_dir().join(format!(
            "hermes-webui-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos()
        ));
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("note.txt"), "hello").unwrap();

        let resolved = resolve_workspace_path(&root, "nested/note.txt").unwrap();

        assert_eq!(resolved, nested.join("note.txt").canonicalize().unwrap());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_destination_keeps_rename_inside_parent_directory() {
        let root = std::env::temp_dir().join(format!(
            "hermes-webui-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos()
        ));
        let nested = root.join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("old.txt"), "hello").unwrap();

        let target = workspace_destination_path(&root, "nested/old.txt", "new.txt").unwrap();

        assert_eq!(target, nested.join("new.txt"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn workspace_destination_rejects_path_like_new_names() {
        let root = std::env::temp_dir().join(format!(
            "hermes-webui-test-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or(Duration::ZERO)
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("old.txt"), "hello").unwrap();

        let err = workspace_destination_path(&root, "old.txt", "../bad.txt").unwrap_err();

        assert!(
            err.to_string()
                .contains("new name must be a single file name")
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn proxy_header_filter_removes_browser_origin_headers() {
        assert!(!should_forward_proxy_header("origin"));
        assert!(!should_forward_proxy_header("referer"));
        assert!(!should_forward_proxy_header("sec-fetch-site"));
        assert!(!should_forward_proxy_header("sec-ch-ua-platform"));
        assert!(!should_forward_proxy_header("cookie"));
        assert!(should_forward_proxy_header("content-type"));
        assert!(should_forward_proxy_header("accept"));
    }

    #[test]
    fn zip_store_contains_selected_image_file() {
        let bytes =
            build_zip_store(&[("sample.png".to_string(), b"image-bytes".to_vec())]).unwrap();

        assert!(bytes.starts_with(b"PK\x03\x04"));
        assert!(
            bytes
                .windows("sample.png".len())
                .any(|w| w == b"sample.png")
        );
        assert!(bytes.ends_with(&[0, 0]));
    }

    fn test_app_state(api_url: String, root: &Path) -> AppState {
        let (updates, _) = broadcast::channel::<String>(1);
        let (deletes, _) = broadcast::channel::<String>(1);
        AppState {
            client: reqwest::Client::new(),
            api_url,
            api_key: None,
            auth_key: None,
            insecure: true,
            workspace: root.to_path_buf(),
            hermes_home: root.to_path_buf(),
            image_dir: root.to_path_buf(),
            updates,
            deletes,
            model_cache: Arc::new(RwLock::new(ModelCache::default())),
        }
    }

    #[tokio::test]
    async fn session_search_uses_api_server_list_endpoint_without_state_db() {
        use std::collections::HashMap;

        async fn api_sessions(
            Query(query): Query<HashMap<String, String>>,
        ) -> Json<serde_json::Value> {
            assert_eq!(
                query.get("include_children").map(String::as_str),
                Some("false")
            );
            assert_eq!(query.get("offset").map(String::as_str), Some("0"));
            Json(serde_json::json!({
                "object": "list",
                "data": [
                    {"id":"s1","source":"telegram","model":"minimax/m3","title":"MiniMax billing","preview":"token cache math","started_at":1.0,"message_count":1},
                    {"id":"tool1","source":"tool","model":"minimax/m3","title":"Tool internal","preview":"cache","started_at":2.0,"message_count":1},
                    {"id":"s2","source":"api_server","model":"gpt-5.5","title":"Other","preview":"unrelated","started_at":3.0,"message_count":1}
                ],
                "has_more": false
            }))
        }

        let app = Router::new().route("/api/sessions", get(api_sessions));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state(format!("http://{addr}"), temp.path());

        let rows = fetch_sessions_from_api_server(&state, "cache", 10)
            .await
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "s1");
        assert_eq!(rows[0]["model"], "minimax/m3");
        assert_eq!(rows[0]["preview"], "token cache math");
    }

    #[tokio::test]
    async fn session_search_uses_api_server_messages_when_list_preview_does_not_match() {
        use std::collections::HashMap;

        async fn api_sessions(
            Query(query): Query<HashMap<String, String>>,
        ) -> Json<serde_json::Value> {
            assert_eq!(
                query.get("include_children").map(String::as_str),
                Some("false")
            );
            Json(serde_json::json!({
                "object": "list",
                "data": [
                    {"id":"s1","source":"telegram","model":"minimax/m3","title":"MiniMax billing","preview":"first prompt","started_at":1.0,"message_count":2},
                    {"id":"s2","source":"api_server","model":"gpt-5.5","title":"Other","preview":"unrelated","started_at":3.0,"message_count":1}
                ],
                "has_more": false
            }))
        }

        async fn api_messages(AxumPath(session_id): AxumPath<String>) -> Json<serde_json::Value> {
            let data = if session_id == "s1" {
                serde_json::json!([{ "id": 1, "role": "user", "content": "please do token cache math" }])
            } else {
                serde_json::json!([{ "id": 2, "role": "user", "content": "unrelated" }])
            };
            Json(serde_json::json!({"object":"list","data":data}))
        }

        let app = Router::new()
            .route("/api/sessions", get(api_sessions))
            .route("/api/sessions/{session_id}/messages", get(api_messages));
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let temp = tempfile::tempdir().unwrap();
        let state = test_app_state(format!("http://{addr}"), temp.path());

        let rows = fetch_sessions_from_api_server(&state, "cache", 10)
            .await
            .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["id"], "s1");
    }
}
