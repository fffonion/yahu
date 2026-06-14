use std::{
    cmp::Reverse,
    collections::{BTreeMap, HashMap, HashSet},
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
    routing::{any, delete, get, patch, post},
};
use base64::{
    Engine as _,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use clap::Parser;
use filetime::FileTime;
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
use yet_another_hermes_ui::{ModelCache, fresh_model_cache_body, model_cache_payload_from_source};

type HmacSha256 = Hmac<Sha256>;

static ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/dist");
const SESSION_COOKIE: &str = "hermes_webui_session";
const MAX_PROXY_BODY: usize = 32 * 1024 * 1024;
const SESSION_TTL: u64 = 7 * 24 * 60 * 60;
const SESSION_REFRESH_AFTER: u64 = SESSION_TTL / 2;
const API_MESSAGE_WATCH_WINDOW: usize = 80;
const CHAT_STREAM_BROADCAST_CAPACITY: usize = 32;

fn path_segment(value: &str) -> String {
    utf8_percent_encode(value, NON_ALPHANUMERIC)
        .to_string()
        .replace("%2D", "-")
        .replace("%2E", ".")
        .replace("%5F", "_")
        .replace("%7E", "~")
}

#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "Yet Another Hermes UI — single-binary web interface for Hermes Agent"
)]
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

    #[arg(long, env = "HERMES_WEBUI_IMAGE_DIR")]
    image_dir: Option<PathBuf>,

    #[arg(
        long,
        env = "HERMES_WEBUI_MODELS_DEV_URL",
        default_value = "https://models.dev/api.json"
    )]
    models_dev_url: String,

    #[arg(long, env = "YAHU_GITHUB_REPO", default_value = "fffonion/yahu")]
    github_repo: String,
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
    chat_streams: broadcast::Sender<String>,
    active_chat_streams: Arc<RwLock<HashMap<String, Vec<serde_json::Value>>>>,
    model_cache: Arc<RwLock<ModelCache>>,
    model_price_cache: Arc<RwLock<ModelCache>>,
    models_dev_url: String,
    github_repo: String,
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
struct WorkspaceSavePayload {
    content: String,
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

#[derive(Deserialize, Serialize)]
struct MemoryPayload {
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

pub async fn run() -> anyhow::Result<()> {
    let args = Args::parse();
    if !args.insecure && args.auth_key.as_deref().unwrap_or_default().is_empty() {
        anyhow::bail!(
            "set HERMES_WEBUI_AUTH_KEY / --auth-key, or pass --insecure for local-only testing"
        );
    }

    let workspace = args.workspace.canonicalize().unwrap_or(args.workspace);
    let hermes_home = std::env::var_os("HERMES_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
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
    let (chat_streams, _) = broadcast::channel::<String>(CHAT_STREAM_BROADCAST_CAPACITY);
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
        chat_streams,
        active_chat_streams: Arc::new(RwLock::new(HashMap::new())),
        model_cache: Arc::new(RwLock::new(ModelCache::default())),
        model_price_cache: Arc::new(RwLock::new(ModelCache::default())),
        models_dev_url: args.models_dev_url,
        github_repo: args.github_repo,
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/runtime-config", get(runtime_config))
        .route("/login", get(login_page).post(login_submit))
        .route("/logout", post(logout))
        .route("/workspace/list", get(workspace_list))
        .route("/workspace/file", get(workspace_file).put(workspace_save))
        // Keep route shape discoverable for regression tests: .route("/workspace/item", patch(workspace_rename).delete(workspace_delete))
        .route(
            "/workspace/item",
            patch(workspace_rename).delete(workspace_delete),
        )
        .route("/skills/list", get(skills_list))
        .route("/skills/files", get(skill_files))
        .route("/skills/file", get(skill_file).put(skill_file_write))
        .route("/skills/toggle/{name}", post(skill_toggle))
        .route(
            "/skills/item",
            patch(skill_item_rename).delete(skill_item_delete),
        )
        .route("/skills/{name}", delete(skill_delete))
        .route("/memory", get(memory_get).put(memory_put))
        .route("/models-cache", get(models_cached))
        .route("/sessions/search", get(sessions_search))
        .route("/chat/messages/{session_id}", get(chat_messages_page))
        .route(
            "/chat/context-window/{session_id}",
            get(chat_context_window),
        )
        .route("/chat/attachments", post(chat_upload_attachments))
        .route("/version", get(yahu_version))
        .route("/update/check", get(check_update))
        .route("/update/apply", post(apply_update))
        .route("/insights/usage", get(insights_usage))
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

include!("auth.rs");
include!("assets.rs");
include!("proxy.rs");
include!("models.rs");
include!("sessions.rs");
include!("insights.rs");
include!("workspace.rs");
include!("skills.rs");
include!("memory.rs");
include!("chat_uploads.rs");
include!("update.rs");
include!("images.rs");
include!("tests.rs");
