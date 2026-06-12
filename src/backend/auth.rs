async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({"status":"ok"}))
}

async fn runtime_config(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "api_url": state.api_url.clone(),
        "api_proxy_base": "/hermes",
    }))
}

async fn require_auth(
    State(state): State<Arc<AppState>>,
    req: Request<Body>,
    next: Next,
) -> Response<Body> {
    let path = req.uri().path();
    if state.insecure || path == "/health" || path == "/login" || path == "/manifest.json" || path == "/sw.js" || path == "/icon.svg" || path == "/icon-192.png" || path == "/icon-512.png" {
        return next.run(req).await;
    }
    if let Some(session) = valid_session(req.headers(), &state) {
        let mut response = next.run(req).await;
        if let Some(cookie) = session.refresh_cookie
            && let Ok(value) = HeaderValue::from_str(&cookie)
        {
            response.headers_mut().append(header::SET_COOKIE, value);
        }
        return response;
    }
    if path.starts_with("/hermes")
        || path.starts_with("/workspace")
        || path.starts_with("/models-cache")
        || path.starts_with("/skills")
        || path.starts_with("/sessions/search")
        || path.starts_with("/chat/messages")
        || path.starts_with("/chat/watch")
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
    let cookie = session_cookie(&token);
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

struct ValidSession {
    refresh_cookie: Option<String>,
}

fn valid_session(headers: &HeaderMap, state: &AppState) -> Option<ValidSession> {
    let key = match state.auth_key.as_deref() {
        Some(key) if !key.is_empty() => key,
        _ => return None,
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
            return Some(ValidSession {
                refresh_cookie: session_token_refresh_cookie(token, key),
            });
        }
    }
    None
}

fn session_token_refresh_cookie(token: &str, key: &str) -> Option<String> {
    let claims = verified_session_claims(token, key)?;
    if now_secs().saturating_sub(claims.iat) <= SESSION_REFRESH_AFTER {
        return None;
    }
    Some(session_cookie(&make_session_token(key)))
}

fn session_cookie(token: &str) -> String {
    format!(
        "{}={}; Path=/; Max-Age={}; HttpOnly; SameSite=Lax",
        SESSION_COOKIE, token, SESSION_TTL
    )
}

fn make_session_token(key: &str) -> String {
    make_session_token_at(key, now_secs())
}

fn make_session_token_at(key: &str, iat: u64) -> String {
    let exp = iat.saturating_add(SESSION_TTL);
    let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let claims = URL_SAFE_NO_PAD.encode(
        serde_json::json!({
            "iss": "yahu",
            "iat": iat,
            "exp": exp,
        })
        .to_string(),
    );
    let signing_input = format!("{header}.{claims}");
    let sig = sign(key, &signing_input);
    format!("{signing_input}.{sig}")
}

fn verify_session_token(token: &str, key: &str) -> bool {
    verified_session_claims(token, key).is_some()
}

struct SessionClaims {
    iat: u64,
}

fn verified_session_claims(token: &str, key: &str) -> Option<SessionClaims> {
    let mut parts = token.split('.');
    let header = parts.next()?;
    let claims = parts.next()?;
    let sig = parts.next()?;
    if parts.next().is_some() {
        return None;
    }

    let signing_input = format!("{header}.{claims}");
    if sign(key, &signing_input) != sig {
        return None;
    }

    let header_json = decode_json_segment(header)?;
    if header_json.get("alg").and_then(|v| v.as_str()) != Some("HS256")
        || header_json.get("typ").and_then(|v| v.as_str()) != Some("JWT")
    {
        return None;
    }

    let claims_json = decode_json_segment(claims)?;
    if claims_json.get("iss").and_then(|v| v.as_str()) != Some("yahu") {
        return None;
    }
    let iat = claims_json.get("iat").and_then(|v| v.as_u64())?;
    let exp = claims_json.get("exp").and_then(|v| v.as_u64())?;
    if now_secs() > exp {
        return None;
    }
    Some(SessionClaims { iat })
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

fn decode_json_segment(segment: &str) -> Option<serde_json::Value> {
    let bytes = URL_SAFE_NO_PAD.decode(segment.as_bytes()).ok()?;
    serde_json::from_slice(&bytes).ok()
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
