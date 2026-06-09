#[derive(Deserialize)]
struct ChatUploadRequest {
    files: Vec<ChatUploadFile>,
}

#[derive(Deserialize)]
struct ChatUploadFile {
    name: String,
    mime: Option<String>,
    kind: Option<String>,
    data_url: String,
}

#[derive(Serialize)]
struct ChatUploadResponse {
    files: Vec<ChatUploadedFile>,
}

#[derive(Serialize)]
struct ChatUploadedFile {
    name: String,
    mime: String,
    kind: String,
    size: u64,
    path: String,
}

const MAX_CHAT_UPLOAD_FILES: usize = 12;
const MAX_CHAT_UPLOAD_BYTES: usize = 32 * 1024 * 1024;

async fn chat_upload_attachments(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<ChatUploadRequest>,
) -> impl IntoResponse {
    if payload.files.len() > MAX_CHAT_UPLOAD_FILES {
        return json_error(StatusCode::BAD_REQUEST, "too many attachments");
    }
    let upload_dir = state.hermes_home.join("cache").join("yahu_uploads");
    if let Err(err) = fs::create_dir_all(&upload_dir).await {
        return json_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            &format!("cannot create upload cache: {err}"),
        );
    }

    let mut saved = Vec::with_capacity(payload.files.len());
    for file in payload.files {
        let mime = file
            .mime
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("application/octet-stream")
            .to_string();
        let bytes = match decode_chat_upload_data_url(&file.data_url) {
            Ok(bytes) => bytes,
            Err(message) => return json_error(StatusCode::BAD_REQUEST, &message),
        };
        if bytes.len() > MAX_CHAT_UPLOAD_BYTES {
            return json_error(StatusCode::PAYLOAD_TOO_LARGE, "attachment is too large");
        }
        let safe_name = sanitize_chat_upload_filename(&file.name);
        let stored_name = unique_chat_upload_filename(&safe_name);
        let path = upload_dir.join(stored_name);
        if let Err(err) = fs::write(&path, &bytes).await {
            return json_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                &format!("cannot save attachment: {err}"),
            );
        }
        saved.push(ChatUploadedFile {
            name: safe_name,
            mime,
            kind: normalize_chat_upload_kind(file.kind.as_deref()),
            size: bytes.len() as u64,
            path: path.to_string_lossy().to_string(),
        });
    }

    Json(ChatUploadResponse { files: saved }).into_response()
}

fn decode_chat_upload_data_url(data_url: &str) -> Result<Vec<u8>, String> {
    let (meta, data) = data_url
        .split_once(',')
        .ok_or_else(|| "attachment data_url is malformed".to_string())?;
    if !meta.to_ascii_lowercase().contains(";base64") {
        return Err("attachment data_url must be base64".to_string());
    }
    STANDARD
        .decode(data.as_bytes())
        .map_err(|_| "attachment data_url base64 is invalid".to_string())
}

fn sanitize_chat_upload_filename(name: &str) -> String {
    let basename = Path::new(name)
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("attachment");
    let cleaned: String = basename
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' | '_' | ' ' => ch,
            _ => '_',
        })
        .collect();
    let trimmed = cleaned.trim_matches([' ', '.']).trim();
    let fallback = if trimmed.is_empty() { "attachment" } else { trimmed };
    fallback.chars().take(96).collect()
}

fn unique_chat_upload_filename(safe_name: &str) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_nanos();
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(safe_name.as_bytes());
    format!("{now:x}_{:08x}_{safe_name}", hasher.finalize())
}

fn normalize_chat_upload_kind(kind: Option<&str>) -> String {
    match kind.unwrap_or("file").trim().to_ascii_lowercase().as_str() {
        "image" => "image".to_string(),
        "text" => "text".to_string(),
        _ => "file".to_string(),
    }
}
