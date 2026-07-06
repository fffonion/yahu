#[derive(Deserialize)]
struct ChatMediaQuery {
    path: String,
    download: Option<String>,
}

async fn chat_media_file(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ChatMediaQuery>,
) -> Response<Body> {
    let file = match resolve_chat_media_path(&query.path, &state.hermes_home) {
        Ok(path) => path,
        Err(message) => return json_error(StatusCode::BAD_REQUEST, &message),
    };
    let bytes = match fs::read(&file).await {
        Ok(bytes) => bytes,
        Err(err) => return json_error(StatusCode::NOT_FOUND, &format!("cannot read media file: {err}")),
    };
    let mime = mime_guess::from_path(&file).first_or_octet_stream();
    let filename = file
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("attachment")
        .replace('"', "");
    let mut builder = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, mime.as_ref())
        .header(header::CACHE_CONTROL, "no-store")
        .header("X-Content-Type-Options", "nosniff");
    if query.download.as_deref() == Some("1") {
        builder = builder.header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{filename}\""),
        );
    }
    builder.body(Body::from(bytes)).unwrap()
}

fn resolve_chat_media_path(raw: &str, hermes_home: &Path) -> Result<PathBuf, String> {
    let mut value = raw
        .trim()
        .trim_matches(|c| matches!(c, '`' | '"' | '\''))
        .trim()
        .to_string();
    while matches!(value.chars().last(), Some('`' | '"' | '\'' | ',' | '.' | ';' | ':' | ')' | '}' | ']')) {
        value.pop();
    }
    if value.is_empty() {
        return Err("empty media path".to_string());
    }
    let expanded = if let Some(rest) = value.strip_prefix("~/") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(rest)
    } else {
        PathBuf::from(&value)
    };
    if !expanded.is_absolute() {
        return Err("media path must be absolute".to_string());
    }
    let resolved = expanded
        .canonicalize()
        .map_err(|err| format!("cannot resolve media path: {err}"))?;
    if !resolved.is_file() {
        return Err("media path is not a file".to_string());
    }
    if chat_media_path_denied(&resolved, hermes_home) {
        return Err("media path is denied".to_string());
    }
    Ok(resolved)
}

fn chat_media_path_denied(path: &Path, hermes_home: &Path) -> bool {
    let mut denied: Vec<PathBuf> = [
        "/etc", "/proc", "/sys", "/dev", "/root", "/boot", "/var/log", "/var/lib", "/var/run",
    ]
    .into_iter()
    .map(PathBuf::from)
    .collect();
    if let Some(home) = std::env::var_os("HOME").map(PathBuf::from) {
        for rel in [".ssh", ".aws", ".gnupg", ".kube", ".docker", ".config", ".azure", ".gcloud"] {
            denied.push(home.join(rel));
        }
    }
    for rel in [
        ".env",
        "auth.json",
        "auth.lock",
        "credentials",
        "config.yaml",
        ".anthropic_oauth.json",
        "google_token.json",
        "google_oauth_pending.json",
        "auth/google_oauth.json",
        "webhook_subscriptions.json",
        "cache/bws_cache.json",
        "pairing",
        "mcp-tokens",
    ] {
        denied.push(hermes_home.join(rel));
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .and_then(|p| p.canonicalize().ok());
    denied.into_iter().any(|root| {
        let Ok(root) = root.canonicalize().or_else(|_| Ok::<PathBuf, std::io::Error>(root.clone())) else { return false; };
        if home.as_ref().is_some_and(|h| h == &root) {
            return false;
        }
        path == root || path_is_within(path, &root)
    })
}

fn path_is_within(path: &Path, root: &Path) -> bool {
    path.strip_prefix(root).is_ok()
}
