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
        || path.starts_with("/chat/attachments")
        || path.starts_with("/insights")
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
